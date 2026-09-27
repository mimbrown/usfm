//! USX into a [`Document`]: the inverse of [`crate::usx`] (ticket 45, M7).
//!
//! ```
//! let usx = r#"<usx version="3.0"><book code="GEN" style="id"/>
//!   <chapter number="1" style="c" sid="GEN 1"/>
//!   <para style="p"><verse number="1" style="v" sid="GEN 1:1"/>In the beginning<verse eid="GEN 1:1"/></para>
//!   <chapter eid="GEN 1"/>
//! </usx>"#;
//! let result = usfm_usx::read_usx(usx);
//! assert!(result.diagnostics.is_empty());
//! assert_eq!(usfm_usx::to_usx_string(&result.document).matches("<verse").count(), 2);
//! ```
//!
//! The XML is `roxmltree`'s job, and a file that is not well-formed XML is
//! one [`Code::UsxNotWellFormed`] and an empty document: entities, CDATA,
//! encodings and a byte-order mark are a solved problem with nothing to do
//! with USFM (spec, M7). Recovery happens one level up, over the USX
//! vocabulary, and wherever a USX file is wrong the way its USFM would be, it
//! is reported with the parser's own code — a style the sheet does not list is
//! `unknown-marker`, a verse number that is not one is
//! `malformed-verse-number` — so a USX file and its USFM say the same thing.
//! What only a reader of USX can find has a `usx-…` code of its own
//! ([`Code::origin`] is [`Origin::Usx`](usfm_diagnostics::Origin::Usx)).
//!
//! Every element maps onto the node the writer in [`crate::usx`] writes it
//! from, attribute for attribute. What USX writes but the tree does not hold
//! is read only to be checked: `sid`, `eid` and `vid` are derived from the
//! numbers and the book, and the writer derives them again.
//!
//! **Spans** are byte ranges into the XML: a node's is its element's, a
//! `Text`'s the text node it was read from, an attribute's its name. As with
//! USFM, `&source[span]` is not expected to equal a `Text`'s content, since
//! entities are resolved and whitespace is normalised. A verse or chapter end
//! carries [`SPAN`], as the AST documents it: an end is where the tree says a
//! verse stops, and a USX 2 file, which has no `eid`, must read to the same
//! tree as a USX 3 one (spec, M7). An attribute list has no `|` in USX, so
//! its `pipe` is [`SPAN`] too.
//!
//! **Verse and chapter ends** (ticket 47). A USX 3 file's `<verse eid>` and
//! `<chapter eid>` are read where they stand. A USX 2 file has none, and the
//! reader builds every end where the parser builds it (plan D4,
//! `usfm_parser/tests/verse_ends.rs`): before the next verse in the same
//! container, with the space before it moved after the end; before the
//! container when the next verse is the first thing in it, and so on
//! outward; at block level, at the end of the last paragraph of verse text or
//! the last table cell, so never in a heading; not inside a sidebar; and
//! before a chapter, a `<periph>` and the end of the file, which close the
//! chapter too. So a file reads to one tree whichever version it is, and
//! nothing is reported. Which rule applies is decided per file: a file with
//! an `eid` anywhere closes its verses itself, and a verse or chapter it
//! leaves open is ended the same way and reported as
//! [`Code::UsxVerseEndMissing`] (machine.py's `Tes/MAT.usx`, a USX 2.6 book
//! with one `eid`, is that case). The rules are mirrored from the parser's
//! `OpenVerse`, not shared: the parser places an end while it builds the
//! container, from state only a parse has, and an output crate may not
//! depend on the parser; a shared pass over the finished tree is the path
//! arithmetic plan D4 took out of the parser. What keeps the two in step is
//! `tasks/conformance/tests/usx_reader.rs`, which strips the `eid`s from
//! every conformance reference and requires the same tree.
//!
//! **Whitespace** in a paragraph, a cell, a character style or a note is read
//! by the rules on [`Text`]: a run of ASCII whitespace is one space, and a
//! run is trimmed where the parser would trim it (after a marker, and at a
//! paragraph or cell end). That is what makes DBL's USX readable, which is
//! pretty-printed *inside* mixed content (`<verse … />\n    <char …>`).
//! Whitespace-only text between block elements is formatting and dropped.
//!
//! **What USX cannot say** is written in the canonical form, which is
//! `usfm_codegen`'s: an attribute is always named (`\w a|b\w*` and
//! `\w a|lemma="b"\w*` are one `<char lemma="b">`), and an attribute list
//! with no attributes is no list (`\ts-s |\*` and `\ts-s\*` are one `<ms>`).
//! `<usx version>` is always a `\usfm` paragraph, since a USX file always
//! declares one; `DEFAULT_USX_VERSION` is what the writer puts there for a
//! document that did not.

use std::borrow::Cow;
use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Arc;

use roxmltree::{Node, StringStorage};
use usfm_ast::string_parser::ParseStr;
use usfm_ast::{
    Alignment, Attribute, Attributes, Block, Book, BookCode, Caller, ChapterEnd, ChapterStart,
    Char, Document, Inline, InlineContainer, Milestone, Note, NumberList, OptBreak, Para, Periph,
    SPAN, Sidebar, StyleId, Table, TableCell, TableRow, Text, VerseEnd, VerseStart,
};
use usfm_diagnostics::{Code, Diagnostic, ParseResult};
use usfm_span::Span;
use usfm_style::{DEFAULT_STYLESHEET, StyleRule, StyleSheet, StyleType, TextProperties, TextType};

/// Read USX, resolving every `style` against the [default
/// stylesheet](DEFAULT_STYLESHEET).
pub fn read_usx(source: &str) -> ParseResult<'_> {
    read_usx_with(source, &DEFAULT_STYLESHEET)
}

/// Read USX, resolving every `style` against `style_sheet`. A style the sheet
/// does not list is derived onto the document's own copy of it, as the parser
/// derives one (plan D3), so the caller's sheet is never changed.
pub fn read_usx_with<'a>(source: &'a str, style_sheet: &Arc<StyleSheet>) -> ParseResult<'a> {
    let mut reader = Reader::new(source, style_sheet);
    let blocks = match roxmltree::Document::parse(source) {
        Ok(xml) => {
            reader.uses_eids = xml.descendants().any(|node| {
                matches!(name(node), "verse" | "chapter") && node.attribute("eid").is_some()
            });
            reader.document(xml.root_element())
        }
        Err(error) => {
            let position = error.pos();
            let at = offset_of(source, position.row, position.col);
            reader.report(
                Code::UsxNotWellFormed,
                Span::empty(at),
                format!("not well-formed XML: {error}"),
            );
            Vec::new()
        }
    };
    let document =
        Document::new(blocks, reader.style_sheet).with_span(Span::new(0, to_u32(source.len())));
    // In source order, as the parser and `usfm_semantic` report: a missing
    // end is only known when the next verse starts, and points back at the
    // start that was never ended.
    reader
        .diagnostics
        .sort_by_key(|diagnostic| diagnostic.span.start);
    ParseResult {
        document,
        diagnostics: reader.diagnostics,
    }
}

/// `roxmltree` positions an error by row and column, 1-based, the column in
/// characters; a diagnostic points at a byte.
fn offset_of(source: &str, row: u32, col: u32) -> u32 {
    let line_start = source
        .split_inclusive('\n')
        .take(row.saturating_sub(1) as usize)
        .map(str::len)
        .sum::<usize>();
    let line = &source[line_start.min(source.len())..];
    let column = line
        .char_indices()
        .nth(col.saturating_sub(1) as usize)
        .map_or(line.len(), |(index, _)| index);
    to_u32(line_start + column)
}

/// Spans are `u32`, as the parser's are: a book is megabytes, not gigabytes.
fn to_u32(offset: usize) -> u32 {
    u32::try_from(offset).unwrap_or(u32::MAX)
}

fn span_of(range: std::ops::Range<usize>) -> Span {
    Span::new(to_u32(range.start), to_u32(range.end))
}

/// `091`, `01-3`: a number (or the first number of a range) written with a
/// leading zero, which the parser reports as `number-has-leading-zero`. The
/// parser's own rule, repeated rather than shared because the reader does not
/// depend on the parser.
fn has_leading_zero(word: &str) -> bool {
    let mut chars = word.chars();
    chars.next() == Some('0') && chars.next().is_some_and(|c| c.is_ascii_digit())
}

/// A string as a `Cow` over the source when `roxmltree` could hand out a
/// slice of it (no entity was resolved), owned otherwise.
fn storage<'a>(storage: &StringStorage<'a>) -> Cow<'a, str> {
    match storage {
        StringStorage::Borrowed(text) => Cow::Borrowed(text),
        StringStorage::Owned(text) => Cow::Owned(text.to_string()),
    }
}

/// Rule 1 on [`Text`]: every run of ASCII whitespace is one space. A run that
/// already is — the common case — stays borrowed.
fn collapse_whitespace(text: Cow<'_, str>) -> Cow<'_, str> {
    let bytes = text.as_bytes();
    let normal = bytes.iter().enumerate().all(|(index, byte)| match byte {
        b' ' => bytes
            .get(index + 1)
            .is_none_or(|next| !next.is_ascii_whitespace()),
        byte => !byte.is_ascii_whitespace(),
    });
    if normal {
        return text;
    }
    let mut out = String::with_capacity(text.len());
    let mut in_whitespace = false;
    for character in text.chars() {
        if character.is_ascii_whitespace() {
            if !in_whitespace {
                out.push(' ');
                in_whitespace = true;
            }
        } else {
            out.push(character);
            in_whitespace = false;
        }
    }
    Cow::Owned(out)
}

/// Whether a text node is a pretty-printer's line break and indentation
/// rather than content: whitespace only, with a line break in it.
///
/// Between block elements every whitespace-only node is formatting. Inside
/// mixed content it is too when it holds a line break, because that is the
/// only way a pretty-printer can write it: Paratext indents the `<char>`s of
/// a note (`<note …>\n      <char style="xo">…</char>\n      <char …>`, where
/// the note has no space between them), and DBL breaks the line after
/// `<para>` and after `<verse …/>`, where rule 6 would drop the space anyway.
/// A space that *is* content is written as one: DBL separates its `<char
/// style="w">` words with `" "`. Text with anything else in it is read by
/// rule 1, so a line break inside a sentence is a space.
fn is_formatting(text: &str) -> bool {
    text.bytes().all(|byte| byte.is_ascii_whitespace()) && text.contains(['\n', '\r'])
}

/// A USX element's name.
fn name<'d>(node: Node<'d, '_>) -> &'d str {
    node.tag_name().name()
}

/// Attributes an element carries that are bookkeeping rather than content:
/// Paratext's `status="unknown"` on something its own checks flagged, and
/// `closed`, which records whether a marker was closed in the source — the
/// AST keeps neither, and the writer writes neither.
fn is_bookkeeping(attribute: &str) -> bool {
    matches!(attribute, "status" | "closed")
}

/// Where inline content is being read, for the rules that depend on it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Inside {
    /// A paragraph, a cell or a character style outside any note.
    Text,
    /// A note, or anything inside one: a `<verse>` there is `verse-in-note`.
    Note,
}

/// A list of blocks being read, with the implicit `\p` that holds content
/// found outside any paragraph until the next block element closes it
/// ([`Reader::close_implicit`], which every block element calls first).
struct BlockList<'a> {
    blocks: Vec<Block<'a>>,
    implicit: Option<Para<'a>>,
}

impl<'a> BlockList<'a> {
    fn new() -> Self {
        Self {
            blocks: Vec::new(),
            implicit: None,
        }
    }

    fn push(&mut self, block: Block<'a>) {
        debug_assert!(self.implicit.is_none(), "an implicit `\\p` left open");
        self.blocks.push(block);
    }

    fn finish(self) -> Vec<Block<'a>> {
        debug_assert!(self.implicit.is_none(), "an implicit `\\p` left open");
        self.blocks
    }
}

/// Rule 5 on [`Text`] at the end of a paragraph or a cell: trailing
/// whitespace goes. A verse that ends there ends *after* the text it closes,
/// so the text to trim may stand before the verse-end milestones; the space
/// [`push_verse_end`] leaves after one goes too.
fn trim_paragraph_end<'a>(container: &mut impl InlineContainer<'a>) {
    container.trim_trailing_whitespace();
    let children = container.children_mut();
    let ends = children
        .iter()
        .rev()
        .take_while(|child| matches!(child, Inline::VerseEnd(_)))
        .count();
    if ends == 0 || ends == children.len() {
        return;
    }
    let index = children.len() - ends - 1;
    if let Inline::Text(text) = &mut children[index] {
        let trimmed = text.trim_end_matches(|c: char| c.is_ascii_whitespace());
        if trimmed.is_empty() {
            children.remove(index);
        } else if trimmed.len() != text.len() {
            text.content = Cow::Owned(trimmed.to_string());
        }
    }
}

/// Whether `children` hold a verse start, directly or inside a character
/// style: the paragraph a verse started in holds its end whatever its style,
/// and `\ip \w p \v 2 b\w*` starts verse 2 in the `\ip` (ticket 49, found by
/// the `usx_roundtrip` fuzz target). A note cannot hold one (`verse-in-note`).
fn holds_verse_start(children: &[Inline<'_>]) -> bool {
    children.iter().any(|inline| match inline {
        Inline::VerseStart(_) => true,
        Inline::Char(char) => holds_verse_start(&char.children),
        _ => false,
    })
}

/// A verse end the tree synthesizes: an end is where the tree says a verse
/// stops, so it carries no span of its own.
fn verse_end<'a>(number: NumberList) -> Inline<'a> {
    Inline::VerseEnd(VerseEnd { number, span: SPAN })
}

/// A verse end goes before the whitespace in front of it, where the parser
/// puts it (`text<eid/> <v/>`, plan D4): a reference written the other way
/// round (`text <eid/><v/>`) reads to the same tree.
fn push_verse_end<'a>(container: &mut impl InlineContainer<'a>, end: VerseEnd) {
    let children = container.children_mut();
    let mut space = false;
    if let Some(Inline::Text(text)) = children.last_mut()
        && text.ends_with(|c: char| c.is_ascii_whitespace())
    {
        space = true;
        let trimmed = text.trim_end_matches(|c: char| c.is_ascii_whitespace());
        if trimmed.is_empty() {
            children.pop();
        } else {
            text.content = Cow::Owned(trimmed.to_string());
        }
    }
    children.push(Inline::VerseEnd(end));
    if space {
        children.push(Inline::Text(Text::synthesized(" ")));
    }
}

/// A verse whose end has been neither read nor built yet: the parser's
/// `OpenVerse`, and what the writer names in an `eid` or a `vid`.
struct OpenVerse {
    number: NumberList,
    /// Whether it started in the block being read. A paragraph a verse
    /// started in holds its end whatever its style.
    started_in_current_block: bool,
    /// The `<verse>` it started at, for `usx-verse-end-missing`.
    span: Span,
}

/// The walk, and what it has to remember while walking: the book, chapter and
/// verse the writer would name in a `sid`, `eid` or `vid`, so that the ones
/// the file has can be checked against them, and the state the verse-end
/// placement needs to build an end the file does not have (see
/// [`Reader::open_verse`]).
struct Reader<'a> {
    source: &'a str,
    /// The caller's sheet until a style has to be derived, then the
    /// document's own copy (`Arc::make_mut`), as in the parser.
    style_sheet: Arc<StyleSheet>,
    diagnostics: Vec<Diagnostic>,
    /// Styles this reader derived for a paragraph, character or note style
    /// the sheet did not list. The parser drops such a marker and reports each
    /// one; the reader keeps the node, so it reports each use of the style
    /// rather than only the first.
    derived: HashSet<usize>,
    book: Option<BookCode>,
    /// The chapter whose end is still to come, and its `<chapter>`.
    chapter: Option<(usize, Span)>,
    /// The verse whose end is still to come.
    verse: Option<OpenVerse>,
    /// Whether the file has an `eid` anywhere. Then it closes its verses and
    /// chapters itself, and one it leaves open is `usx-verse-end-missing`; a
    /// file with none is USX 2, whose ends are built without a word.
    uses_eids: bool,
    /// An end that could not go in the container where the next verse
    /// started, because that container was empty (or is a paragraph that is
    /// not verse text), with that container's depth: the parser's
    /// `pending_verse_end`.
    ///
    /// A stack, innermost last: an end handed out to block level can still be
    /// waiting when a verse starts at the head of a character style in the
    /// same paragraph and hands out a second (ticket 49).
    pending_verse_end: Vec<(OpenVerse, usize)>,
    /// Character styles and notes open around what is being read: 0 is a
    /// paragraph or a cell.
    depth: usize,
    /// Reading a table cell rather than a paragraph at depth 0.
    in_cell: bool,
    /// Whether the paragraph being read is verse text.
    para_is_verse_text: bool,
    /// Depth inside sidebars, where verses are not tracked.
    suspended: usize,
}

impl<'a> Reader<'a> {
    fn new(source: &'a str, style_sheet: &Arc<StyleSheet>) -> Self {
        Self {
            source,
            style_sheet: Arc::clone(style_sheet),
            diagnostics: Vec::new(),
            derived: HashSet::new(),
            book: None,
            chapter: None,
            verse: None,
            uses_eids: false,
            pending_verse_end: Vec::new(),
            depth: 0,
            in_cell: false,
            para_is_verse_text: true,
            suspended: 0,
        }
    }

    fn report(&mut self, code: Code, span: Span, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic::new(code, span, message));
    }

    /// An element's attribute, with the span of its value.
    fn attribute(&self, node: Node<'_, 'a>, name: &str) -> Option<(Cow<'a, str>, Span)> {
        node.attributes()
            .find(|attribute| attribute.name() == name)
            .map(|attribute| {
                (
                    storage(attribute.value_storage()),
                    span_of(attribute.range_value()),
                )
            })
    }

    /// An attribute that stands for a marker in USFM — a `\cat` category, a
    /// `\periph` title — as the [`Text`] the tree keeps for it. Its span is
    /// the whole attribute, `category="People"`, as a category's is the whole
    /// `\cat People\cat*` when it is parsed.
    fn attribute_text(&self, node: Node<'_, 'a>, name: &str) -> Option<Text<'a>> {
        node.attributes()
            .find(|attribute| attribute.name() == name)
            .map(|attribute| {
                Text::new(
                    storage(attribute.value_storage()),
                    span_of(attribute.range()),
                )
            })
    }

    /// An element that is dropped, reported as `usx-unknown-element`.
    fn unknown_element(&mut self, node: Node<'_, 'a>, why: &str) {
        let tag = name(node).to_string();
        self.report(
            Code::UsxUnknownElement,
            span_of(node.range()),
            format!("`<{tag}>` {why}; dropped"),
        );
    }

    // ----- styles ------------------------------------------------------

    /// The id of the style `marker` names, derived onto the document's sheet
    /// as a `kind` style when the sheet does not list it.
    ///
    /// A milestone is derived the way the parser derives one: silently when
    /// it is the `-s`/`-e` form of a marker the sheet has (`\k-s`), otherwise
    /// once per name as `unknown-milestone` (or `unknown-custom-milestone`
    /// for `\z`). Any other style is `unknown-marker` (or
    /// `unknown-custom-marker`), at every use: the parser drops such a marker
    /// wherever it stands, while the reader keeps the node, so the finding is
    /// the same and the tree differs by that node.
    fn style(&mut self, marker: &str, kind: StyleType, span: Span) -> StyleId {
        if let Some(&index) = self.style_sheet.get_marker_index(marker) {
            if self.derived.contains(&index) {
                self.report_unknown_marker(marker, span);
            }
            return StyleId::new(index as u32);
        }
        let mut text_type = TextType::Other;
        if kind == StyleType::Milestone {
            let base = marker.trim_end_matches("-s").trim_end_matches("-e");
            let derived_from = (marker.ends_with("-s") || marker.ends_with("-e"))
                .then(|| self.style_sheet.get_rule_by_marker(base))
                .flatten();
            match derived_from {
                Some(rule) => text_type = rule.text_type.clone(),
                None if marker.starts_with('z') => self.report(
                    Code::UnknownCustomMilestone,
                    span,
                    format!("custom milestone `\\{marker}` is not in the stylesheet"),
                ),
                None => self.report(
                    Code::UnknownMilestone,
                    span,
                    format!("milestone `\\{marker}` is not in the stylesheet"),
                ),
            }
        } else {
            self.report_unknown_marker(marker, span);
        }
        let rule = StyleRule {
            marker: marker.to_string(),
            name: None,
            description: None,
            style_type: kind.clone(),
            text_type,
            text_properties: TextProperties::default(),
            nest: false,
            occurs_under: vec![],
        };
        let index = Arc::make_mut(&mut self.style_sheet).add_rule(rule);
        if kind != StyleType::Milestone {
            self.derived.insert(index);
        }
        StyleId::new(index as u32)
    }

    fn report_unknown_marker(&mut self, marker: &str, span: Span) {
        if marker.starts_with('z') {
            self.report(
                Code::UnknownCustomMarker,
                span,
                format!("custom marker `\\{marker}` is not in the stylesheet; kept"),
            );
        } else {
            self.report(
                Code::UnknownMarker,
                span,
                format!("unknown marker `\\{marker}`; kept"),
            );
        }
    }

    /// The element's `style`, resolved as a `kind` style; `None`, reported,
    /// for an element that has none to be read by.
    ///
    /// Every caller but `<ms>`'s then reads what the element holds in its
    /// place, which is what the report says; a milestone holds nothing.
    fn element_style(&mut self, node: Node<'_, 'a>, kind: StyleType) -> Option<StyleId> {
        let Some((marker, span)) = self.attribute(node, "style") else {
            let why = if kind == StyleType::Milestone {
                "has no `style`; dropped"
            } else {
                "has no `style`; its content is read in its place"
            };
            let tag = name(node).to_string();
            self.report(
                Code::UsxUnknownElement,
                span_of(node.range()),
                format!("`<{tag}>` {why}"),
            );
            return None;
        };
        Some(self.style(&marker, kind, span))
    }

    /// The attributes of a `<char>`, `<ms>`, `<figure>`, `<ref>` or
    /// `<periph>` as the USFM attribute list they were written from. Every
    /// one is named, since USX cannot say which was the default; `None` when
    /// there are none, since it cannot say whether a `|` stood there either.
    fn attribute_list(
        &self,
        node: Node<'_, 'a>,
        skip: &[&str],
        rename: &[(&str, &'a str)],
    ) -> Option<Attributes<'a>> {
        let pairs: Vec<Attribute<'a>> = node
            .attributes()
            .filter(|attribute| {
                !skip.contains(&attribute.name()) && !is_bookkeeping(attribute.name())
            })
            .map(|attribute| {
                let written = &self.source[attribute.range_qname()];
                let name = rename
                    .iter()
                    .find(|(from, _)| *from == written)
                    .map_or(written, |(_, to)| *to);
                Attribute {
                    name: Cow::Borrowed(name),
                    value: storage(attribute.value_storage()),
                    span: span_of(attribute.range_qname()),
                }
            })
            .collect();
        (!pairs.is_empty()).then_some(Attributes { pairs, pipe: SPAN })
    }

    // ----- references --------------------------------------------------

    /// `BOOK C:V`, as the writer spells a `sid`, `eid` or `vid`.
    fn verse_reference(&self, number: &NumberList) -> String {
        format!(
            "{} {}:{}",
            self.book.unwrap_or(BookCode::Oth),
            self.chapter.map_or(0, |(number, _)| number),
            number
        )
    }

    fn chapter_reference(&self, number: usize) -> String {
        format!("{} {}", self.book.unwrap_or(BookCode::Oth), number)
    }

    /// Check a `sid` or `vid` against what the writer would write there.
    fn check_reference(&mut self, node: Node<'_, 'a>, attribute: &str, expected: Option<String>) {
        let Some((written, span)) = self.attribute(node, attribute) else {
            return;
        };
        match expected {
            Some(expected) if expected == written => {}
            Some(expected) => self.report(
                Code::UsxReferenceMismatch,
                span,
                format!("`{attribute}=\"{written}\"` should be `{expected}`"),
            ),
            None => self.report(
                Code::UsxReferenceMismatch,
                span,
                format!("`{attribute}=\"{written}\"` names a verse, but none is open"),
            ),
        }
    }

    /// A paragraph or table continuing an open verse may name it in `vid`.
    fn check_vid(&mut self, node: Node<'_, 'a>) {
        let expected = self
            .verse
            .as_ref()
            .map(|verse| self.verse_reference(&verse.number));
        self.check_reference(node, "vid", expected);
    }

    // ----- verse and chapter ends (plan D4) ------------------------------
    //
    // A USX 3 file's `eid`s are read where they stand. An end the file does
    // not have — every one in a USX 2 file, and in a USX 3 file the one a
    // verse or chapter was left without — is built where the parser builds
    // it, by the parser's rules (its `OpenVerse`), mirrored here in the order
    // the walk meets the same structure: before the next verse in the same
    // container; before the container when it is empty, and so on outward;
    // before a paragraph that is not verse text, unless the verse started in
    // it; at block level, at the end of the last paragraph of verse text or
    // the last table cell. So a file reads to one tree whichever version it
    // is, and `tasks/conformance/tests/usx_reader.rs` strips the `eid`s from
    // every reference to hold that.
    //
    // Mirrored, not shared: the parser places an end *while* it builds the
    // container, from state only a parse has (its open-marker stack, the
    // paragraph's text type, the `\esbe` line pushed before it is read), and
    // an output crate may not depend on the parser. The alternative — one
    // pass over a finished tree, in `usfm_ast`, that both would call — is the
    // path arithmetic plan D4 took out of the parser.

    /// Whether verse and chapter ends are built here at all: not inside a
    /// sidebar.
    fn tracking(&self) -> bool {
        self.suspended == 0
    }

    /// A new block is starting: the open verse, if any, did not start in it.
    fn start_block(&mut self) {
        if let Some(open) = &mut self.verse {
            open.started_in_current_block = false;
        }
    }

    /// A paragraph styled `style` is starting, at depth 0.
    fn start_paragraph(&mut self, style: StyleId) {
        self.para_is_verse_text = self.style_sheet.get_rule(style.index()).is_verse_text();
        self.in_cell = false;
        self.start_block();
    }

    /// `usx-verse-end-missing` for `what`, in a file that closes its verses
    /// itself. A USX 2 file has no `eid` to be missing.
    fn missing_end(&mut self, span: Span, what: String) {
        if self.uses_eids {
            self.report(
                Code::UsxVerseEndMissing,
                span,
                format!(
                    "{what} is never closed by an `eid`; it ends where the parser would end it"
                ),
            );
        }
    }

    /// A `<verse number>` in `container`: end the open verse, when the file
    /// has not, and open this one.
    fn open_verse(
        &mut self,
        container: &mut impl InlineContainer<'a>,
        number: NumberList,
        span: Span,
    ) {
        if self.tracking()
            && let Some(open) = self.verse.take()
        {
            let what = format!("verse `{}`", self.verse_reference(&open.number));
            self.missing_end(open.span, what);
            self.place_verse_end(container, open);
        }
        self.verse = Some(OpenVerse {
            number,
            started_in_current_block: true,
            span,
        });
    }

    /// Place the end of `open` before whatever is added to `container` next,
    /// or hand it outward when it belongs before `container` itself.
    fn place_verse_end(&mut self, container: &mut impl InlineContainer<'a>, open: OpenVerse) {
        let in_non_verse_text_para = self.depth == 0
            && !self.in_cell
            && !self.para_is_verse_text
            && !open.started_in_current_block;
        if in_non_verse_text_para || container.children().is_empty() {
            self.pending_verse_end.push((open, self.depth));
            return;
        }
        push_verse_end(
            container,
            VerseEnd {
                number: open.number,
                span: SPAN,
            },
        );
    }

    /// The pending verse end, if it was left by a container at `depth`.
    fn take_pending_verse_end(&mut self, depth: usize) -> Option<OpenVerse> {
        match self.pending_verse_end.last() {
            Some((_, at)) if *at == depth => self.pending_verse_end.pop().map(|(open, _)| open),
            _ => None,
        }
    }

    /// Add a character style to `container`, placing first a verse end that
    /// could not go inside it.
    fn add_char(&mut self, container: &mut impl InlineContainer<'a>, char: Char<'a>) {
        if let Some(open) = self.take_pending_verse_end(self.depth + 1) {
            self.place_verse_end(container, open);
        }
        container.add_child(Inline::Char(char));
    }

    /// A paragraph is about to be pushed: a verse end that belongs before it
    /// goes in the last block before it that can hold one.
    fn place_pending_verse_end_before_block(&mut self, blocks: &mut [Block<'a>]) {
        if let Some(open) = self.take_pending_verse_end(0) {
            self.end_verse_in_last_block(blocks, open.number);
        }
    }

    /// Close the open verse at a block boundary.
    fn end_verse_before_block(&mut self, blocks: &mut [Block<'a>]) {
        if !self.tracking() {
            return;
        }
        for (open, _) in std::mem::take(&mut self.pending_verse_end) {
            self.end_verse_in_last_block(blocks, open.number);
        }
        if let Some(open) = self.verse.take() {
            let what = format!("verse `{}`", self.verse_reference(&open.number));
            self.missing_end(open.span, what);
            self.end_verse_in_last_block(blocks, open.number);
        }
    }

    /// Close the open verse and the open chapter at a block boundary: a
    /// chapter, a `<periph>`, the end of a division or of the file.
    fn end_chapter(&mut self, blocks: &mut Vec<Block<'a>>) {
        if !self.tracking() {
            return;
        }
        self.end_verse_before_block(blocks);
        if let Some((number, span)) = self.chapter.take() {
            let what = format!("chapter `{}`", self.chapter_reference(number));
            self.missing_end(span, what);
            blocks.push(Block::ChapterEnd(ChapterEnd { number, span: SPAN }));
        }
    }

    /// Append the end of verse `number` to the last paragraph of verse text,
    /// or the last table cell, in `blocks`. Sidebars, periphs, headings and
    /// other non-verse blocks are skipped: a verse never ends inside them.
    fn end_verse_in_last_block(&self, blocks: &mut [Block<'a>], number: NumberList) {
        for block in blocks.iter_mut().rev() {
            match block {
                Block::Para(para) => {
                    // A paragraph the verse started in holds its end whatever
                    // its style; otherwise only verse text qualifies.
                    let starts_a_verse = holds_verse_start(&para.children);
                    if !para.children.is_empty()
                        && (starts_a_verse
                            || self
                                .style_sheet
                                .get_rule(para.style.index())
                                .is_verse_text())
                    {
                        para.children.push(verse_end(number));
                        return;
                    }
                }
                Block::Table(table) => {
                    if let Some(cell) = table.rows.last_mut().and_then(|row| row.cells.last_mut()) {
                        cell.children.push(verse_end(number));
                        return;
                    }
                }
                _ => {}
            }
        }
    }

    /// Close `list`'s implicit `\p`, placing first a verse end that belongs
    /// before it. Every block element calls this before it is read, so that a
    /// verse end the element hands outward lands after the implicit
    /// paragraph, not before it.
    fn close_implicit(&mut self, list: &mut BlockList<'a>) {
        if let Some(mut para) = list.implicit.take() {
            trim_paragraph_end(&mut para);
            self.place_pending_verse_end_before_block(&mut list.blocks);
            list.blocks.push(Block::Para(para));
        }
    }

    // ----- blocks ------------------------------------------------------

    fn document(&mut self, root: Node<'_, 'a>) -> Vec<Block<'a>> {
        if name(root) != "usx" {
            let tag = name(root).to_string();
            self.report(
                Code::UsxUnknownElement,
                span_of(root.range()),
                format!("the root element is `<{tag}>`, not `<usx>`; its content is read"),
            );
        }
        let mut list = BlockList::new();
        self.blocks(root, &mut list);
        // End of input closes the open verse and chapter.
        self.close_implicit(&mut list);
        self.end_chapter(&mut list.blocks);
        let mut blocks = list.finish();
        // The version is a `\usfm` paragraph, which USFM writes right after
        // `\id` (see `Document::usfm_version`). It was read from the
        // attribute, so that is its span: `version="3.1"` for the paragraph,
        // `3.1` for its text.
        if let Some(version) = root
            .attributes()
            .find(|attribute| attribute.name() == "version")
            && let Some(&usfm) = self.style_sheet.get_marker_index("usfm")
        {
            let text = Text::new(
                storage(version.value_storage()),
                span_of(version.range_value()),
            );
            let para = Block::Para(Para {
                style: StyleId::new(usfm as u32),
                children: vec![Inline::Text(text)],
                span: span_of(version.range()),
            });
            let at = usize::from(matches!(blocks.first(), Some(Block::Book(_))));
            blocks.insert(at, para);
        }
        blocks
    }

    /// The children of `parent` as blocks, appended to `list`.
    fn blocks(&mut self, parent: Node<'_, 'a>, list: &mut BlockList<'a>) {
        for child in parent.children() {
            if child.is_text() {
                if child
                    .text()
                    .is_some_and(|text| !text.trim_ascii().is_empty())
                {
                    self.outside_paragraph(child, list);
                }
                continue;
            }
            if !child.is_element() {
                continue;
            }
            match name(child) {
                "book" => {
                    self.close_implicit(list);
                    if let Some(book) = self.book(child) {
                        list.push(Block::Book(book));
                    }
                }
                "chapter" => {
                    self.close_implicit(list);
                    if let Some(block) = self.chapter(child, &mut list.blocks) {
                        list.push(block);
                    }
                }
                "para" => match self.element_style(child, StyleType::Paragraph) {
                    Some(style) => {
                        self.close_implicit(list);
                        if self.style_sheet.get_rule(style.index()).marker == "esbe" {
                            // A sidebar ends at `</sidebar>`: this is an
                            // `\esbe` with no open `\esb`, kept as the empty
                            // paragraph the parser keeps (ticket 49).
                            self.report(
                                Code::UnmatchedSidebarEnd,
                                span_of(child.range()),
                                "`\\esbe` with no open `\\esb`",
                            );
                        }
                        let para = self.para(child, style);
                        self.place_pending_verse_end_before_block(&mut list.blocks);
                        list.push(Block::Para(para));
                    }
                    None => self.blocks(child, list),
                },
                "table" => {
                    self.close_implicit(list);
                    let (table, before_table) = self.table(child);
                    if let Some(open) = before_table {
                        self.end_verse_in_last_block(&mut list.blocks, open.number);
                    }
                    list.push(Block::Table(table));
                }
                "ms" => {
                    self.close_implicit(list);
                    if let Some(milestone) = self.milestone(child) {
                        list.push(Block::Milestone(milestone));
                    }
                }
                "sidebar" => match self.element_style(child, StyleType::Paragraph) {
                    Some(style) => {
                        self.close_implicit(list);
                        let sidebar = self.sidebar(child, style);
                        list.push(Block::Sidebar(sidebar));
                    }
                    None => self.blocks(child, list),
                },
                "periph" => {
                    // A division runs to the next `<periph>` or the end of
                    // the book, so the verse and chapter open before it end
                    // before it (the parser's rule since ticket 44).
                    self.close_implicit(list);
                    self.end_chapter(&mut list.blocks);
                    let periph = self.periph(child);
                    list.push(Block::Periph(periph));
                }
                "verse" | "char" | "note" | "figure" | "ref" | "optbreak" => {
                    self.outside_paragraph(child, list);
                }
                "unmatched" => self.unmatched(child),
                "row" | "cell" => {
                    self.unknown_element(child, "stands outside a `<table>`");
                }
                _ => {
                    // `<list>` and anything else: the element goes, what it
                    // holds is read where it stands.
                    let tag = name(child).to_string();
                    self.report(
                        Code::UsxUnknownElement,
                        span_of(child.range()),
                        format!("`<{tag}>` is not a USX element; its content is read in its place"),
                    );
                    self.blocks(child, list);
                }
            }
        }
    }

    /// Inline content between blocks goes into an implicit `\p`, as the
    /// parser keeps content outside a paragraph.
    fn outside_paragraph(&mut self, node: Node<'_, 'a>, list: &mut BlockList<'a>) {
        let span = span_of(node.range());
        if list.implicit.is_none() {
            let Some(&p) = self.style_sheet.get_marker_index("p") else {
                self.report(
                    Code::ContentDropped,
                    span,
                    "content outside a paragraph, and no `\\p` in the stylesheet to hold it; dropped",
                );
                return;
            };
            self.report(
                Code::ContentOutsideParagraph,
                span,
                "content outside a paragraph; kept in an implicit `\\p`",
            );
            self.start_paragraph(StyleId::new(p as u32));
            list.implicit = Some(Para {
                style: StyleId::new(p as u32),
                children: Vec::new(),
                span,
            });
        }
        let mut para = list.implicit.take().expect("opened above");
        para.span.end = span.end;
        self.inline(node, &mut para, Inside::Text);
        list.implicit = Some(para);
    }

    fn book(&mut self, node: Node<'_, 'a>) -> Option<Book<'a>> {
        let span = span_of(node.range());
        let Some((code, code_span)) = self.attribute(node, "code") else {
            self.report(
                Code::MissingBookCode,
                span,
                "`<book>` has no `code`; dropped",
            );
            return None;
        };
        let Ok(code) = BookCode::from_str(&code) else {
            self.report(
                Code::UnknownBookCode,
                code_span,
                format!("`{code}` is not a book code"),
            );
            return None;
        };
        self.book = Some(code);
        let description: String = node
            .descendants()
            .filter(|node| node.is_text())
            .filter_map(|node| node.text())
            .collect();
        let description = collapse_whitespace(Cow::Owned(description));
        let description = description.trim_matches(|c: char| c.is_ascii_whitespace());
        Some(Book {
            code,
            description: Cow::Owned(description.to_string()),
            span,
        })
    }

    /// A `<chapter>`. A start ends the open verse and chapter first, into
    /// `blocks`, when the file has not ended them itself.
    fn chapter(&mut self, node: Node<'_, 'a>, blocks: &mut Vec<Block<'a>>) -> Option<Block<'a>> {
        let span = span_of(node.range());
        if let Some((number, number_span)) = self.attribute(node, "number") {
            let Ok(parsed) = number.parse::<usize>() else {
                self.report(
                    Code::MalformedChapterNumber,
                    number_span,
                    format!("`{number}` is not a valid chapter number"),
                );
                return None;
            };
            if has_leading_zero(&number) {
                self.report(
                    Code::NumberHasLeadingZero,
                    number_span,
                    format!("chapter number `{number}` has a leading zero"),
                );
            }
            let alt_number = match self.attribute(node, "altnumber") {
                Some((alt, alt_span)) => match NumberList::parse_str(&alt) {
                    Ok(alt) => Some(alt),
                    Err(_) => {
                        self.report(
                            Code::MalformedChapterNumber,
                            alt_span,
                            format!("`{alt}` is not a valid alternate chapter number"),
                        );
                        None
                    }
                },
                None => None,
            };
            let pub_number = self.attribute(node, "pubnumber").map(|(value, _)| value);
            self.end_chapter(blocks);
            self.chapter = Some((parsed, span));
            let expected = self.chapter_reference(parsed);
            self.check_reference(node, "sid", Some(expected));
            return Some(Block::ChapterStart(ChapterStart {
                number: parsed,
                alt_number,
                pub_number,
                span,
            }));
        }
        if let Some((eid, eid_span)) = self.attribute(node, "eid") {
            // A verse still open ends before the chapter does.
            self.end_verse_before_block(blocks);
            let Some((open, _)) = self.chapter.take() else {
                self.report(
                    Code::UsxVerseEndMismatch,
                    eid_span,
                    format!("`<chapter eid=\"{eid}\">` ends no open chapter; dropped"),
                );
                return None;
            };
            let expected = self.chapter_reference(open);
            if eid != expected {
                self.report(
                    Code::UsxVerseEndMismatch,
                    eid_span,
                    format!("`eid=\"{eid}\"` ends chapter `{expected}`, which is the one open"),
                );
            }
            return Some(Block::ChapterEnd(ChapterEnd {
                number: open,
                span: SPAN,
            }));
        }
        self.report(
            Code::MissingChapterNumber,
            span,
            "`<chapter>` has neither a `number` nor an `eid`; dropped",
        );
        None
    }

    fn para(&mut self, node: Node<'_, 'a>, style: StyleId) -> Para<'a> {
        self.check_vid(node);
        self.start_paragraph(style);
        let mut para = Para {
            style,
            children: Vec::new(),
            span: span_of(node.range()),
        };
        self.inlines(node, &mut para, Inside::Text);
        trim_paragraph_end(&mut para);
        para
    }

    /// A `<table>`, and the end of a verse that a `<verse>` at the start of
    /// its first cell left for the caller to place before it.
    fn table(&mut self, node: Node<'_, 'a>) -> (Table<'a>, Option<OpenVerse>) {
        self.check_vid(node);
        self.start_block();
        let mut rows: Vec<TableRow<'a>> = Vec::new();
        let mut before_table = None;
        for child in node.children() {
            if child.is_element() && name(child) == "row" {
                let (row, before_row) = self.row(child);
                // A verse starting in the first cell of this row ends the
                // previous one at the end of the previous row's last cell,
                // or before the table when this is the first row.
                if let Some(open) = before_row {
                    match rows.last_mut().and_then(|row| row.cells.last_mut()) {
                        Some(cell) => cell.children.push(verse_end(open.number)),
                        None => before_table = Some(open),
                    }
                }
                rows.push(row);
            } else if child.is_element() {
                self.unknown_element(child, "is not a row of the `<table>` it stands in");
            } else if child
                .text()
                .is_some_and(|text| !text.trim_ascii().is_empty())
            {
                self.report(
                    Code::UsxUnknownElement,
                    span_of(child.range()),
                    "text outside any row of a `<table>`; dropped",
                );
            }
        }
        let table = Table {
            rows,
            span: span_of(node.range()),
        };
        (table, before_table)
    }

    /// A row's cells. `usx.rnc` lets a `<verse>` stand between them
    /// (usfm-grammar ends a verse after the last cell); the tree has nowhere
    /// but a cell to put it, so an end goes into the cell before it and a
    /// start into the cell after it.
    ///
    /// Also returns the end of a verse that a `<verse>` at the start of the
    /// row's first cell left for the caller to place before the row.
    fn row(&mut self, node: Node<'_, 'a>) -> (TableRow<'a>, Option<OpenVerse>) {
        let mut cells: Vec<TableCell<'a>> = Vec::new();
        let mut before_row = None;
        let mut pending: Vec<Node<'_, 'a>> = Vec::new();
        for child in node.children() {
            if !child.is_element() {
                if child
                    .text()
                    .is_some_and(|text| !text.trim_ascii().is_empty())
                {
                    self.report(
                        Code::UsxUnknownElement,
                        span_of(child.range()),
                        "text outside any cell of a `<row>`; dropped",
                    );
                }
                continue;
            }
            match name(child) {
                "cell" => {
                    let next_column = cells
                        .last()
                        .map_or(1, |cell| cell.column.saturating_add(cell.colspan));
                    let cell = self.cell(child, next_column, &mut pending);
                    // A verse starting at the beginning of this cell ends
                    // the previous one at the end of the previous cell.
                    if let Some(open) = self.take_pending_verse_end(0) {
                        match cells.last_mut() {
                            Some(previous) => previous.children.push(verse_end(open.number)),
                            None => before_row = Some(open),
                        }
                    }
                    cells.push(cell);
                }
                "verse" if self.attribute(child, "eid").is_some() && !cells.is_empty() => {
                    let cell = cells.last_mut().expect("checked non-empty");
                    self.inline(child, cell, Inside::Text);
                }
                "verse" => pending.push(child),
                _ => self.unknown_element(child, "is not a cell of the `<row>` it stands in"),
            }
        }
        if let Some(cell) = cells.last_mut() {
            self.in_cell = true;
            for verse in pending.drain(..) {
                self.inline(verse, cell, Inside::Text);
            }
            self.in_cell = false;
        } else {
            for verse in pending {
                self.unknown_element(verse, "stands in a `<row>` with no cell to hold it");
            }
        }
        let row = TableRow {
            cells,
            span: span_of(node.range()),
        };
        (row, before_row)
    }

    /// A `<cell style="tcr1-2">`: header or not, the alignment letter and the
    /// columns it spans are all in the style, which is how the writer spells
    /// them (`align` repeats the letter and is not read).
    ///
    /// `pending` holds the `<verse>` starts the row had before this cell,
    /// which open it.
    fn cell(
        &mut self,
        node: Node<'_, 'a>,
        next_column: u8,
        pending: &mut Vec<Node<'_, 'a>>,
    ) -> TableCell<'a> {
        let span = span_of(node.range());
        let parsed = self
            .attribute(node, "style")
            .map(|(style, style_span)| (parse_cell_style(&style), style, style_span));
        let (header, alignment, column, colspan) = match parsed {
            Some((Some(parsed), _, _)) => parsed,
            Some((None, style, style_span)) => {
                self.report_unknown_marker(&style, style_span);
                (false, Alignment::Start, next_column, 1)
            }
            None => {
                self.report(
                    Code::UsxUnknownElement,
                    span,
                    "`<cell>` has no `style`; read as the next `\\tc` column",
                );
                (false, Alignment::Start, next_column, 1)
            }
        };
        let mut cell = TableCell {
            header,
            alignment,
            column,
            colspan,
            children: Vec::new(),
            span,
        };
        self.in_cell = true;
        for verse in pending.drain(..) {
            self.inline(verse, &mut cell, Inside::Text);
        }
        self.inlines(node, &mut cell, Inside::Text);
        self.in_cell = false;
        trim_paragraph_end(&mut cell);
        cell
    }

    fn sidebar(&mut self, node: Node<'_, 'a>, style: StyleId) -> Sidebar<'a> {
        let category = self.attribute_text(node, "category");
        // A sidebar is outside the verse flow, as the writer has it: its
        // paragraphs name no verse, and the one open before it is open again
        // after it. As in the parser, verses are not tracked inside: no end
        // is built there, and none is built for a verse started there.
        let mut verse = self.verse.take();
        self.suspended += 1;
        let mut list = BlockList::new();
        self.blocks(node, &mut list);
        self.close_implicit(&mut list);
        self.suspended -= 1;
        // The sidebar was a block of its own, so the verse open around it did
        // not start in whatever block comes next.
        if let Some(open) = &mut verse {
            open.started_in_current_block = false;
        }
        self.verse = verse;
        Sidebar {
            style,
            category,
            blocks: list.finish(),
            span: span_of(node.range()),
        }
    }

    fn periph(&mut self, node: Node<'_, 'a>) -> Periph<'a> {
        let span = span_of(node.range());
        let style = self.style("periph", StyleType::Paragraph, span);
        let title = self.attribute_text(node, "alt");
        let attributes = self.attribute_list(node, &["alt", "style"], &[]);
        let mut list = BlockList::new();
        self.blocks(node, &mut list);
        // Whatever the division opened ends with it.
        self.close_implicit(&mut list);
        self.end_chapter(&mut list.blocks);
        Periph {
            style,
            title,
            attributes,
            blocks: list.finish(),
            span,
        }
    }

    fn unmatched(&mut self, node: Node<'_, 'a>) {
        let marker = self
            .attribute(node, "marker")
            .map_or(Cow::Borrowed("?"), |(marker, _)| marker);
        self.report(
            Code::UsxUnmatched,
            span_of(node.range()),
            format!(
                "`<unmatched marker=\"{marker}\">`: a closing marker that closed nothing; dropped"
            ),
        );
    }

    // ----- inlines -----------------------------------------------------

    /// The children of `parent`, read into `container`.
    fn inlines(
        &mut self,
        parent: Node<'_, 'a>,
        container: &mut impl InlineContainer<'a>,
        inside: Inside,
    ) {
        for child in parent.children() {
            self.inline(child, container, inside);
        }
    }

    /// One node read into `container`.
    fn inline(
        &mut self,
        node: Node<'_, 'a>,
        container: &mut impl InlineContainer<'a>,
        inside: Inside,
    ) {
        if node.is_text() {
            if let Some(text) = node.text_storage()
                && !is_formatting(text.as_str())
            {
                let content = collapse_whitespace(storage(text));
                if !content.is_empty() {
                    container.add_child(Inline::Text(Text::new(content, span_of(node.range()))));
                }
            }
            return;
        }
        if !node.is_element() {
            return;
        }
        match name(node) {
            "verse" => self.verse(node, container, inside),
            "char" => match self.element_style(node, StyleType::Character) {
                Some(style) => {
                    let char = self.char(node, style, &["style"], &[], inside);
                    self.add_char(container, char);
                }
                None => self.inlines(node, container, inside),
            },
            "figure" => {
                let span = span_of(node.range());
                let style = self.style("fig", StyleType::Character, span);
                // The writer spells `\fig`'s `src` as USX's `file`.
                let char = self.char(node, style, &["style"], &[("file", "src")], inside);
                self.add_char(container, char);
            }
            "ref" => {
                let span = span_of(node.range());
                let style = self.style("ref", StyleType::Character, span);
                let char = self.char(node, style, &["style"], &[], inside);
                self.add_char(container, char);
            }
            "note" => match self.note(node) {
                Some(note) => container.add_child(Inline::Note(note)),
                None => self.inlines(node, container, Inside::Note),
            },
            "ms" => {
                if let Some(milestone) = self.milestone(node) {
                    container.add_child(Inline::Milestone(milestone));
                }
            }
            "optbreak" => container.add_child(Inline::OptBreak(OptBreak {
                span: span_of(node.range()),
            })),
            "unmatched" => self.unmatched(node),
            tag @ ("book" | "chapter" | "para" | "table" | "row" | "cell" | "sidebar"
            | "periph") => {
                let tag = tag.to_string();
                self.report(
                    Code::UsxUnknownElement,
                    span_of(node.range()),
                    format!("`<{tag}>` cannot stand inside inline content; its content is read in its place"),
                );
                self.inlines(node, container, inside);
            }
            _ => {
                let tag = name(node).to_string();
                self.report(
                    Code::UsxUnknownElement,
                    span_of(node.range()),
                    format!("`<{tag}>` is not a USX element; its content is read in its place"),
                );
                self.inlines(node, container, inside);
            }
        }
    }

    fn verse(
        &mut self,
        node: Node<'_, 'a>,
        container: &mut impl InlineContainer<'a>,
        inside: Inside,
    ) {
        let span = span_of(node.range());
        if let Some((number, number_span)) = self.attribute(node, "number") {
            if inside == Inside::Note {
                self.report(
                    Code::VerseInNote,
                    span,
                    format!("verse `{number}` inside a note; dropped"),
                );
                return;
            }
            let parsed = match NumberList::parse_str(&number) {
                Ok(parsed) => parsed,
                Err(_) => {
                    self.report(
                        Code::MalformedVerseNumber,
                        number_span,
                        format!("`{number}` is not a valid verse number"),
                    );
                    return;
                }
            };
            if has_leading_zero(&number) {
                self.report(
                    Code::NumberHasLeadingZero,
                    number_span,
                    format!("verse number `{number}` has a leading zero"),
                );
            }
            let alt_number = match self.attribute(node, "altnumber") {
                Some((alt, alt_span)) => match NumberList::parse_str(&alt) {
                    Ok(alt) => Some(alt),
                    Err(_) => {
                        self.report(
                            Code::MalformedVerseNumber,
                            alt_span,
                            format!("`{alt}` is not a valid alternate verse number"),
                        );
                        None
                    }
                },
                None => None,
            };
            let pub_number = self.attribute(node, "pubnumber").map(|(value, _)| value);
            let expected = self.verse_reference(&parsed);
            self.check_reference(node, "sid", Some(expected));
            self.open_verse(container, parsed.clone(), span);
            container.add_child(Inline::VerseStart(VerseStart {
                number: parsed,
                alt_number,
                pub_number,
                span,
            }));
            return;
        }
        if let Some((eid, eid_span)) = self.attribute(node, "eid") {
            let Some(open) = self.verse.take() else {
                self.report(
                    Code::UsxVerseEndMismatch,
                    eid_span,
                    format!("`<verse eid=\"{eid}\">` ends no open verse; dropped"),
                );
                return;
            };
            let expected = self.verse_reference(&open.number);
            if eid != expected {
                self.report(
                    Code::UsxVerseEndMismatch,
                    eid_span,
                    format!("`eid=\"{eid}\"` ends verse `{expected}`, which is the one open"),
                );
            }
            push_verse_end(
                container,
                VerseEnd {
                    number: open.number,
                    span: SPAN,
                },
            );
            return;
        }
        self.report(
            Code::MissingVerseNumber,
            span,
            "`<verse>` has neither a `number` nor an `eid`; dropped",
        );
    }

    fn char(
        &mut self,
        node: Node<'_, 'a>,
        style: StyleId,
        skip: &[&str],
        rename: &[(&str, &'a str)],
        inside: Inside,
    ) -> Char<'a> {
        let mut char = Char {
            style,
            children: Vec::new(),
            attributes: self.attribute_list(node, skip, rename),
            span: span_of(node.range()),
        };
        self.depth += 1;
        self.inlines(node, &mut char, inside);
        self.depth -= 1;
        char
    }

    fn note(&mut self, node: Node<'_, 'a>) -> Option<Note<'a>> {
        let style = self.element_style(node, StyleType::Note)?;
        let span = span_of(node.range());
        let caller = match self.attribute(node, "caller") {
            Some((Cow::Borrowed(caller), _)) => Caller::from(caller),
            Some((Cow::Owned(caller), _)) => match caller.as_str() {
                "+" => Caller::Plus,
                "-" => Caller::Minus,
                _ => Caller::Custom(Cow::Owned(caller)),
            },
            None => {
                self.report(
                    Code::MissingNoteCaller,
                    span,
                    "`<note>` has no `caller`; `+` assumed",
                );
                Caller::Plus
            }
        };
        let category = self.attribute_text(node, "category");
        let mut note = Note {
            style,
            caller,
            category,
            children: Vec::new(),
            span,
        };
        self.depth += 1;
        self.inlines(node, &mut note, Inside::Note);
        self.depth -= 1;
        Some(note)
    }

    fn milestone(&mut self, node: Node<'_, 'a>) -> Option<Milestone<'a>> {
        let style = self.element_style(node, StyleType::Milestone)?;
        Some(Milestone {
            style,
            attributes: self.attribute_list(node, &["style"], &[]),
            span: span_of(node.range()),
        })
    }
}

/// `t[hc][rc]?\d+(-\d+)?`, the pattern `usx.rnc` gives `cell@style` and the
/// writer spells from a cell: header or not, the alignment letter, the first
/// column and, for a span, the last. `None` for anything else.
fn parse_cell_style(style: &str) -> Option<(bool, Alignment, u8, u8)> {
    let rest = style.strip_prefix('t')?;
    let (header, rest) = match rest.as_bytes().first()? {
        b'h' => (true, &rest[1..]),
        b'c' => (false, &rest[1..]),
        _ => return None,
    };
    let (alignment, rest) = match rest.as_bytes().first()? {
        b'r' => (Alignment::End, &rest[1..]),
        b'c' => (Alignment::Center, &rest[1..]),
        _ => (Alignment::Start, rest),
    };
    let (first, last) = match rest.split_once('-') {
        Some((first, last)) => (first, Some(last)),
        None => (rest, None),
    };
    let is_number = |digits: &str| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit());
    if !is_number(first) || !last.is_none_or(is_number) {
        return None;
    }
    let column: u8 = first.parse().ok()?;
    let last: u8 = match last {
        Some(last) => last.parse().ok()?,
        None => column,
    };
    // The parser's reading of the same marker (`TableCellInfo`): column 0 is
    // kept for `unexpected-table-column` to report, and a span that ends
    // before it starts is one column (ticket 49).
    let colspan = if last < column { 1 } else { last - column + 1 };
    Some((header, alignment, column, colspan))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cell_style_is_header_alignment_and_columns() {
        assert_eq!(
            parse_cell_style("tc1"),
            Some((false, Alignment::Start, 1, 1))
        );
        assert_eq!(
            parse_cell_style("th2"),
            Some((true, Alignment::Start, 2, 1))
        );
        assert_eq!(
            parse_cell_style("tcr3"),
            Some((false, Alignment::End, 3, 1))
        );
        assert_eq!(
            parse_cell_style("thc1-3"),
            Some((true, Alignment::Center, 1, 3))
        );
        for style in ["tc", "tx1", "tc1-", "p", "tc1a"] {
            assert_eq!(parse_cell_style(style), None, "{style}");
        }
        // As the parser reads the marker (ticket 49, found by the
        // `usx_roundtrip` fuzz target on `\thc0`): column 0 is a column
        // (`unexpected-table-column` is `usfm_semantic`'s to report), and a
        // span that ends before it starts is one column.
        assert_eq!(
            parse_cell_style("thc0"),
            Some((true, Alignment::Center, 0, 1))
        );
        assert_eq!(
            parse_cell_style("tc3-2"),
            Some((false, Alignment::Start, 3, 1))
        );
    }

    #[test]
    fn whitespace_collapses_and_stays_borrowed_when_it_can() {
        assert!(matches!(
            collapse_whitespace(Cow::Borrowed("a b")),
            Cow::Borrowed("a b")
        ));
        assert_eq!(collapse_whitespace(Cow::Borrowed("a\n    b")), "a b");
        assert_eq!(collapse_whitespace(Cow::Borrowed("  a  ")), " a ");
        // Rule 2: only ASCII whitespace.
        assert_eq!(
            collapse_whitespace(Cow::Borrowed("a\u{a0}\u{a0}b")),
            "a\u{a0}\u{a0}b"
        );
    }

    #[test]
    fn an_error_position_is_a_byte_offset() {
        let source = "ab\ncdé\nf";
        assert_eq!(offset_of(source, 1, 1), 0);
        assert_eq!(offset_of(source, 2, 1), 3);
        assert_eq!(offset_of(source, 2, 4), 7);
        assert_eq!(offset_of(source, 3, 2), 9);
        assert_eq!(offset_of(source, 9, 9), source.len() as u32);
    }
}
