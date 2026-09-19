//! Read-only traversal: [`Visit`] and its walkers.
//!
//! A visitor overrides the `visit_*` methods it cares about; every default
//! calls the matching `walk_*` function, which visits the node's children.
//! Override a method and call the walker yourself to do something before or
//! after the children, or leave the walker out to prune the subtree.
//!
//! [`VisitMut`](crate::visit_mut::VisitMut) is the same traversal over
//! `&mut`. Both are generated from one description by [`define_visitor!`], so
//! they cannot drift: adding a node type is one line in that table and one
//! walker body.
//!
//! What is visited: every node in the tree, including milestones and
//! attributes (`visit_attributes` for the list, `visit_attribute` for each
//! pair, after a character style's children and for a milestone or periph).
//! Fields that are metadata rather than content are not visited and are
//! read from the node: a note's caller and category, a sidebar's category, a
//! periph's title, a verse's alternate and published numbers.
//!
//! Style resolution: a visitor that needs a node's marker or stylesheet
//! rule holds the document's sheet (plan D3: `document.style_sheet()`),
//! either from construction or captured in `visit_document` before walking.

/// Defines a visitor trait and its walkers over one kind of reference.
///
/// `$Visit` is the trait name, `$iter` the `Vec` iterator (`iter` or
/// `iter_mut`) and `$r` the reference tokens (`&` or `&mut`).
macro_rules! define_visitor {
    ($(#[$doc:meta])* $Visit:ident, $iter:ident, [$($r:tt)+]) => {
        $(#[$doc])*
        pub trait $Visit: Sized {
            fn visit_document(&mut self, document: $($r)+ Document<'_>) {
                walk_document(self, document)
            }

            fn visit_block(&mut self, block: $($r)+ Block<'_>) {
                walk_block(self, block)
            }

            fn visit_book(&mut self, book: $($r)+ Book<'_>) {
                let _ = book;
            }

            fn visit_chapter_start(&mut self, chapter: $($r)+ ChapterStart<'_>) {
                let _ = chapter;
            }

            fn visit_chapter_end(&mut self, chapter: $($r)+ ChapterEnd) {
                let _ = chapter;
            }

            fn visit_para(&mut self, para: $($r)+ Para<'_>) {
                walk_para(self, para)
            }

            fn visit_table(&mut self, table: $($r)+ Table<'_>) {
                walk_table(self, table)
            }

            fn visit_table_row(&mut self, row: $($r)+ TableRow<'_>) {
                walk_table_row(self, row)
            }

            fn visit_table_cell(&mut self, cell: $($r)+ TableCell<'_>) {
                walk_table_cell(self, cell)
            }

            fn visit_sidebar(&mut self, sidebar: $($r)+ Sidebar<'_>) {
                walk_sidebar(self, sidebar)
            }

            fn visit_periph(&mut self, periph: $($r)+ Periph<'_>) {
                walk_periph(self, periph)
            }

            fn visit_inline(&mut self, inline: $($r)+ Inline<'_>) {
                walk_inline(self, inline)
            }

            fn visit_text(&mut self, text: $($r)+ Text<'_>) {
                let _ = text;
            }

            fn visit_verse_start(&mut self, verse: $($r)+ VerseStart<'_>) {
                let _ = verse;
            }

            fn visit_verse_end(&mut self, verse: $($r)+ VerseEnd) {
                let _ = verse;
            }

            fn visit_char(&mut self, char: $($r)+ Char<'_>) {
                walk_char(self, char)
            }

            fn visit_note(&mut self, note: $($r)+ Note<'_>) {
                walk_note(self, note)
            }

            /// A milestone, whether inside a paragraph or between blocks.
            fn visit_milestone(&mut self, milestone: $($r)+ Milestone<'_>) {
                walk_milestone(self, milestone)
            }

            fn visit_opt_break(&mut self, opt_break: $($r)+ OptBreak) {
                let _ = opt_break;
            }

            fn visit_attributes(&mut self, attributes: $($r)+ Attributes<'_>) {
                walk_attributes(self, attributes)
            }

            fn visit_attribute(&mut self, attribute: $($r)+ Attribute<'_>) {
                let _ = attribute;
            }
        }

        pub fn walk_document<V: $Visit>(visitor: &mut V, document: $($r)+ Document<'_>) {
            for block in document.blocks.$iter() {
                visitor.visit_block(block);
            }
        }

        pub fn walk_block<V: $Visit>(visitor: &mut V, block: $($r)+ Block<'_>) {
            match block {
                Block::Book(book) => visitor.visit_book(book),
                Block::ChapterStart(chapter) => visitor.visit_chapter_start(chapter),
                Block::ChapterEnd(chapter) => visitor.visit_chapter_end(chapter),
                Block::Para(para) => visitor.visit_para(para),
                Block::Table(table) => visitor.visit_table(table),
                Block::Milestone(milestone) => visitor.visit_milestone(milestone),
                Block::Sidebar(sidebar) => visitor.visit_sidebar(sidebar),
                Block::Periph(periph) => visitor.visit_periph(periph),
            }
        }

        pub fn walk_para<V: $Visit>(visitor: &mut V, para: $($r)+ Para<'_>) {
            for inline in para.children.$iter() {
                visitor.visit_inline(inline);
            }
        }

        pub fn walk_table<V: $Visit>(visitor: &mut V, table: $($r)+ Table<'_>) {
            for row in table.rows.$iter() {
                visitor.visit_table_row(row);
            }
        }

        pub fn walk_table_row<V: $Visit>(visitor: &mut V, row: $($r)+ TableRow<'_>) {
            for cell in row.cells.$iter() {
                visitor.visit_table_cell(cell);
            }
        }

        pub fn walk_table_cell<V: $Visit>(visitor: &mut V, cell: $($r)+ TableCell<'_>) {
            for inline in cell.children.$iter() {
                visitor.visit_inline(inline);
            }
        }

        pub fn walk_sidebar<V: $Visit>(visitor: &mut V, sidebar: $($r)+ Sidebar<'_>) {
            for block in sidebar.blocks.$iter() {
                visitor.visit_block(block);
            }
        }

        pub fn walk_periph<V: $Visit>(visitor: &mut V, periph: $($r)+ Periph<'_>) {
            if let Some(attributes) = $($r)+ periph.attributes {
                visitor.visit_attributes(attributes);
            }
            for block in periph.blocks.$iter() {
                visitor.visit_block(block);
            }
        }

        pub fn walk_inline<V: $Visit>(visitor: &mut V, inline: $($r)+ Inline<'_>) {
            match inline {
                Inline::Text(text) => visitor.visit_text(text),
                Inline::VerseStart(verse) => visitor.visit_verse_start(verse),
                Inline::VerseEnd(verse) => visitor.visit_verse_end(verse),
                Inline::Char(char) => visitor.visit_char(char),
                Inline::Note(note) => visitor.visit_note(note),
                Inline::Milestone(milestone) => visitor.visit_milestone(milestone),
                Inline::OptBreak(opt_break) => visitor.visit_opt_break(opt_break),
            }
        }

        /// Children first, then the attributes, which is their source order.
        pub fn walk_char<V: $Visit>(visitor: &mut V, char: $($r)+ Char<'_>) {
            for inline in char.children.$iter() {
                visitor.visit_inline(inline);
            }
            if let Some(attributes) = $($r)+ char.attributes {
                visitor.visit_attributes(attributes);
            }
        }

        pub fn walk_note<V: $Visit>(visitor: &mut V, note: $($r)+ Note<'_>) {
            for inline in note.children.$iter() {
                visitor.visit_inline(inline);
            }
        }

        pub fn walk_milestone<V: $Visit>(visitor: &mut V, milestone: $($r)+ Milestone<'_>) {
            visitor.visit_attributes($($r)+ milestone.attributes);
        }

        pub fn walk_attributes<V: $Visit>(visitor: &mut V, attributes: $($r)+ Attributes<'_>) {
            for attribute in attributes.pairs.$iter() {
                visitor.visit_attribute(attribute);
            }
        }
    };
}

use crate::{
    Attribute, Attributes, Block, Book, ChapterEnd, ChapterStart, Char, Document, Inline,
    Milestone, Note, OptBreak, Para, Periph, Sidebar, Table, TableCell, TableRow, Text, VerseEnd,
    VerseStart,
};

define_visitor!(
    /// A read-only visitor over a [`Document`]. See the module docs.
    Visit,
    iter,
    [&]
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::sample_document;

    /// Counts what it sees, and checks that pruning works: nothing inside a
    /// note is counted.
    #[derive(Default)]
    struct Counter {
        paras: usize,
        texts: usize,
        attributes: usize,
        verse_starts: usize,
    }

    impl Visit for Counter {
        fn visit_para(&mut self, para: &Para<'_>) {
            self.paras += 1;
            walk_para(self, para);
        }

        fn visit_text(&mut self, _text: &Text<'_>) {
            self.texts += 1;
        }

        fn visit_attribute(&mut self, _attribute: &Attribute<'_>) {
            self.attributes += 1;
        }

        fn visit_verse_start(&mut self, _verse: &VerseStart<'_>) {
            self.verse_starts += 1;
        }

        fn visit_note(&mut self, _note: &Note<'_>) {}
    }

    #[test]
    fn visits_every_node_and_prunes_where_told() {
        let document = sample_document();
        let mut counter = Counter::default();
        counter.visit_document(&document);
        // Two in the chapter and one in the sidebar.
        assert_eq!(counter.paras, 3);
        // "In the beginning ", "God", " ", "created", " the heavens", "and ",
        // "the earth", the sidebar's "Aside", and the cell's "Reuben"; the
        // note's text is pruned.
        assert_eq!(counter.texts, 9);
        // `\w created|lemma="create" strong="H1254"\w*` and the milestone's
        // `who`.
        assert_eq!(counter.attributes, 3);
        assert_eq!(counter.verse_starts, 2);
    }
}
