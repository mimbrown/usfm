//! Mutating traversal: [`VisitMut`] and its walkers.
//!
//! The same traversal as [`visit`](crate::visit) over `&mut`, generated
//! from the same description; see that module for what is visited and how
//! to prune. This is the API for in-place rewrites (text replacement, a
//! language-server edit); building a new tree is [`Fold`](crate::fold::Fold).

use crate::{
    Attribute, Attributes, Block, Book, ChapterEnd, ChapterStart, Char, Document, Inline,
    Milestone, Note, OptBreak, Para, Periph, Sidebar, Table, TableCell, TableRow, Text, VerseEnd,
    VerseStart,
};

define_visitor!(
    /// A mutating visitor over a [`Document`]. See the module docs.
    VisitMut,
    iter_mut,
    [&mut]
);

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::*;
    use crate::test_fixtures::sample_document;
    use crate::text::PlainText;

    /// Upper-cases every text run outside notes.
    struct Shout;

    impl VisitMut for Shout {
        fn visit_text(&mut self, text: &mut Text<'_>) {
            text.content = Cow::Owned(text.content.to_uppercase());
        }

        fn visit_note(&mut self, _note: &mut Note<'_>) {}
    }

    #[test]
    fn rewrites_in_place() {
        let mut document = sample_document();
        Shout.visit_document(&mut document);
        let text = PlainText::new(document.style_sheet())
            .with_notes(true)
            .of_document(&document);
        assert_eq!(
            text,
            "IN THE BEGINNING GOD CREATED THE HEAVENS a note AND THE EARTH ASIDE REUBEN"
        );
    }
}
