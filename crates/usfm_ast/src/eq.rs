//! Comparing two documents that were parsed from different text:
//! [`eq_ignoring_spans`].
//!
//! `Document`'s own `PartialEq` compares its `blocks` and nothing else (the
//! stylesheet is deliberately left out, see the note on that impl), and every
//! node's `PartialEq` is derived — so it compares the node's `span` too, and
//! its `StyleId` as a raw index. Both are wrong for the one question a
//! round-trip test asks: *is this the same document, written differently?*
//!
//! - A span is where a node was read from. Two spellings of the same document
//!   put the same node at different offsets, and a synthesized node carries
//!   [`SPAN`](crate::SPAN) whatever it was synthesized from.
//! - A [`StyleId`] is an index into *its own* document's sheet. The base sheet
//!   is the same for two parses with the same stylesheet, so most ids do
//!   match; the ones that need not are the styles the parser **derives** while
//!   parsing (a milestone form such as `\k-s`, an unknown `\zaln-s`), which
//!   are appended in the order they are first seen. Drop or reorder one and
//!   every derived id after it shifts. So this compares the marker each id
//!   resolves to, through each document's own sheet, which is what a
//!   serializer writes and the only thing an id means outside its document.
//!
//! Everything else is compared by value, including the fields the visitors
//! treat as metadata (a note's caller and category, a periph's title).
//!
//! # Why this and not a `Fold`-based `strip_spans`
//!
//! A `strip_spans` returning a rebuilt `Document` was the other option (the
//! ticket offers both). This one is smaller *and* stronger: it needs no new
//! tree-building code, it allocates nothing, it does not need `Block` and
//! `Inline` to become `Clone` — they are not, and making them so would be a
//! real change to the AST's surface — and it is the only one of the two that
//! can answer the `StyleId` question above, because a stripped tree still
//! carries the raw indices and is still compared with the derived `PartialEq`.

use crate::{
    Attributes, Block, Book, ChapterEnd, ChapterStart, Char, Document, Inline, Milestone, Note,
    OptBreak, Para, Periph, Sidebar, StyleId, Table, TableCell, TableRow, Text, VerseEnd,
    VerseStart,
};

/// Whether two documents hold the same tree, comparing styles by marker name
/// and ignoring every `span`.
///
/// ```
/// # use usfm_ast::{Document, eq_ignoring_spans};
/// # fn show(first: &Document<'_>, second: &Document<'_>) {
/// assert!(eq_ignoring_spans(first, second));
/// # }
/// ```
pub fn eq_ignoring_spans(left: &Document<'_>, right: &Document<'_>) -> bool {
    Compare { left, right }.documents()
}

/// The two documents, so a style can be resolved against the sheet of the one
/// it came from.
struct Compare<'l, 'r> {
    left: &'l Document<'l>,
    right: &'r Document<'r>,
}

impl Compare<'_, '_> {
    fn documents(&self) -> bool {
        self.blocks(&self.left.blocks, &self.right.blocks)
    }

    /// The markers two ids resolve to, each through its own document's sheet.
    fn style(&self, left: StyleId, right: StyleId) -> bool {
        self.left.marker(left) == self.right.marker(right)
    }

    fn blocks(&self, left: &[Block<'_>], right: &[Block<'_>]) -> bool {
        left.len() == right.len()
            && left
                .iter()
                .zip(right)
                .all(|(left, right)| self.block(left, right))
    }

    fn block(&self, left: &Block<'_>, right: &Block<'_>) -> bool {
        match (left, right) {
            (Block::Book(left), Block::Book(right)) => self.book(left, right),
            (Block::ChapterStart(left), Block::ChapterStart(right)) => {
                self.chapter_start(left, right)
            }
            (Block::ChapterEnd(left), Block::ChapterEnd(right)) => self.chapter_end(left, right),
            (Block::Para(left), Block::Para(right)) => self.para(left, right),
            (Block::Table(left), Block::Table(right)) => self.table(left, right),
            (Block::Milestone(left), Block::Milestone(right)) => self.milestone(left, right),
            (Block::Sidebar(left), Block::Sidebar(right)) => self.sidebar(left, right),
            (Block::Periph(left), Block::Periph(right)) => self.periph(left, right),
            _ => false,
        }
    }

    fn book(&self, left: &Book<'_>, right: &Book<'_>) -> bool {
        left.code == right.code && left.description == right.description
    }

    fn chapter_start(&self, left: &ChapterStart<'_>, right: &ChapterStart<'_>) -> bool {
        left.number == right.number
            && left.alt_number == right.alt_number
            && left.pub_number == right.pub_number
    }

    fn chapter_end(&self, left: &ChapterEnd, right: &ChapterEnd) -> bool {
        left.number == right.number
    }

    fn para(&self, left: &Para<'_>, right: &Para<'_>) -> bool {
        self.style(left.style, right.style) && self.inlines(&left.children, &right.children)
    }

    fn table(&self, left: &Table<'_>, right: &Table<'_>) -> bool {
        left.rows.len() == right.rows.len()
            && left
                .rows
                .iter()
                .zip(&right.rows)
                .all(|(left, right)| self.row(left, right))
    }

    fn row(&self, left: &TableRow<'_>, right: &TableRow<'_>) -> bool {
        left.cells.len() == right.cells.len()
            && left
                .cells
                .iter()
                .zip(&right.cells)
                .all(|(left, right)| self.cell(left, right))
    }

    fn cell(&self, left: &TableCell<'_>, right: &TableCell<'_>) -> bool {
        left.header == right.header
            && left.alignment == right.alignment
            && left.column == right.column
            && left.colspan == right.colspan
            && self.inlines(&left.children, &right.children)
    }

    fn sidebar(&self, left: &Sidebar<'_>, right: &Sidebar<'_>) -> bool {
        self.style(left.style, right.style)
            && Self::option_text(&left.category, &right.category)
            && self.blocks(&left.blocks, &right.blocks)
    }

    fn periph(&self, left: &Periph<'_>, right: &Periph<'_>) -> bool {
        self.style(left.style, right.style)
            && Self::option_text(&left.title, &right.title)
            && Self::option_attributes(&left.attributes, &right.attributes)
            && self.blocks(&left.blocks, &right.blocks)
    }

    fn inlines(&self, left: &[Inline<'_>], right: &[Inline<'_>]) -> bool {
        left.len() == right.len()
            && left
                .iter()
                .zip(right)
                .all(|(left, right)| self.inline(left, right))
    }

    fn inline(&self, left: &Inline<'_>, right: &Inline<'_>) -> bool {
        match (left, right) {
            (Inline::Text(left), Inline::Text(right)) => left.content == right.content,
            (Inline::VerseStart(left), Inline::VerseStart(right)) => {
                self.verse_start(left, right)
            }
            (Inline::VerseEnd(left), Inline::VerseEnd(right)) => self.verse_end(left, right),
            (Inline::Char(left), Inline::Char(right)) => self.char(left, right),
            (Inline::Note(left), Inline::Note(right)) => self.note(left, right),
            (Inline::Milestone(left), Inline::Milestone(right)) => self.milestone(left, right),
            (Inline::OptBreak(left), Inline::OptBreak(right)) => Self::opt_break(left, right),
            _ => false,
        }
    }

    fn verse_start(&self, left: &VerseStart<'_>, right: &VerseStart<'_>) -> bool {
        left.number == right.number
            && left.alt_number == right.alt_number
            && left.pub_number == right.pub_number
    }

    fn verse_end(&self, left: &VerseEnd, right: &VerseEnd) -> bool {
        left.number == right.number
    }

    fn char(&self, left: &Char<'_>, right: &Char<'_>) -> bool {
        self.style(left.style, right.style)
            && self.inlines(&left.children, &right.children)
            && Self::option_attributes(&left.attributes, &right.attributes)
    }

    fn note(&self, left: &Note<'_>, right: &Note<'_>) -> bool {
        self.style(left.style, right.style)
            && left.caller == right.caller
            && Self::option_text(&left.category, &right.category)
            && self.inlines(&left.children, &right.children)
    }

    fn milestone(&self, left: &Milestone<'_>, right: &Milestone<'_>) -> bool {
        self.style(left.style, right.style)
            && Self::option_attributes(&left.attributes, &right.attributes)
    }

    /// An optional break has nothing but a span, so two of them are equal.
    fn opt_break(_left: &OptBreak, _right: &OptBreak) -> bool {
        true
    }

    /// Only the content: a `Text` used as metadata (a category, a title)
    /// carries the span of the marker it was lifted out of.
    fn option_text(left: &Option<Text<'_>>, right: &Option<Text<'_>>) -> bool {
        match (left, right) {
            (Some(left), Some(right)) => left.content == right.content,
            (None, None) => true,
            _ => false,
        }
    }

    /// `None` (no `|` at all) and `Some` with no pairs (a bare `|`) are
    /// different documents, so the `Option` is compared before the pairs.
    fn option_attributes(left: &Option<Attributes<'_>>, right: &Option<Attributes<'_>>) -> bool {
        match (left, right) {
            (Some(left), Some(right)) => {
                left.pairs.len() == right.pairs.len()
                    && left
                        .pairs
                        .iter()
                        .zip(&right.pairs)
                        .all(|(left, right)| left.name == right.name && left.value == right.value)
            }
            (None, None) => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use usfm_style::{StyleRule, StyleSheet, StyleType, TextProperties, TextType};

    use super::*;
    use crate::{SPAN, Span, test_fixtures::sample_document};

    fn rule(marker: &str, style_type: StyleType) -> StyleRule {
        StyleRule {
            marker: marker.to_string(),
            name: None,
            description: None,
            style_type,
            text_type: TextType::Other,
            text_properties: TextProperties::default(),
            nest: false,
            occurs_under: vec![],
        }
    }

    /// One paragraph holding one text run, with `marker` at index `index` of a
    /// sheet padded so that the same marker can sit at two different indices.
    fn one_para(index: usize, marker: &str, text: &str, span: Span) -> Document<'static> {
        let mut rules: Vec<StyleRule> = (0..index)
            .map(|filler| rule(&format!("z{filler}"), StyleType::Milestone))
            .collect();
        rules.push(rule(marker, StyleType::Paragraph));
        let para = Para {
            style: StyleId::new(index as u32),
            children: vec![Inline::Text(Text::new(text.to_string(), span))],
            span,
        };
        Document::new(vec![Block::Para(para)], Arc::new(StyleSheet::new(rules)))
    }

    #[test]
    fn a_document_equals_itself() {
        let document = sample_document();
        assert!(eq_ignoring_spans(&document, &document));
    }

    /// The point of the function: the same tree read from two different
    /// offsets is the same tree.
    #[test]
    fn spans_are_ignored() {
        let first = one_para(0, "p", "text", Span::new(0, 4));
        let second = one_para(0, "p", "text", Span::new(100, 104));
        assert!(eq_ignoring_spans(&first, &second));
        // Whereas the derived `PartialEq` on the blocks does not agree.
        assert_ne!(first.blocks, second.blocks);
    }

    /// The other half: a style is compared by the marker it resolves to, not
    /// by its index, because a parser that derived a style put it at whatever
    /// index came next.
    #[test]
    fn a_style_is_compared_by_its_marker() {
        let first = one_para(0, "q1", "text", SPAN);
        let shifted = one_para(3, "q1", "text", SPAN);
        assert!(eq_ignoring_spans(&first, &shifted));
        assert_ne!(first.blocks, shifted.blocks);

        let other_marker = one_para(0, "q2", "text", SPAN);
        assert!(!eq_ignoring_spans(&first, &other_marker));
    }

    #[test]
    fn content_still_has_to_match() {
        let first = one_para(0, "p", "text", SPAN);
        let second = one_para(0, "p", "other", SPAN);
        assert!(!eq_ignoring_spans(&first, &second));
    }

    /// A bare `|` with no pairs is not the same document as no `|` at all,
    /// which is the distinction `Milestone::attributes` exists to keep.
    #[test]
    fn an_empty_attribute_list_differs_from_none() {
        let empty = Some(Attributes {
            pairs: vec![],
            pipe: SPAN,
        });
        assert!(!Compare::option_attributes(&empty, &None));
        assert!(Compare::option_attributes(&empty, &empty.clone()));
    }
}
