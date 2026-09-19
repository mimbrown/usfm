//! Bottom-up transformation of a [`Document`] into a tree of the caller's
//! own types: [`Fold`].
//!
//! A fold is a catamorphism: the driver ([`fold_document`]) folds a node's
//! children first and hands the results to the `fold_*` method for the node,
//! so a method sees the node and its already-transformed children, and
//! returns the transformed node. This is how USFM → JSON (or any other tree
//! output) is written without a hand-rolled walk: one method per node type,
//! and no mutable state threaded through the traversal.
//!
//! Every method is required. A fold must produce something for every node,
//! and the associated types say what: a fold to one uniform value type (a
//! JSON value) sets them all to that type; a fold to a typed tree sets each
//! to its own.
//!
//! Because children are folded before their parent, state that has to be
//! captured at a node's *start* (the verse open when a paragraph begins,
//! say) is not available here. That is a job for [`Visit`](crate::visit)
//! or for a pre-pass such as [`ReferenceIndex`](crate::ReferenceIndex).

use crate::{
    Block, Book, ChapterEnd, ChapterStart, Char, Document, Inline, Milestone, Note, OptBreak, Para,
    Periph, Sidebar, Table, TableCell, TableRow, Text, VerseEnd, VerseStart,
};

pub trait Fold {
    /// What a whole document folds to.
    type Output;
    /// What a block folds to.
    type Block;
    /// What a table row folds to.
    type Row;
    /// What a table cell folds to.
    type Cell;
    /// What an inline node folds to.
    type Inline;

    fn fold_document(&mut self, document: &Document<'_>, blocks: Vec<Self::Block>) -> Self::Output;

    fn fold_book(&mut self, book: &Book<'_>) -> Self::Block;

    fn fold_chapter_start(&mut self, chapter: &ChapterStart<'_>) -> Self::Block;

    fn fold_chapter_end(&mut self, chapter: &ChapterEnd) -> Self::Block;

    fn fold_para(&mut self, para: &Para<'_>, children: Vec<Self::Inline>) -> Self::Block;

    fn fold_table(&mut self, table: &Table<'_>, rows: Vec<Self::Row>) -> Self::Block;

    fn fold_table_row(&mut self, row: &TableRow<'_>, cells: Vec<Self::Cell>) -> Self::Row;

    fn fold_table_cell(&mut self, cell: &TableCell<'_>, children: Vec<Self::Inline>)
    -> Self::Cell;

    /// A milestone between blocks (`Block::Milestone`).
    fn fold_block_milestone(&mut self, milestone: &Milestone<'_>) -> Self::Block;

    fn fold_sidebar(&mut self, sidebar: &Sidebar<'_>, blocks: Vec<Self::Block>) -> Self::Block;

    fn fold_periph(&mut self, periph: &Periph<'_>, blocks: Vec<Self::Block>) -> Self::Block;

    fn fold_text(&mut self, text: &Text<'_>) -> Self::Inline;

    fn fold_verse_start(&mut self, verse: &VerseStart<'_>) -> Self::Inline;

    fn fold_verse_end(&mut self, verse: &VerseEnd) -> Self::Inline;

    fn fold_char(&mut self, char: &Char<'_>, children: Vec<Self::Inline>) -> Self::Inline;

    fn fold_note(&mut self, note: &Note<'_>, children: Vec<Self::Inline>) -> Self::Inline;

    /// A milestone inside a paragraph (`Inline::Milestone`).
    fn fold_milestone(&mut self, milestone: &Milestone<'_>) -> Self::Inline;

    fn fold_opt_break(&mut self, opt_break: &OptBreak) -> Self::Inline;
}

/// Fold a whole document.
pub fn fold_document<F: Fold>(folder: &mut F, document: &Document<'_>) -> F::Output {
    let blocks = fold_blocks(folder, &document.blocks);
    folder.fold_document(document, blocks)
}

pub fn fold_blocks<F: Fold>(folder: &mut F, blocks: &[Block<'_>]) -> Vec<F::Block> {
    blocks.iter().map(|block| fold_block(folder, block)).collect()
}

pub fn fold_block<F: Fold>(folder: &mut F, block: &Block<'_>) -> F::Block {
    match block {
        Block::Book(book) => folder.fold_book(book),
        Block::ChapterStart(chapter) => folder.fold_chapter_start(chapter),
        Block::ChapterEnd(chapter) => folder.fold_chapter_end(chapter),
        Block::Para(para) => {
            let children = fold_inlines(folder, &para.children);
            folder.fold_para(para, children)
        }
        Block::Table(table) => {
            let rows = table
                .rows
                .iter()
                .map(|row| {
                    let cells = row
                        .cells
                        .iter()
                        .map(|cell| {
                            let children = fold_inlines(folder, &cell.children);
                            folder.fold_table_cell(cell, children)
                        })
                        .collect();
                    folder.fold_table_row(row, cells)
                })
                .collect();
            folder.fold_table(table, rows)
        }
        Block::Milestone(milestone) => folder.fold_block_milestone(milestone),
        Block::Sidebar(sidebar) => {
            let blocks = fold_blocks(folder, &sidebar.blocks);
            folder.fold_sidebar(sidebar, blocks)
        }
        Block::Periph(periph) => {
            let blocks = fold_blocks(folder, &periph.blocks);
            folder.fold_periph(periph, blocks)
        }
    }
}

pub fn fold_inlines<F: Fold>(folder: &mut F, inlines: &[Inline<'_>]) -> Vec<F::Inline> {
    inlines
        .iter()
        .map(|inline| fold_inline(folder, inline))
        .collect()
}

pub fn fold_inline<F: Fold>(folder: &mut F, inline: &Inline<'_>) -> F::Inline {
    match inline {
        Inline::Text(text) => folder.fold_text(text),
        Inline::VerseStart(verse) => folder.fold_verse_start(verse),
        Inline::VerseEnd(verse) => folder.fold_verse_end(verse),
        Inline::Char(char) => {
            let children = fold_inlines(folder, &char.children);
            folder.fold_char(char, children)
        }
        Inline::Note(note) => {
            let children = fold_inlines(folder, &note.children);
            folder.fold_note(note, children)
        }
        Inline::Milestone(milestone) => folder.fold_milestone(milestone),
        Inline::OptBreak(opt_break) => folder.fold_opt_break(opt_break),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use usfm_style::StyleSheet;

    use super::*;
    use crate::test_fixtures::sample_document;

    /// Folds to an S-expression, one uniform type for everything.
    struct Sexp {
        style_sheet: Arc<StyleSheet>,
    }

    impl Sexp {
        fn marker(&self, style: crate::StyleId) -> &str {
            &self.style_sheet.get_rule(style.index()).marker
        }

        fn list(&self, head: &str, items: Vec<String>) -> String {
            if items.is_empty() {
                format!("({head})")
            } else {
                format!("({head} {})", items.join(" "))
            }
        }
    }

    impl Fold for Sexp {
        type Output = String;
        type Block = String;
        type Row = String;
        type Cell = String;
        type Inline = String;

        fn fold_document(&mut self, _: &Document<'_>, blocks: Vec<String>) -> String {
            self.list("doc", blocks)
        }

        fn fold_book(&mut self, book: &Book<'_>) -> String {
            format!("(id {:?})", book.code)
        }

        fn fold_chapter_start(&mut self, chapter: &ChapterStart<'_>) -> String {
            format!("(c {})", chapter.number)
        }

        fn fold_chapter_end(&mut self, chapter: &ChapterEnd) -> String {
            format!("(/c {})", chapter.number)
        }

        fn fold_para(&mut self, para: &Para<'_>, children: Vec<String>) -> String {
            self.list(self.marker(para.style), children)
        }

        fn fold_table(&mut self, _: &Table<'_>, rows: Vec<String>) -> String {
            self.list("table", rows)
        }

        fn fold_table_row(&mut self, _: &TableRow<'_>, cells: Vec<String>) -> String {
            self.list("tr", cells)
        }

        fn fold_table_cell(&mut self, cell: &TableCell<'_>, children: Vec<String>) -> String {
            self.list(&format!("tc{}", cell.column), children)
        }

        fn fold_block_milestone(&mut self, milestone: &Milestone<'_>) -> String {
            format!("(ms {})", self.marker(milestone.style))
        }

        fn fold_sidebar(&mut self, _: &Sidebar<'_>, blocks: Vec<String>) -> String {
            self.list("esb", blocks)
        }

        fn fold_periph(&mut self, _: &Periph<'_>, blocks: Vec<String>) -> String {
            self.list("periph", blocks)
        }

        fn fold_text(&mut self, text: &Text<'_>) -> String {
            format!("{:?}", text.content)
        }

        fn fold_verse_start(&mut self, verse: &VerseStart<'_>) -> String {
            format!("(v {})", verse.number)
        }

        fn fold_verse_end(&mut self, verse: &VerseEnd) -> String {
            format!("(/v {})", verse.number)
        }

        fn fold_char(&mut self, char: &Char<'_>, children: Vec<String>) -> String {
            let mut items = children;
            if let Some(attributes) = &char.attributes {
                for pair in &attributes.pairs {
                    items.push(format!("{}={:?}", pair.name, pair.value));
                }
            }
            self.list(self.marker(char.style), items)
        }

        fn fold_note(&mut self, note: &Note<'_>, children: Vec<String>) -> String {
            self.list(self.marker(note.style), children)
        }

        fn fold_milestone(&mut self, milestone: &Milestone<'_>) -> String {
            let who = milestone
                .pairs()
                .iter()
                .map(|pair| format!("{}={:?}", pair.name, pair.value))
                .collect();
            self.list(&format!("ms {}", self.marker(milestone.style)), who)
        }

        fn fold_opt_break(&mut self, _: &OptBreak) -> String {
            "//".into()
        }
    }

    #[test]
    fn folds_bottom_up_into_the_callers_type() {
        let document = sample_document();
        let mut folder = Sexp {
            style_sheet: Arc::clone(document.style_sheet()),
        };
        let sexp = fold_document(&mut folder, &document);
        assert_eq!(
            sexp,
            concat!(
                "(doc (id Gen) (c 1) ",
                "(p (v 1) \"In the beginning \" (nd \"God\") \" \" (w \"created\" lemma=\"create\" strong=\"H1254\") \" the heavens\" (f (ft \"a note\")) (/v 1)) ",
                "(q1 (ms qt-s who=\"God\") \"and \" (v 2) \"the earth\" (ms qt-e) (/v 2)) ",
                "(esb (p \"Aside\")) ",
                "(table (tr (tc1 \"Reuben\"))) ",
                "(/c 1))"
            )
        );
    }
}
