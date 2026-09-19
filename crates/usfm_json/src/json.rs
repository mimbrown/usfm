//! The AST as JSON.
//!
//! # The shape
//!
//! One object per node. Every object carries:
//!
//! * `"type"`: which node it is, snake_case, one of [`TYPES`]. The names are
//!   the AST's enum variants (`"chapter_start"`, `"opt_break"`), not USX's
//!   element names.
//! * `"span"`: `[start, end]`, the byte range of the source the node was read
//!   from. A synthesized node — a verse or chapter end, anything built by hand
//!   — carries `usfm_ast::SPAN`, which is `[0, 0]`. A `"text"` node's span is
//!   the run it was read from and is **not** a slice equal to its `"content"`
//!   (hardening plan D2: whitespace is normalised and escapes resolved).
//!
//! A node with a `StyleId` carries `"style"`, the marker name (`"p"`, `"w"`,
//! `"f"`) resolved through the document's own stylesheet — never the index,
//! which means nothing outside the process (plan D3). `"children"` holds the
//! child nodes: a table's rows, a row's cells, a paragraph's inlines, a
//! sidebar's or periph's blocks.
//!
//! The rest are the node's own fields, under their AST names: `code` and
//! `description` on a book, `number` / `alt_number` / `pub_number` on chapters
//! and verses, `caller` and `category` on a note, `category` on a sidebar,
//! `title` on a periph, `header` / `alignment` / `column` / `colspan` on a
//! cell, `content` on text, `usfm_version` on the document.
//!
//! Three rules hold everywhere:
//!
//! * **An absent optional field is left out.** The output never contains
//!   `null`; `"pub_number"` is there or it is not. The one place this carries
//!   information is `"attributes"`: a `\w` with no `|` at all has no
//!   `"attributes"` key, and a bare `\w word|\w*` has `"attributes": []`,
//!   which is the distinction `Char::attributes` draws (`Option<Attributes>`).
//!   A milestone draws it the same way since ticket 20: `\ts-s\*` has no
//!   `"attributes"` key and `\ts-s |\*` has `"attributes": []`.
//! * **A number that USFM writes as text stays text.** `"number"` is `"1"`,
//!   `"3-5"`, `"1,3\u{200f}-5"` — a verse number is a list of ranges, so it
//!   could not be a JSON number, and a chapter number is written the same way
//!   for consistency. `"column"` and `"colspan"` are JSON numbers: they are
//!   counts, not references.
//! * **Attributes keep their order and their duplicates**, because the tree
//!   does: `"attributes"` is an array of `{"name", "value"}` objects. The
//!   default (unnamed) attribute has `"name": ""`, exactly as
//!   `usfm_ast::Attribute` holds it — this writer does not apply
//!   `default_attribute_name`, which is an output convention of USX, not a
//!   fact about the tree. A consumer that wants the name resolves it itself.
//!
//! Two notes on the document object. `"usfm_version"` is
//! [`Document::usfm_version`], present only when the source had a `\usfm`
//! marker; the marker itself also stays in `"children"` as the paragraph it is
//! in the AST, which is what "the AST as it is" means and is where this output
//! differs from USX, which drops it. And a document has no span of its own, so
//! its `"span"` is `[0, end]`, from the start of the source to the furthest
//! point any of its blocks reached.
//!
//! Object keys come out sorted, which is what `serde_json`'s `Map` (a
//! `BTreeMap` unless its `preserve_order` feature is on, and this crate does
//! not turn features on for the workspace) does; `"type"` is therefore near
//! the end of an object, not at its front.
//!
//! ```json
//! {"children":[{"content":"God","span":[24,27],"type":"text"}],
//!  "span":[20,32],"style":"nd","type":"char"}
//! ```
//!
//! # Why a recursion and not a `Visit`
//!
//! [`usfm_ast::visit::Visit`] returns nothing from a `visit_*` method, so a
//! visitor that builds a value has to keep the children of the node being
//! built on a stack and swap it in and out, as `usfm_usx`'s writer does. That
//! earns its keep there because most of that writer's work is state carried
//! *between* nodes (the open chapter and verse). Here every node maps to
//! exactly one value and nothing is carried between them, so every `visit_*`
//! would be overridden and no generated walker would ever run: the trait would
//! be scaffolding around functions that already want to return their result.
//! The recursion below returns a `Value` per node and is the same traversal,
//! spelled directly. The one thing the trait buys — a compile error when a
//! node type is added to the AST — is covered by the `match`es here, which are
//! exhaustive over `Block` and `Inline`.

use serde_json::{Map, Value};
use usfm_ast::{
    Alignment, Attributes, Block, Book, Caller, ChapterEnd, ChapterStart, Char, Document, Inline,
    Milestone, Note, OptBreak, Para, Periph, Sidebar, Span, StyleId, Table, TableCell, TableRow,
    Text, VerseEnd, VerseStart,
};
use usfm_style::StyleSheet;

/// Every `"type"` string this crate can emit, in the order the AST declares
/// the variants.
///
/// A consumer switching on `"type"` has the whole vocabulary here, and the
/// coverage test in `tests/coverage.rs` parses a document that produces all of
/// them.
pub const TYPES: [&str; 17] = [
    "document",
    "book",
    "chapter_start",
    "chapter_end",
    "para",
    "table",
    "row",
    "cell",
    "sidebar",
    "periph",
    "milestone",
    "text",
    "char",
    "note",
    "verse_start",
    "verse_end",
    "opt_break",
];

/// The document as a JSON value, resolved against the document's own
/// stylesheet.
pub fn to_json_value(document: &Document) -> Value {
    JsonWriter {
        style_sheet: document.style_sheet(),
    }
    .document(document)
}

/// The document as compact JSON, on one line and with no trailing newline.
pub fn to_json_string(document: &Document) -> String {
    to_json_value(document).to_string()
}

/// The document as indented JSON, for reading rather than for piping.
pub fn to_json_string_pretty(document: &Document) -> String {
    serde_json::to_string_pretty(&to_json_value(document))
        .expect("a value built from the AST holds no map key that is not a string")
}

/// The walk, and the one thing it has to remember: the sheet the document
/// owns, which is the base sheet plus every style the parser derived
/// (hardening plan D3).
struct JsonWriter<'a> {
    style_sheet: &'a StyleSheet,
}

/// `[start, end]`.
fn span_value(span: Span) -> Value {
    Value::Array(vec![Value::from(span.start), Value::from(span.end)])
}

/// An object with the two fields every node has. `kind` is one of [`TYPES`],
/// which the debug build checks: a `"type"` the vocabulary does not list would
/// otherwise reach a consumer unannounced.
fn node(kind: &'static str, span: Span) -> Map<String, Value> {
    debug_assert!(TYPES.contains(&kind), "{kind} is not in TYPES");
    let mut map = Map::new();
    map.insert("type".to_owned(), Value::from(kind));
    map.insert("span".to_owned(), span_value(span));
    map
}

/// Add `value` under `key`.
fn set(map: &mut Map<String, Value>, key: &'static str, value: impl Into<Value>) {
    map.insert(key.to_owned(), value.into());
}

/// Add `value` under `key`, or nothing at all when it is `None`: the output
/// carries no `null`s.
fn set_optional(map: &mut Map<String, Value>, key: &'static str, value: Option<impl Into<Value>>) {
    if let Some(value) = value {
        set(map, key, value);
    }
}

/// The source range a block covers, for the document's own span.
fn block_span(block: &Block<'_>) -> Span {
    match block {
        Block::Book(book) => book.span,
        Block::ChapterStart(chapter) => chapter.span,
        Block::ChapterEnd(chapter) => chapter.span,
        Block::Para(para) => para.span,
        Block::Table(table) => table.span,
        Block::Milestone(milestone) => milestone.span,
        Block::Sidebar(sidebar) => sidebar.span,
        Block::Periph(periph) => periph.span,
    }
}

/// The alignment as USX and this output name it. Spelled out rather than taken
/// from `Display`, which would format through a `String` per cell.
fn alignment_name(alignment: &Alignment) -> &'static str {
    match alignment {
        Alignment::Start => "start",
        Alignment::Center => "center",
        Alignment::End => "end",
    }
}

/// The caller as written in the source: `+`, `-` or the custom text.
fn caller_str<'a>(caller: &'a Caller<'a>) -> &'a str {
    match caller {
        Caller::Plus => "+",
        Caller::Minus => "-",
        Caller::Custom(text) => text.as_ref(),
    }
}

impl JsonWriter<'_> {
    /// The marker name for a node's style, e.g. `"p"`.
    fn marker(&self, style: StyleId) -> &str {
        &self.style_sheet.get_rule(style.index()).marker
    }

    fn document(&self, document: &Document<'_>) -> Value {
        // A document has no span; it covers its source from the start to
        // wherever its last block ended.
        let end = document
            .blocks
            .iter()
            .map(|block| block_span(block).end)
            .max()
            .unwrap_or(0);
        let mut map = node("document", Span::new(0, end));
        set_optional(&mut map, "usfm_version", document.usfm_version());
        set(&mut map, "children", self.blocks(&document.blocks));
        Value::Object(map)
    }

    fn blocks(&self, blocks: &[Block<'_>]) -> Value {
        Value::Array(blocks.iter().map(|block| self.block(block)).collect())
    }

    fn inlines(&self, inlines: &[Inline<'_>]) -> Value {
        Value::Array(inlines.iter().map(|inline| self.inline(inline)).collect())
    }

    fn block(&self, block: &Block<'_>) -> Value {
        match block {
            Block::Book(book) => self.book(book),
            Block::ChapterStart(chapter) => self.chapter_start(chapter),
            Block::ChapterEnd(chapter) => self.chapter_end(chapter),
            Block::Para(para) => self.para(para),
            Block::Table(table) => self.table(table),
            Block::Milestone(milestone) => self.milestone(milestone),
            Block::Sidebar(sidebar) => self.sidebar(sidebar),
            Block::Periph(periph) => self.periph(periph),
        }
    }

    fn inline(&self, inline: &Inline<'_>) -> Value {
        match inline {
            Inline::Text(text) => self.text(text),
            Inline::VerseStart(verse) => self.verse_start(verse),
            Inline::VerseEnd(verse) => self.verse_end(verse),
            Inline::Char(char) => self.char(char),
            Inline::Note(note) => self.note(note),
            Inline::Milestone(milestone) => self.milestone(milestone),
            Inline::OptBreak(opt_break) => self.opt_break(opt_break),
        }
    }

    fn book(&self, book: &Book<'_>) -> Value {
        let mut map = node("book", book.span);
        set(&mut map, "code", book.code.as_str());
        set(&mut map, "description", book.description.as_ref());
        Value::Object(map)
    }

    fn chapter_start(&self, chapter: &ChapterStart<'_>) -> Value {
        let mut map = node("chapter_start", chapter.span);
        set(&mut map, "number", chapter.number.to_string());
        set_optional(
            &mut map,
            "alt_number",
            chapter.alt_number.as_ref().map(ToString::to_string),
        );
        set_optional(
            &mut map,
            "pub_number",
            chapter.pub_number.as_ref().map(|number| number.as_ref()),
        );
        Value::Object(map)
    }

    fn chapter_end(&self, chapter: &ChapterEnd) -> Value {
        let mut map = node("chapter_end", chapter.span);
        set(&mut map, "number", chapter.number.to_string());
        Value::Object(map)
    }

    fn para(&self, para: &Para<'_>) -> Value {
        let mut map = node("para", para.span);
        set(&mut map, "style", self.marker(para.style));
        set(&mut map, "children", self.inlines(&para.children));
        Value::Object(map)
    }

    fn table(&self, table: &Table<'_>) -> Value {
        let mut map = node("table", table.span);
        let rows = table.rows.iter().map(|row| self.row(row)).collect();
        set(&mut map, "children", Value::Array(rows));
        Value::Object(map)
    }

    fn row(&self, row: &TableRow<'_>) -> Value {
        let mut map = node("row", row.span);
        let cells = row.cells.iter().map(|cell| self.cell(cell)).collect();
        set(&mut map, "children", Value::Array(cells));
        Value::Object(map)
    }

    fn cell(&self, cell: &TableCell<'_>) -> Value {
        let mut map = node("cell", cell.span);
        set(&mut map, "header", cell.header);
        set(&mut map, "alignment", alignment_name(&cell.alignment));
        // The column the marker named, kept from the source rather than
        // counted: `\th3` after `\th1` is still 3.
        set(&mut map, "column", cell.column);
        set(&mut map, "colspan", cell.colspan);
        set(&mut map, "children", self.inlines(&cell.children));
        Value::Object(map)
    }

    fn sidebar(&self, sidebar: &Sidebar<'_>) -> Value {
        let mut map = node("sidebar", sidebar.span);
        set(&mut map, "style", self.marker(sidebar.style));
        set_optional(
            &mut map,
            "category",
            sidebar.category.as_ref().map(|text| text.content.as_ref()),
        );
        set(&mut map, "children", self.blocks(&sidebar.blocks));
        Value::Object(map)
    }

    fn periph(&self, periph: &Periph<'_>) -> Value {
        let mut map = node("periph", periph.span);
        set(&mut map, "style", self.marker(periph.style));
        set_optional(
            &mut map,
            "title",
            periph.title.as_ref().map(|text| text.content.as_ref()),
        );
        set_optional(
            &mut map,
            "attributes",
            periph.attributes.as_ref().map(attributes_value),
        );
        set(&mut map, "children", self.blocks(&periph.blocks));
        Value::Object(map)
    }

    fn milestone(&self, milestone: &Milestone<'_>) -> Value {
        let mut map = node("milestone", milestone.span);
        set(&mut map, "style", self.marker(milestone.style));
        // `Some` with no pairs is a bare `|` (`\ts-s |\*`), which is not no
        // `|` at all, so the key is absent for a milestone written without one.
        set_optional(
            &mut map,
            "attributes",
            milestone.attributes.as_ref().map(attributes_value),
        );
        Value::Object(map)
    }

    fn text(&self, text: &Text<'_>) -> Value {
        let mut map = node("text", text.span);
        set(&mut map, "content", text.content.as_ref());
        Value::Object(map)
    }

    fn char(&self, char: &Char<'_>) -> Value {
        let mut map = node("char", char.span);
        set(&mut map, "style", self.marker(char.style));
        // `Some` with no pairs is a bare `|`, which is not no `|` at all.
        set_optional(
            &mut map,
            "attributes",
            char.attributes.as_ref().map(attributes_value),
        );
        set(&mut map, "children", self.inlines(&char.children));
        Value::Object(map)
    }

    fn note(&self, note: &Note<'_>) -> Value {
        let mut map = node("note", note.span);
        set(&mut map, "style", self.marker(note.style));
        set(&mut map, "caller", caller_str(&note.caller));
        set_optional(
            &mut map,
            "category",
            note.category.as_ref().map(|text| text.content.as_ref()),
        );
        set(&mut map, "children", self.inlines(&note.children));
        Value::Object(map)
    }

    fn verse_start(&self, verse: &VerseStart<'_>) -> Value {
        let mut map = node("verse_start", verse.span);
        set(&mut map, "number", verse.number.to_string());
        set_optional(
            &mut map,
            "alt_number",
            verse.alt_number.as_ref().map(ToString::to_string),
        );
        set_optional(
            &mut map,
            "pub_number",
            verse.pub_number.as_ref().map(|number| number.as_ref()),
        );
        Value::Object(map)
    }

    fn verse_end(&self, verse: &VerseEnd) -> Value {
        let mut map = node("verse_end", verse.span);
        set(&mut map, "number", verse.number.to_string());
        Value::Object(map)
    }

    fn opt_break(&self, opt_break: &OptBreak) -> Value {
        Value::Object(node("opt_break", opt_break.span))
    }
}

/// An attribute list, in source order and with its duplicates: the default
/// attribute keeps the empty name the AST gives it.
fn attributes_value(attributes: &Attributes<'_>) -> Value {
    Value::Array(
        attributes
            .pairs
            .iter()
            .map(|attribute| {
                let mut map = Map::new();
                set(&mut map, "name", attribute.name.as_ref());
                set(&mut map, "value", attribute.value.as_ref());
                Value::Object(map)
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The vocabulary is a set, and `node` says so in a debug build. The
    /// end-to-end check that every one of these is reachable is
    /// `tests/coverage.rs`, which needs a parser this crate deliberately does
    /// not depend on outside its tests.
    #[test]
    fn the_type_vocabulary_has_no_repeats() {
        let mut seen: Vec<&str> = Vec::new();
        for kind in TYPES {
            assert!(!seen.contains(&kind), "{kind} is listed twice");
            seen.push(kind);
        }
    }

    /// A span is two numbers, and a synthesized node's is `[0, 0]`.
    #[test]
    fn a_span_is_a_pair_of_offsets() {
        assert_eq!(span_value(Span::new(3, 7)).to_string(), "[3,7]");
        assert_eq!(span_value(usfm_ast::SPAN).to_string(), "[0,0]");
    }

    /// An absent optional field is absent, not `null`.
    #[test]
    fn an_absent_field_is_left_out() {
        let mut map = node("text", usfm_ast::SPAN);
        set_optional(&mut map, "category", None::<&str>);
        set_optional(&mut map, "content", Some("a"));
        assert!(!map.contains_key("category"));
        assert_eq!(map["content"], Value::from("a"));
    }
}
