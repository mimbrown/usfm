use std::fmt::{Display, Formatter, Result};

use usfm_style::StyleSheet;

use crate::{ast::*, context::Context};

pub trait Serialize
where
    Self: Sized,
{
    fn serialize_chapter_start(
        &self,
        f: &mut Formatter<'_>,
        context: &mut Context,
        chapter: &ChapterStart,
    ) -> Result;

    fn serialize_chapter_end(
        &self,
        f: &mut Formatter<'_>,
        context: &mut Context,
        chapter: &ChapterEnd,
    ) -> Result;

    fn serialize_verse_start(
        &self,
        f: &mut Formatter<'_>,
        context: &mut Context,
        verse: &VerseStart,
    ) -> Result;

    fn serialize_verse_end(
        &self,
        f: &mut Formatter<'_>,
        context: &mut Context,
        verse: &VerseEnd,
    ) -> Result;

    fn serialize_text(&self, f: &mut Formatter<'_>, context: &mut Context, text: &str) -> Result;

    fn serialize_book(&self, f: &mut Formatter<'_>, context: &mut Context, book: &Book) -> Result;

    fn serialize_char(&self, f: &mut Formatter<'_>, context: &mut Context, char: &Char) -> Result;

    fn serialize_note(&self, f: &mut Formatter<'_>, context: &mut Context, note: &Note) -> Result;

    fn serialize_milestone(
        &self,
        f: &mut Formatter<'_>,
        context: &mut Context,
        milestone: &Milestone,
    ) -> Result;

    /// An optional line break (`//`).
    fn serialize_opt_break(
        &self,
        f: &mut Formatter<'_>,
        context: &mut Context,
        opt_break: &OptBreak,
    ) -> Result;

    /// A milestone that sits between blocks rather than inside a paragraph.
    /// Defaults to the inline form; a serializer with block-level layout
    /// (indentation, one block per line) overrides this to match.
    fn serialize_block_milestone(
        &self,
        f: &mut Formatter<'_>,
        context: &mut Context,
        milestone: &Milestone,
    ) -> Result {
        self.serialize_milestone(f, context, milestone)
    }

    fn serialize_table(
        &self,
        f: &mut Formatter<'_>,
        context: &mut Context,
        table: &Table,
    ) -> Result;

    /// A sidebar's blocks. Defaults to the blocks with no wrapper; a
    /// serializer with a container element overrides this.
    fn serialize_sidebar(
        &self,
        f: &mut Formatter<'_>,
        context: &mut Context,
        sidebar: &Sidebar,
    ) -> Result {
        sidebar.serialize(self, f, context)
    }

    /// A peripheral division's blocks; same default as `serialize_sidebar`.
    fn serialize_periph(
        &self,
        f: &mut Formatter<'_>,
        context: &mut Context,
        periph: &Periph,
    ) -> Result {
        periph.serialize(self, f, context)
    }

    fn serialize_para(&self, f: &mut Formatter<'_>, context: &mut Context, para: &Para) -> Result {
        para.serialize(self, f, context)
    }

    fn serialize_inline(
        &self,
        f: &mut Formatter<'_>,
        context: &mut Context,
        inline: &Inline,
    ) -> Result {
        inline.serialize(self, f, context)
    }

    fn serialize_block(
        &self,
        f: &mut Formatter<'_>,
        context: &mut Context,
        block: &Block,
    ) -> Result {
        block.serialize(self, f, context)
    }

    fn serialize_document(
        &self,
        f: &mut Formatter<'_>,
        context: &mut Context,
        document: &Document,
    ) -> Result {
        document.serialize(self, f, context)
    }
}

pub trait SerializeSelf<S>
where
    S: Serialize,
{
    fn serialize(&self, serializer: &S, f: &mut Formatter<'_>, context: &mut Context) -> Result;
}

impl<S> SerializeSelf<S> for Document<'_>
where
    S: Serialize,
{
    fn serialize(&self, serializer: &S, f: &mut Formatter<'_>, context: &mut Context) -> Result {
        for block in self.blocks.iter() {
            serializer.serialize_block(f, context, block)?;
        }
        Ok(())
    }
}

impl<S> SerializeSelf<S> for Block<'_>
where
    S: Serialize,
{
    fn serialize(&self, serializer: &S, f: &mut Formatter<'_>, context: &mut Context) -> Result {
        match self {
            Block::Book(book) => {
                context.book_code = Some(book.code);
                serializer.serialize_book(f, context, book)
            }
            Block::ChapterStart(chapter) => {
                context.chapter_number = Some(chapter.number);
                serializer.serialize_chapter_start(f, context, chapter)
            }
            Block::ChapterEnd(chapter) => {
                serializer.serialize_chapter_end(f, context, chapter)?;
                context.chapter_number = None;
                Ok(())
            }
            Block::Para(para) => serializer.serialize_para(f, context, para),
            Block::Table(table) => serializer.serialize_table(f, context, table),
            Block::Milestone(milestone) => {
                serializer.serialize_block_milestone(f, context, milestone)
            }
            Block::Sidebar(sidebar) => serializer.serialize_sidebar(f, context, sidebar),
            Block::Periph(periph) => serializer.serialize_periph(f, context, periph),
        }
    }
}

impl<S> SerializeSelf<S> for Periph<'_>
where
    S: Serialize,
{
    fn serialize(&self, serializer: &S, f: &mut Formatter<'_>, context: &mut Context) -> Result {
        for block in self.blocks.iter() {
            serializer.serialize_block(f, context, block)?;
        }
        Ok(())
    }
}

impl<S> SerializeSelf<S> for Sidebar<'_>
where
    S: Serialize,
{
    fn serialize(&self, serializer: &S, f: &mut Formatter<'_>, context: &mut Context) -> Result {
        for block in self.blocks.iter() {
            serializer.serialize_block(f, context, block)?;
        }
        Ok(())
    }
}

impl<S> SerializeSelf<S> for Inline<'_>
where
    S: Serialize,
{
    fn serialize(&self, serializer: &S, f: &mut Formatter<'_>, context: &mut Context) -> Result {
        match self {
            Inline::Text(text) => serializer.serialize_text(f, context, text),
            Inline::VerseStart(verse) => {
                context.verse_number = Some(verse.number.clone());
                serializer.serialize_verse_start(f, context, verse)
            }
            Inline::VerseEnd(verse) => {
                serializer.serialize_verse_end(f, context, verse)?;
                context.verse_number = None;
                Ok(())
            }
            Inline::Char(char) => serializer.serialize_char(f, context, char),
            Inline::Note(note) => serializer.serialize_note(f, context, note),
            Inline::Milestone(milestone) => serializer.serialize_milestone(f, context, milestone),
            Inline::OptBreak(opt_break) => serializer.serialize_opt_break(f, context, opt_break),
        }
    }
}

impl<S> SerializeSelf<S> for Para<'_>
where
    S: Serialize,
{
    fn serialize(&self, serializer: &S, f: &mut Formatter<'_>, context: &mut Context) -> Result {
        for para in self.children.iter() {
            serializer.serialize_inline(f, context, para)?;
        }
        Ok(())
    }
}

impl<S> SerializeSelf<S> for Char<'_>
where
    S: Serialize,
{
    fn serialize(&self, serializer: &S, f: &mut Formatter<'_>, context: &mut Context) -> Result {
        for inline in self.children.iter() {
            serializer.serialize_inline(f, context, inline)?;
        }
        Ok(())
    }
}

impl<S> SerializeSelf<S> for Note<'_>
where
    S: Serialize,
{
    fn serialize(&self, serializer: &S, f: &mut Formatter<'_>, context: &mut Context) -> Result {
        for inline in self.children.iter() {
            serializer.serialize_inline(f, context, inline)?;
        }
        Ok(())
    }
}

struct Serializer<'a, 'b, S>
where
    S: Serialize,
{
    serializer: S,
    document: &'a Document<'a>,
    style_sheet: &'b StyleSheet,
}

impl<'a, 'b, S> Display for Serializer<'a, 'b, S>
where
    S: Serialize,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        self.serializer
            .serialize_document(f, &mut Context::new(self.style_sheet), self.document)
    }
}

pub fn serialize<S>(document: &Document, style_sheet: &StyleSheet, serializer: S) -> String
where
    S: Serialize,
{
    format!(
        "{}",
        Serializer {
            serializer,
            document,
            style_sheet
        }
    )
}
