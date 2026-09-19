//! The AST as a USX tree.
//!
//! The walk is a [`usfm_ast::visit::Visit`] implementation (ticket 13), not
//! the old `ToUsx` trait that threaded `usfm_parser`'s `Context` through every
//! node. All the state the USX output needs is private to [`UsxWriter`]: the
//! book code, the open chapter and verse (for `sid`/`eid`/`vid`), the
//! `include_vid` flag and the stylesheet the document carries. Note numbering
//! and the counters the HTML writer keeps are not among them, so nothing here
//! reaches back into the parser.
//!
//! Building a tree with a visitor, whose methods return nothing, means holding
//! the children of the element being built: [`UsxWriter::children`] is that
//! vector, and [`UsxWriter::element`] swaps an empty one in for the length of
//! a node's own children before putting the parent's back with the finished
//! element appended. So a node is written once, into the vector its parent
//! keeps, and an element costs the one allocation the old
//! `children: …collect()` cost.
//!
//! A node whose USFM attributes become XML attributes (`\w`, a milestone, a
//! `\periph` line) overrides its `visit_*` and walks its children itself
//! rather than calling the generated walker, which would visit the attribute
//! list as if it were content.

use usfm_ast::visit::{
    Visit, walk_note, walk_para, walk_sidebar, walk_table, walk_table_cell, walk_table_row,
};
use usfm_ast::{
    Alignment, Block, Book, ChapterEnd, ChapterStart, Char, Document, Milestone, Note, NumberList,
    OptBreak, Para, Periph, Sidebar, StyleId, Table, TableCell, TableRow, Text, VerseEnd,
    VerseStart, default_attribute_name, is_valid_attribute_name,
};
use usfm_style::{StyleRule, StyleSheet};
use xml::attribute::OwnedAttribute;
use xml::name::OwnedName;
use xml::namespace::Namespace;

use crate::xml_document::{XmlElement, XmlNode};

/// The USX version emitted when the source has no `\usfm` marker.
pub const DEFAULT_USX_VERSION: &str = "3.0";

/// What the caller can vary about the output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsxOptions {
    /// Whether a `<para>` or `<table>` that continues a verse from an earlier
    /// paragraph carries the `vid` attribute naming it. The conformance
    /// harness turns this off for the reference files that predate `vid`.
    pub include_vid: bool,
}

impl Default for UsxOptions {
    fn default() -> Self {
        Self { include_vid: true }
    }
}

/// The document as a USX tree, resolved against the document's own stylesheet.
pub fn to_usx_node(document: &Document) -> XmlNode {
    to_usx_node_with_options(document, UsxOptions::default())
}

/// [`to_usx_node`] with the output options spelled out.
pub fn to_usx_node_with_options(document: &Document, options: UsxOptions) -> XmlNode {
    let mut writer = UsxWriter::new(document.style_sheet(), options);
    writer.visit_document(document);
    writer.finish()
}

/// The document as USX text.
pub fn to_usx_string(document: &Document) -> String {
    format!("{}\n", to_usx_node(document))
}

/// One XML attribute. `name` is generic so a name that is already a `String` —
/// the one `visit_char` works out per attribute — moves in instead of being
/// copied into a second allocation.
fn attribute(name: impl Into<String>, value: String) -> OwnedAttribute {
    OwnedAttribute::new(OwnedName::local(name), value)
}

/// Whether `name` can be added to `attrs`: a valid XML name that is not
/// already there. XML has no repeated attribute, so a name given twice
/// (`duplicate-attribute`) keeps its first value.
fn is_writable(attrs: &[OwnedAttribute], name: &str) -> bool {
    is_valid_attribute_name(name) && !attrs.iter().any(|attr| attr.name.local_name == name)
}

/// The walk, and everything it has to remember while walking.
struct UsxWriter<'a> {
    /// The sheet the document owns, which is the base sheet plus every style
    /// the parser derived (hardening plan D3).
    style_sheet: &'a StyleSheet,
    book_code: Option<usfm_ast::BookCode>,
    chapter_number: Option<usize>,
    /// The verse that is open, which a `<para>` or `<table>` after the one it
    /// started in names in its `vid`.
    verse_number: Option<NumberList>,
    include_vid: bool,
    /// The children of the element being built. [`UsxWriter::element`] swaps
    /// an empty vector in for the duration of a node's own children and swaps
    /// the parent's back afterwards, so a child node is written once, into the
    /// vector the finished element keeps — the one write and the one
    /// allocation per element that `children: …collect()` used to cost.
    /// When the walk is over it holds the root, alone.
    children: Vec<XmlNode>,
}

impl<'a> UsxWriter<'a> {
    fn new(style_sheet: &'a StyleSheet, options: UsxOptions) -> Self {
        Self {
            style_sheet,
            book_code: None,
            chapter_number: None,
            verse_number: None,
            include_vid: options.include_vid,
            children: Vec::with_capacity(1),
        }
    }

    /// The root element, once the walk is done.
    fn finish(mut self) -> XmlNode {
        self.children
            .pop()
            .expect("the walk builds exactly one root element")
    }

    fn rule(&self, style: StyleId) -> &'a StyleRule {
        self.style_sheet.get_rule(style.index())
    }

    /// The marker name for a node's style, e.g. `"p"`.
    fn marker(&self, style: StyleId) -> &'a str {
        &self.rule(style).marker
    }

    /// Add a finished node to the element being built.
    fn push(&mut self, node: XmlNode) {
        self.children.push(node);
    }

    /// A leaf element: no children, so nothing to walk.
    fn push_element(&mut self, name: &str, attributes: Vec<OwnedAttribute>) {
        self.push(XmlNode::Element(XmlElement {
            name: OwnedName::local(name),
            attributes,
            namespace: Namespace::empty(),
            children: vec![],
        }));
    }

    /// An element whose children `walk` produces. `capacity` is the node's own
    /// child count, so the vector `walk` fills is allocated once and at the
    /// right size; the parent's children are set aside meanwhile and restored
    /// with the finished element appended.
    fn element(
        &mut self,
        name: &str,
        attributes: Vec<OwnedAttribute>,
        capacity: usize,
        walk: impl FnOnce(&mut Self),
    ) {
        let parent = std::mem::replace(&mut self.children, Vec::with_capacity(capacity));
        walk(self);
        let children = std::mem::replace(&mut self.children, parent);
        self.push(XmlNode::Element(XmlElement {
            name: OwnedName::local(name),
            attributes,
            namespace: Namespace::empty(),
            children,
        }));
    }

    /// `BOOK C:V` for the verse that is open, as `sid`, `eid` and `vid` spell
    /// a reference. A missing book or chapter is written the way the whole
    /// walk treats missing structure: `OTH` and `0`, never a panic.
    fn verse_reference(&self, number: &NumberList) -> String {
        format!(
            "{} {}:{}",
            self.book_code.unwrap_or(usfm_ast::BookCode::Oth),
            self.chapter_number.unwrap_or(0),
            number
        )
    }

    fn chapter_reference(&self, number: usize) -> String {
        format!(
            "{} {}",
            self.book_code.unwrap_or(usfm_ast::BookCode::Oth),
            number
        )
    }

    /// The `vid` a `<para>` or `<table>` carries when it continues a verse
    /// that started in an earlier one.
    fn vid(&self) -> Option<OwnedAttribute> {
        if !self.include_vid {
            return None;
        }
        let verse_number = self.verse_number.as_ref()?;
        Some(attribute("vid", self.verse_reference(verse_number)))
    }

    /// The USFM attribute list of `\w`, a milestone or a `\periph` line as XML
    /// attributes, appended to `attrs`. An unnamed value is the marker's
    /// default attribute (`\qt-s |Speaker\*` is `who`); a name USX cannot
    /// carry — an empty one, a malformed one (`malformed-attribute-name`) or a
    /// repeat (`duplicate-attribute`) — is dropped rather than written as
    /// broken XML.
    fn push_attributes(
        &self,
        attrs: &mut Vec<OwnedAttribute>,
        pairs: &[usfm_ast::Attribute<'_>],
        marker: &str,
    ) {
        for attr in pairs {
            let name = if attr.name.is_empty() {
                default_attribute_name(marker)
            } else {
                Some(attr.name.as_ref())
            };
            if let Some(name) = name.filter(|name| is_writable(attrs, name)) {
                attrs.push(attribute(name, attr.value.to_string()));
            }
        }
    }
}

impl Visit for UsxWriter<'_> {
    fn visit_document(&mut self, document: &Document<'_>) {
        // `\usfm 3.1` becomes the version attribute, not a paragraph.
        let version = document
            .usfm_version()
            .unwrap_or_else(|| DEFAULT_USX_VERSION.to_string());
        let style_sheet = self.style_sheet;
        let blocks = document.blocks.iter().filter(
            move |block| !matches!(block, Block::Para(para) if para.is_usfm_version(style_sheet)),
        );
        self.element(
            "usx",
            vec![attribute("version", version)],
            document.blocks.len(),
            |writer| {
                for block in blocks {
                    writer.visit_block(block);
                }
            },
        );
    }

    fn visit_book(&mut self, book: &Book<'_>) {
        self.book_code = Some(book.code);
        let attrs = vec![
            attribute("code", book.code.to_string()),
            attribute("style", "id".to_string()),
        ];
        let children = if book.description.is_empty() {
            vec![]
        } else {
            vec![XmlNode::Text(book.description.to_string())]
        };
        self.push(XmlNode::Element(XmlElement {
            name: OwnedName::local("book"),
            attributes: attrs,
            namespace: Namespace::empty(),
            children,
        }));
    }

    fn visit_chapter_start(&mut self, chapter: &ChapterStart<'_>) {
        self.chapter_number = Some(chapter.number);
        let mut attrs = vec![
            attribute("number", chapter.number.to_string()),
            attribute("style", "c".to_string()),
        ];
        if let Some(alt) = &chapter.alt_number {
            attrs.push(attribute("altnumber", alt.to_string()));
        }
        if let Some(pub_number) = &chapter.pub_number {
            attrs.push(attribute("pubnumber", pub_number.to_string()));
        }
        attrs.push(attribute("sid", self.chapter_reference(chapter.number)));
        self.push_element("chapter", attrs);
    }

    fn visit_chapter_end(&mut self, chapter: &ChapterEnd) {
        self.chapter_number = None;
        let eid = attribute("eid", self.chapter_reference(chapter.number));
        self.push_element("chapter", vec![eid]);
    }

    fn visit_para(&mut self, para: &Para<'_>) {
        let mut attrs = vec![attribute("style", self.marker(para.style).to_string())];
        // A paragraph that continues a verse from an earlier one names it.
        attrs.extend(self.vid());
        self.element("para", attrs, para.children.len(), |writer| {
            walk_para(writer, para)
        });
    }

    fn visit_table(&mut self, table: &Table<'_>) {
        let attrs = self.vid().into_iter().collect();
        self.element("table", attrs, table.rows.len(), |writer| {
            walk_table(writer, table)
        });
    }

    fn visit_table_row(&mut self, row: &TableRow<'_>) {
        let attrs = vec![attribute("style", "tr".to_string())];
        self.element("row", attrs, row.cells.len(), |writer| {
            walk_table_row(writer, row)
        });
    }

    fn visit_table_cell(&mut self, cell: &TableCell<'_>) {
        // Build style: t + (h if header else c) + the alignment letter + column
        // number(s), which is the `t[hc][rc]?\d+(-\d+)?` pattern USX gives
        // `cell@style`. The letter is part of the style, not only of `align`:
        // tcdocs writes `<cell style="tcr3" align="end">`, so `\tcc3` is
        // `<cell style="tcc3" align="center">`.
        let header_char = if cell.header { 'h' } else { 'c' };
        let align_char = match cell.alignment {
            Alignment::Start => "",
            Alignment::Center => "c",
            Alignment::End => "r",
        };
        let column_str = if cell.colspan > 1 {
            format!("{}-{}", cell.column, cell.column + cell.colspan - 1)
        } else {
            cell.column.to_string()
        };
        let attrs = vec![
            attribute("style", format!("t{header_char}{align_char}{column_str}")),
            attribute("align", cell.alignment.to_string()),
        ];
        self.element("cell", attrs, cell.children.len(), |writer| {
            walk_table_cell(writer, cell)
        });
    }

    fn visit_sidebar(&mut self, sidebar: &Sidebar<'_>) {
        let mut attrs = vec![attribute("style", self.marker(sidebar.style).to_string())];
        if let Some(category) = &sidebar.category {
            attrs.push(attribute("category", category.content.to_string()));
        }
        // A sidebar is outside the verse flow: its paragraphs carry no `vid`,
        // but the verse is still open for the paragraphs after `\esbe`.
        let verse_number = self.verse_number.take();
        self.element("sidebar", attrs, sidebar.blocks.len(), |writer| {
            walk_sidebar(writer, sidebar)
        });
        self.verse_number = verse_number;
    }

    fn visit_periph(&mut self, periph: &Periph<'_>) {
        let mut attrs = vec![];
        if let Some(title) = &periph.title {
            attrs.push(attribute("alt", title.content.to_string()));
        }
        if let Some(attributes) = &periph.attributes {
            self.push_attributes(&mut attrs, &attributes.pairs, self.marker(periph.style));
        }
        // The attribute list is written above, so the blocks are walked here
        // rather than through `walk_periph`, which would visit it as content.
        self.element("periph", attrs, periph.blocks.len(), |writer| {
            for block in &periph.blocks {
                writer.visit_block(block);
            }
        });
    }

    fn visit_text(&mut self, text: &Text<'_>) {
        self.push(XmlNode::Text(text.content.to_string()));
    }

    fn visit_verse_start(&mut self, verse: &VerseStart<'_>) {
        self.verse_number = Some(verse.number.clone());
        let mut attrs = vec![
            attribute("number", verse.number.to_string()),
            attribute("style", "v".to_string()),
        ];
        if let Some(alt) = &verse.alt_number {
            attrs.push(attribute("altnumber", alt.to_string()));
        }
        if let Some(pub_number) = &verse.pub_number {
            attrs.push(attribute("pubnumber", pub_number.to_string()));
        }
        attrs.push(attribute("sid", self.verse_reference(&verse.number)));
        self.push_element("verse", attrs);
    }

    fn visit_verse_end(&mut self, verse: &VerseEnd) {
        self.verse_number = None;
        let eid = attribute("eid", self.verse_reference(&verse.number));
        self.push_element("verse", vec![eid]);
    }

    fn visit_char(&mut self, char: &Char<'_>) {
        let style_name = self.marker(char.style);

        let (name, mut attrs) = match style_name {
            "ref" => ("ref", vec![]),
            "fig" => ("figure", vec![attribute("style", style_name.to_string())]),
            _ => ("char", vec![attribute("style", style_name.to_string())]),
        };

        if let Some(attributes) = &char.attributes {
            for attr in &attributes.pairs {
                let name = if attr.name.is_empty() {
                    // The marker's default attribute, if it has one; `\fig`
                    // has none, so an unnamed value there is dropped.
                    default_attribute_name(style_name)
                        .unwrap_or_default()
                        .to_string()
                } else if attr.name == "src" && style_name == "fig" {
                    "file".to_string()
                } else {
                    attr.name.to_string()
                };
                // Only add a name USX can carry: an empty one (a default
                // attribute the marker does not have), a malformed one
                // (`malformed-attribute-name`) and a repeat
                // (`duplicate-attribute`) are not writable as XML.
                if is_writable(&attrs, &name) {
                    attrs.push(attribute(name, attr.value.to_string()));
                }
            }
        }

        // As in `visit_periph`: the attribute list is already written, so the
        // children are walked directly instead of through `walk_char`.
        self.element(name, attrs, char.children.len(), |writer| {
            for child in &char.children {
                writer.visit_inline(child);
            }
        });
    }

    fn visit_note(&mut self, note: &Note<'_>) {
        let mut attrs = vec![
            attribute("caller", note.caller.to_string()),
            attribute("style", self.marker(note.style).to_string()),
        ];
        if let Some(category) = &note.category {
            attrs.push(attribute("category", category.content.to_string()));
        }
        self.element("note", attrs, note.children.len(), |writer| {
            walk_note(writer, note)
        });
    }

    fn visit_milestone(&mut self, milestone: &Milestone<'_>) {
        let marker = self.marker(milestone.style);
        let mut attrs = vec![attribute("style", marker.to_string())];
        // `sid`, `eid`, `who` and the rest; milestones are self-closing `<ms>`
        // elements with no children.
        self.push_attributes(&mut attrs, milestone.pairs(), marker);
        self.push_element("ms", attrs);
    }

    fn visit_opt_break(&mut self, _opt_break: &OptBreak) {
        self.push_element("optbreak", vec![]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rule every attribute list goes through. What it rejects reaches
    /// the output as a dropped attribute rather than as broken XML; the
    /// end-to-end cases are in `usfm_parser/tests/usx_text.rs`, which needs a
    /// parser this crate deliberately does not depend on.
    #[test]
    fn an_attribute_is_written_once_and_only_under_an_xml_name() {
        let mut attrs = vec![attribute("style", "w".to_string())];
        assert!(is_writable(&attrs, "lemma"));
        attrs.push(attribute("lemma", "first".to_string()));
        // `duplicate-attribute`: XML has no repeated attribute, so the second
        // value is dropped and the first stands.
        assert!(!is_writable(&attrs, "lemma"));
        assert!(!is_writable(&attrs, "style"));
        // `malformed-attribute-name`, and the empty name a default attribute
        // leaves behind when its marker has none.
        assert!(!is_writable(&attrs, "b<c"));
        assert!(!is_writable(&attrs, ""));
    }

    /// `vid` is on unless a caller turns it off.
    #[test]
    fn vid_is_included_by_default() {
        assert!(UsxOptions::default().include_vid);
    }
}
