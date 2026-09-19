//! `into_owned`: detach a document from the source it borrows (plan D5).
//!
//! `Document<'a>` borrows the source through `Cow`, which is what keeps
//! parsing cheap. A caller that wants to cache a parse, send it to another
//! thread, or return it from a function that owns the input needs it to stop
//! borrowing. `Cow` makes that a matter of calling `into_owned` on the
//! borrowed leaves; everything else moves.

use std::borrow::Cow;

use crate::{
    Attribute, Attributes, Block, Book, Caller, ChapterStart, Char, Document, Inline, Milestone,
    Note, Para, Periph, Sidebar, Table, TableCell, TableRow, Text, VerseStart,
};

/// A node that can be detached from the source it borrows.
///
/// Implemented for every AST type that holds a lifetime. `Document::into_owned`
/// is the entry point; the rest exists so each node states its own conversion
/// instead of one function knowing the whole tree.
pub trait IntoOwned {
    /// The same type with no borrow of the source.
    type Owned;

    fn into_owned(self) -> Self::Owned;
}

/// `Cow::into_owned` gives a `String`; this keeps it a `Cow` so field types do
/// not change between the borrowed and owned forms.
fn own(cow: Cow<'_, str>) -> Cow<'static, str> {
    Cow::Owned(cow.into_owned())
}

fn own_all<T: IntoOwned>(items: Vec<T>) -> Vec<T::Owned> {
    items.into_iter().map(IntoOwned::into_owned).collect()
}

impl<'a> Document<'a> {
    /// Detach this document from the source it borrows, so it can be cached,
    /// sent across threads, or outlive the input (plan D5).
    ///
    /// The stylesheet is shared, not copied: it is already an `Arc`.
    pub fn into_owned(self) -> Document<'static> {
        let style_sheet = self.style_sheet;
        Document::new(own_all(self.blocks), style_sheet)
    }
}

impl<'a> IntoOwned for Block<'a> {
    type Owned = Block<'static>;

    fn into_owned(self) -> Self::Owned {
        match self {
            Block::Book(book) => Block::Book(book.into_owned()),
            Block::ChapterStart(chapter) => Block::ChapterStart(chapter.into_owned()),
            Block::ChapterEnd(chapter) => Block::ChapterEnd(chapter),
            Block::Para(para) => Block::Para(para.into_owned()),
            Block::Table(table) => Block::Table(table.into_owned()),
            Block::Milestone(milestone) => Block::Milestone(milestone.into_owned()),
            Block::Sidebar(sidebar) => Block::Sidebar(sidebar.into_owned()),
            Block::Periph(periph) => Block::Periph(periph.into_owned()),
        }
    }
}

impl<'a> IntoOwned for Periph<'a> {
    type Owned = Periph<'static>;

    fn into_owned(self) -> Self::Owned {
        Periph {
            style: self.style,
            title: self.title.map(IntoOwned::into_owned),
            attributes: self.attributes.map(IntoOwned::into_owned),
            blocks: own_all(self.blocks),
            span: self.span,
        }
    }
}

impl<'a> IntoOwned for Sidebar<'a> {
    type Owned = Sidebar<'static>;

    fn into_owned(self) -> Self::Owned {
        Sidebar {
            style: self.style,
            category: self.category.map(IntoOwned::into_owned),
            blocks: own_all(self.blocks),
            span: self.span,
        }
    }
}

impl<'a> IntoOwned for Inline<'a> {
    type Owned = Inline<'static>;

    fn into_owned(self) -> Self::Owned {
        match self {
            Inline::Text(text) => Inline::Text(text.into_owned()),
            Inline::VerseStart(verse) => Inline::VerseStart(verse.into_owned()),
            Inline::VerseEnd(verse) => Inline::VerseEnd(verse),
            Inline::Char(char) => Inline::Char(char.into_owned()),
            Inline::Note(note) => Inline::Note(note.into_owned()),
            Inline::Milestone(milestone) => Inline::Milestone(milestone.into_owned()),
            Inline::OptBreak(opt_break) => Inline::OptBreak(opt_break),
        }
    }
}

impl<'a> IntoOwned for Text<'a> {
    type Owned = Text<'static>;

    fn into_owned(self) -> Self::Owned {
        Text {
            content: own(self.content),
            span: self.span,
        }
    }
}

impl<'a> IntoOwned for Book<'a> {
    type Owned = Book<'static>;

    fn into_owned(self) -> Self::Owned {
        Book {
            code: self.code,
            description: own(self.description),
            span: self.span,
        }
    }
}

impl<'a> IntoOwned for ChapterStart<'a> {
    type Owned = ChapterStart<'static>;

    fn into_owned(self) -> Self::Owned {
        ChapterStart {
            number: self.number,
            alt_number: self.alt_number,
            pub_number: self.pub_number.map(own),
            span: self.span,
        }
    }
}

impl<'a> IntoOwned for VerseStart<'a> {
    type Owned = VerseStart<'static>;

    fn into_owned(self) -> Self::Owned {
        VerseStart {
            number: self.number,
            alt_number: self.alt_number,
            pub_number: self.pub_number.map(own),
            span: self.span,
        }
    }
}

impl<'a> IntoOwned for Para<'a> {
    type Owned = Para<'static>;

    fn into_owned(self) -> Self::Owned {
        Para {
            style: self.style,
            children: own_all(self.children),
            span: self.span,
        }
    }
}

impl<'a> IntoOwned for Char<'a> {
    type Owned = Char<'static>;

    fn into_owned(self) -> Self::Owned {
        Char {
            style: self.style,
            children: own_all(self.children),
            attributes: self.attributes.map(IntoOwned::into_owned),
            span: self.span,
        }
    }
}

impl<'a> IntoOwned for Note<'a> {
    type Owned = Note<'static>;

    fn into_owned(self) -> Self::Owned {
        Note {
            style: self.style,
            caller: self.caller.into_owned(),
            category: self.category.map(IntoOwned::into_owned),
            children: own_all(self.children),
            span: self.span,
        }
    }
}

impl<'a> IntoOwned for Caller<'a> {
    type Owned = Caller<'static>;

    fn into_owned(self) -> Self::Owned {
        match self {
            Caller::Plus => Caller::Plus,
            Caller::Minus => Caller::Minus,
            Caller::Custom(caller) => Caller::Custom(own(caller)),
        }
    }
}

impl<'a> IntoOwned for Milestone<'a> {
    type Owned = Milestone<'static>;

    fn into_owned(self) -> Self::Owned {
        Milestone {
            style: self.style,
            attributes: self.attributes.into_owned(),
            span: self.span,
        }
    }
}

impl<'a> IntoOwned for Attributes<'a> {
    type Owned = Attributes<'static>;

    fn into_owned(self) -> Self::Owned {
        Attributes {
            pairs: own_all(self.pairs),
        }
    }
}

impl<'a> IntoOwned for Attribute<'a> {
    type Owned = Attribute<'static>;

    fn into_owned(self) -> Self::Owned {
        Attribute {
            name: own(self.name),
            value: own(self.value),
        }
    }
}

impl<'a> IntoOwned for Table<'a> {
    type Owned = Table<'static>;

    fn into_owned(self) -> Self::Owned {
        Table {
            rows: own_all(self.rows),
            span: self.span,
        }
    }
}

impl<'a> IntoOwned for TableRow<'a> {
    type Owned = TableRow<'static>;

    fn into_owned(self) -> Self::Owned {
        TableRow {
            cells: own_all(self.cells),
            span: self.span,
        }
    }
}

impl<'a> IntoOwned for TableCell<'a> {
    type Owned = TableCell<'static>;

    fn into_owned(self) -> Self::Owned {
        TableCell {
            header: self.header,
            alignment: self.alignment,
            column: self.column,
            colspan: self.colspan,
            children: own_all(self.children),
            span: self.span,
        }
    }
}
