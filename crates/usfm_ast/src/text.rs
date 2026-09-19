//! Plain-text extraction: the words of a document, a node, or a verse,
//! without markup. Every pipeline needs this (search, concordances, audio
//! alignment, an LLM prompt), and every one used to reinvent it.
//!
//! [`PlainText`] is a [`Visit`] with a few knobs: whether note text is
//! included (off by default: a footnote is not the verse), which character
//! styles contribute their text (all by default; drop `\fig` captions or
//! `\rq` references with [`PlainText::keep_char`]), which paragraphs do
//! (all by default), and what separates blocks (a space by default).
//!
//! Attributes, verse and chapter numbers, callers and categories are never
//! text. Trailing whitespace at a block boundary is dropped, as the parser
//! does at paragraph level, so a separator never follows a space; and a
//! dropped style never leaves a double space behind.

use usfm_style::{StyleRule, StyleSheet};

use crate::visit::{Visit, walk_char, walk_note, walk_para, walk_table_cell};
use crate::{Char, Document, NodePath, NodeRef, Note, Para, StyleId, TableCell, Text};

pub struct PlainText<'s> {
    style_sheet: &'s StyleSheet,
    notes: bool,
    keep_char: Box<dyn Fn(&StyleRule) -> bool + 's>,
    keep_para: Box<dyn Fn(&StyleRule) -> bool + 's>,
    separator: String,
    out: String,
    /// A block boundary was crossed since the last text; the separator goes
    /// in before the next text, if there is any.
    at_boundary: bool,
}

impl<'s> PlainText<'s> {
    /// An extractor with the defaults above. The sheet must be the
    /// document's own (`document.style_sheet()`, plan D3).
    pub fn new(style_sheet: &'s StyleSheet) -> Self {
        Self {
            style_sheet,
            notes: false,
            keep_char: Box::new(|_| true),
            keep_para: Box::new(|_| true),
            separator: " ".to_string(),
            out: String::new(),
            at_boundary: false,
        }
    }

    /// Include the text of notes, set off from the surrounding text by the
    /// block separator.
    pub fn with_notes(mut self, notes: bool) -> Self {
        self.notes = notes;
        self
    }

    /// Which character styles contribute their text. A style that does not
    /// is skipped with everything inside it.
    pub fn keep_char(mut self, keep: impl Fn(&StyleRule) -> bool + 's) -> Self {
        self.keep_char = Box::new(keep);
        self
    }

    /// Which paragraphs contribute their text.
    pub fn keep_para(mut self, keep: impl Fn(&StyleRule) -> bool + 's) -> Self {
        self.keep_para = Box::new(keep);
        self
    }

    /// What goes between blocks (paragraphs, table cells, included notes).
    pub fn block_separator(mut self, separator: &str) -> Self {
        self.separator = separator.to_string();
        self
    }

    /// The text of a whole document.
    pub fn of_document(&mut self, document: &Document<'_>) -> String {
        self.reset();
        self.visit_document(document);
        self.finish()
    }

    /// The text of one node and everything in it.
    pub fn of_node(&mut self, node: NodeRef<'_>) -> String {
        self.reset();
        node.visit_with(self);
        self.finish()
    }

    /// The text of nodes with their paths, as `usfm_semantic`'s
    /// `VerseRef::nodes` yields them: a change of block (the first path
    /// element) is a boundary.
    pub fn of_located<'a>(
        &mut self,
        nodes: impl IntoIterator<Item = (NodePath, NodeRef<'a>)>,
    ) -> String {
        self.reset();
        let mut block: Option<usize> = None;
        for (path, node) in nodes {
            let first = path.first().copied();
            if block.is_some() && block != first {
                self.boundary();
            }
            block = first;
            node.visit_with(self);
        }
        self.finish()
    }

    fn reset(&mut self) {
        self.out.clear();
        self.at_boundary = false;
    }

    fn finish(&mut self) -> String {
        self.trim_end();
        std::mem::take(&mut self.out)
    }

    fn rule(&self, style: StyleId) -> &'s StyleRule {
        self.style_sheet.get_rule(style.index())
    }

    fn boundary(&mut self) {
        if !self.out.is_empty() {
            self.at_boundary = true;
        }
    }

    fn trim_end(&mut self) {
        let trimmed = self
            .out
            .trim_end_matches(|c: char| c.is_ascii_whitespace())
            .len();
        self.out.truncate(trimmed);
    }

    fn push(&mut self, text: &str) {
        if self.at_boundary {
            self.trim_end();
            self.out.push_str(&self.separator);
            self.at_boundary = false;
        }
        // A dropped style or note leaves the spaces on both sides of it;
        // keep one, as the parser does for a run of whitespace (rule 1).
        let text = if self.out.ends_with(|c: char| c.is_ascii_whitespace()) {
            text.trim_start_matches(|c: char| c.is_ascii_whitespace())
        } else {
            text
        };
        self.out.push_str(text);
    }
}

impl Visit for PlainText<'_> {
    fn visit_text(&mut self, text: &Text<'_>) {
        self.push(&text.content);
    }

    fn visit_para(&mut self, para: &Para<'_>) {
        if (self.keep_para)(self.rule(para.style)) {
            self.boundary();
            walk_para(self, para);
            self.boundary();
        }
    }

    fn visit_table_cell(&mut self, cell: &TableCell<'_>) {
        self.boundary();
        walk_table_cell(self, cell);
        self.boundary();
    }

    fn visit_char(&mut self, char: &Char<'_>) {
        if (self.keep_char)(self.rule(char.style)) {
            walk_char(self, char);
        }
    }

    fn visit_note(&mut self, note: &Note<'_>) {
        if self.notes {
            self.boundary();
            walk_note(self, note);
            self.boundary();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::sample_document;

    #[test]
    fn document_text_skips_notes_and_joins_blocks() {
        let document = sample_document();
        let text = PlainText::new(document.style_sheet()).of_document(&document);
        assert_eq!(
            text,
            "In the beginning God created the heavens and the earth Aside Reuben"
        );
    }

    #[test]
    fn notes_and_character_styles_are_configurable() {
        let document = sample_document();
        let text = PlainText::new(document.style_sheet())
            .with_notes(true)
            .keep_char(|rule| rule.marker != "nd")
            .block_separator("\n")
            .of_document(&document);
        assert_eq!(
            text,
            "In the beginning created the heavens\na note\nand the earth\nAside\nReuben"
        );
    }

    #[test]
    fn paragraphs_are_configurable() {
        let document = sample_document();
        let text = PlainText::new(document.style_sheet())
            .keep_para(|rule| rule.marker == "q1")
            .of_document(&document);
        assert_eq!(text, "and the earth Reuben");
    }

    #[test]
    fn a_single_node() {
        let document = sample_document();
        let text = PlainText::new(document.style_sheet()).of_node(NodeRef::from_block(&document.blocks[3]));
        assert_eq!(text, "and the earth");
    }
}
