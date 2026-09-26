//! Recursive-descent USFM parser.
//!
//! The parser never fails: every malformed construct is repaired according
//! to the recovery table documented on [`crate::diagnostics::Code`], and a
//! [`Diagnostic`] is recorded for each repair. See
//! `tests/recovery.rs` for one test per rule.
//!
//! Structure: [`ParserImpl::parse`] runs the block loop (books, chapters,
//! paragraphs, tables). [`ParserImpl::parse_inner_list`] parses inline content
//! for one container (paragraph, character style, note, or table cell) and
//! returns the [`InnerListCloser`] that ended it, so the caller can decide
//! whether that closer belongs to it or must propagate outward.

use std::sync::Arc;
use std::{borrow::Cow, str::FromStr};
use usfm_style::{StyleRule, StyleSheet, StyleType, TextProperties, TextType};

use crate::ast::*;
use crate::diagnostics::{Code, Diagnostic, ParseResult};
use crate::lexer::span::{SPAN, Span};
use crate::style::Style;
use usfm_ast::string_parser::ParseStr;

use crate::{
    lexer::{Kind, Lexer, is_word_byte_terminator},
    parser_parse::UniquePromise,
};

#[derive(Debug)]
pub enum ParserInlineContext<'a> {
    Para(Para<'a>),
    Note(Note<'a>),
    Char(Char<'a>),
    TableCell(TableCell<'a>),
}

impl<'a> ParserInlineContext<'a> {
    /// A character style is implicitly closed by another (non-nested)
    /// character style opening inside it.
    fn is_implicitly_closed_by_char(&self) -> bool {
        matches!(self, ParserInlineContext::Char(_))
    }

    fn is_char(&self) -> bool {
        matches!(self, ParserInlineContext::Char(_))
    }

    /// The style of the character style being parsed, if this is one.
    fn char_style(&self) -> Option<StyleId> {
        match self {
            ParserInlineContext::Char(char) => Some(char.style),
            _ => None,
        }
    }

    /// Attach word-level attributes to the character style being parsed.
    /// Returns false if this context is not a character style.
    ///
    /// A second `|` in one style replaces the first, which is what the old
    /// last-child-wins representation did.
    fn set_attributes(&mut self, attributes: Attributes<'a>) -> bool {
        match self {
            ParserInlineContext::Char(char) => {
                char.attributes = Some(attributes);
                true
            }
            _ => false,
        }
    }
}

impl<'a> InlineContainer<'a> for ParserInlineContext<'a> {
    fn children(&self) -> &Vec<Inline<'a>> {
        match self {
            ParserInlineContext::Para(para) => &para.children,
            ParserInlineContext::Note(note) => &note.children,
            ParserInlineContext::Char(char) => &char.children,
            ParserInlineContext::TableCell(table_cell) => &table_cell.children,
        }
    }

    fn children_mut(&mut self) -> &mut Vec<Inline<'a>> {
        match self {
            ParserInlineContext::Para(para) => &mut para.children,
            ParserInlineContext::Note(note) => &mut note.children,
            ParserInlineContext::Char(char) => &mut char.children,
            ParserInlineContext::TableCell(table_cell) => &mut table_cell.children,
        }
    }
}

pub struct Parser<'a> {
    pub source_text: &'a str,
}

impl<'a> Parser<'a> {
    pub fn new(text: &'a str) -> Parser<'a> {
        Parser { source_text: text }
    }
}

/// An open character style or note, for matching closing markers.
struct Open {
    style: usize,
    is_note: bool,
}

/// The verse whose end milestone has not been placed yet (plan D4).
///
/// The parser tracks the open verse and emits its `VerseEnd` where the next
/// verse starts, where the chapter ends, or at end of input. The rules for
/// *where* in the tree the end goes are the ones tcdocs pins down:
///
/// - Before the next `\v` in the same container, with the whitespace before
///   `\v` moved after the end (`text<eid/> <v/>`).
/// - When the next `\v` opens an empty container (the first thing in a
///   character style, a table cell, or a paragraph), before that container
///   in its parent; and so on outward. At block level this means the end of
///   the last paragraph of verse text, or the last table cell, before the
///   block: headings, sidebars and peripheral matter never hold a verse end.
/// - When the next `\v` is in a paragraph that is not verse text (`\lit`),
///   also before that paragraph, unless the open verse started in it.
/// - Inside a sidebar or a periph, verses are not tracked at all: the
///   scripture flow continues around a sidebar, and peripheral matter has no
///   verses.
#[derive(Debug)]
struct OpenVerse {
    number: NumberList,
    /// Whether `\v` was in the block being parsed. A paragraph a verse
    /// started in holds its end whatever its style.
    started_in_current_block: bool,
}

pub struct ParserImpl<'a> {
    pub(crate) source_text: &'a str,
    pub(crate) lexer: Lexer<'a>,
    /// The document's stylesheet (hardening plan D3). Owned rather than
    /// borrowed because the parser extends it: derived milestone forms such
    /// as `\\k-s` are registered while parsing, and an index into an extended
    /// sheet would be meaningless to a caller holding only the base sheet.
    /// `Arc::make_mut` means a document needing no derived style pays no
    /// clone at all.
    pub(crate) style_sheet: Arc<StyleSheet>,
    // Stylesheet indices of the markers the parser has to recognise, resolved
    // once here so the hot paths compare a `usize` instead of a marker string
    // — `marker_name` allocates, and on a per-node path that allocation was
    // two thirds of ticket 37's regression. `usize::MAX` stands for a sheet
    // without the marker, which no style id equals; and `add_rule` only
    // appends and never renames, so a style derived while parsing can take
    // neither one of these indices nor one of these names.
    id: usize,
    c: usize,
    tr: usize,
    p: usize,
    esb: usize,
    esbe: usize,
    cat: usize,
    periph: usize,
    va: usize,
    vp: usize,
    cp: usize,
    should_insert_end_milestones: bool,
    diagnostics: Vec<Diagnostic>,
    /// Stack of open character styles and notes, innermost last.
    open: Vec<Open>,
    /// How many notes are open (verses are not allowed inside notes).
    note_depth: usize,
    /// Inside a table cell (cell markers close it from any depth).
    in_cell: bool,
    /// Text type of the paragraph being parsed: where a verse end goes
    /// depends on it (see `place_verse_end`).
    para_text_type: TextType,
    /// Parsing the `\periph Title|attrs` line, where the attribute list ends
    /// at the line break rather than at a closing marker.
    in_periph_title: bool,
    /// Parsing the attribute list of a start milestone that no `\*` closes,
    /// which ends at the line break as a `\periph` title's does, but leaves
    /// the line break where it is (ticket 43).
    line_bounded_attributes: bool,
    /// The verse whose `VerseEnd` is still to come; see [`OpenVerse`].
    open_verse: Option<OpenVerse>,
    /// The chapter whose `ChapterEnd` is still to come.
    open_chapter: Option<usize>,
    /// A verse end that could not go in the container where the next `\v`
    /// was found because that container was empty, with the depth of that
    /// container (`open.len()` at the time). The parent, at the depth below,
    /// places it before the child it is about to add, or hands it further
    /// out; depth 0 is a paragraph or table cell, handed to block level.
    pending_verse_end: Option<(OpenVerse, usize)>,
    /// Depth inside sidebars, where verses are not tracked. (A `\periph`
    /// division tracks its own since ticket 44.)
    verses_suspended: usize,
    /// End offset of the last consumed token; see `prev_token_end`.
    pub(crate) prev_token_end: u32,
}

struct TableCellInfo {
    header: bool,
    alignment: Alignment,
    column: u8,
    colspan: u8,
}

impl TableCellInfo {
    fn first_column() -> Self {
        Self {
            header: false,
            alignment: Alignment::Start,
            column: 1,
            colspan: 1,
        }
    }
}

impl FromStr for TableCellInfo {
    type Err = ();

    fn from_str(marker: &str) -> std::result::Result<Self, Self::Err> {
        let mut iter = marker.chars();
        if iter.next() != Some('t') {
            return Err(());
        }
        let header = match iter.next() {
            Some('c') => false,
            Some('h') => true,
            _ => return Err(()),
        };
        let mut digits = String::new();
        let mut start = u8::MAX;
        let mut end = u8::MAX;
        let alignment = match iter.next() {
            Some('c') => Alignment::Center,
            Some('r') => Alignment::End,
            Some(digit) if digit.is_ascii_digit() => {
                digits.push(digit);
                Alignment::Start
            }
            _ => return Err(()),
        };
        for c in iter {
            if c.is_ascii_digit() {
                digits.push(c);
            } else if c == '-' {
                if start == u8::MAX {
                    start = digits.parse().map_err(|_| ())?;
                    digits.clear();
                } else if end == u8::MAX {
                    end = digits.parse().map_err(|_| ())?;
                } else {
                    return Err(());
                }
            } else {
                return Err(());
            }
        }
        if start == u8::MAX {
            start = digits.parse().map_err(|_| ())?;
        } else if end == u8::MAX {
            end = digits.parse().map_err(|_| ())?;
        }
        Ok(TableCellInfo {
            header,
            alignment,
            column: start,
            colspan: if end == u8::MAX || end < start {
                1
            } else {
                end - start + 1
            },
        })
    }
}

/// Why `parse_inner_list` stopped.
/// What `~` stands for in USFM text.
const NO_BREAK_SPACE: &str = "\u{a0}";

/// `091`, `01-3`: a number (or the first number of a range) written with a
/// leading zero. `0` on its own is not.
fn has_leading_zero(word: &str) -> bool {
    let mut chars = word.chars();
    chars.next() == Some('0') && chars.next().is_some_and(|c| c.is_ascii_digit())
}

/// Rule 1 on `Text` over a string that was built by joining runs: every run of
/// ASCII whitespace becomes one space. Each run the parser read is already
/// normalised, so this only ever collapses a seam between two of them.
fn collapse_ascii_whitespace(text: &str) -> String {
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
    out
}

/// A synthesized verse end milestone.
fn verse_end<'a>(number: NumberList) -> Inline<'a> {
    Inline::VerseEnd(VerseEnd { number, span: SPAN })
}

/// An inline node's own span, whichever variant it is.
fn inline_span(inline: &Inline<'_>) -> Span {
    match inline {
        Inline::Text(text) => text.span,
        Inline::VerseStart(verse) => verse.span,
        Inline::VerseEnd(verse) => verse.span,
        Inline::Char(char) => char.span,
        Inline::Note(note) => note.span,
        Inline::Milestone(milestone) => milestone.span,
        Inline::OptBreak(opt_break) => opt_break.span,
    }
}

enum InnerListCloser {
    /// A table cell marker (already consumed) with its span; only produced
    /// inside a table cell.
    TableCell(TableCellInfo, Span),
    /// A paragraph-style marker (already consumed) with its span.
    Paragraph(usize, Span),
    /// A closing marker matching an open style; the owner of that style
    /// consumes it, anyone in between propagates it outward.
    ClosedMarker(usize),
    /// A non-nested character marker opened inside a character style,
    /// implicitly closing it. The marker has been consumed; the receiver
    /// opens it as a sibling.
    OtherOpenedMarker(usize, Span),
    Eof,
}

impl InnerListCloser {
    /// Whether this closer ends the text run at a paragraph-level boundary,
    /// where trailing whitespace is not content. See `Text` in `usfm_ast`.
    fn ends_block(&self) -> bool {
        matches!(
            self,
            InnerListCloser::Paragraph(..) | InnerListCloser::TableCell(..) | InnerListCloser::Eof
        )
    }
}

/// What stands before the first block of the list [`ParserImpl::parse_blocks`]
/// is filling, which decides whether a [`Block::Milestone`] may open it. See
/// [`ParserImpl::place_block_milestone`].
#[derive(Clone, Copy)]
enum BlockListHead {
    /// The document's own block list: nothing precedes it, so a milestone
    /// written first is read back as a milestone.
    Free,
    /// A `Sidebar`'s or a `Periph`'s list. The writer opens it with a line of
    /// its own — `\esb`, `\periph Title` — and that line runs to the next
    /// paragraph marker, so a milestone written first would be read back as
    /// part of it. The name is the marker that opens the line.
    AfterOpeningLine(&'static str),
}

/// What the block loop found when it needed a block marker.
enum BlockStart<'a> {
    Marker(usize, Span),
    /// A milestone between blocks, already parsed.
    Milestone(Milestone<'a>),
    /// Inline content with no paragraph open. Nothing consumed.
    Implicit,
    Eof,
}

impl<'a> ParserImpl<'a> {
    pub fn new(
        source_text: &'a str,
        style_sheet: &Arc<StyleSheet>,
        unique: UniquePromise,
    ) -> ParserImpl<'a> {
        let index = |marker: &str| *style_sheet.get_marker_index(marker).unwrap_or(&usize::MAX);
        ParserImpl {
            source_text,
            lexer: Lexer::new(source_text, unique),
            style_sheet: Arc::clone(style_sheet),
            id: index("id"),
            c: index("c"),
            tr: index("tr"),
            p: index("p"),
            esb: index("esb"),
            esbe: index("esbe"),
            cat: index("cat"),
            periph: index("periph"),
            va: index("va"),
            vp: index("vp"),
            cp: index("cp"),
            should_insert_end_milestones: true,
            diagnostics: Vec::new(),
            open: Vec::new(),
            note_depth: 0,
            in_cell: false,
            para_text_type: TextType::Other,
            in_periph_title: false,
            line_bounded_attributes: false,
            open_verse: None,
            open_chapter: None,
            pending_verse_end: None,
            verses_suspended: 0,
            prev_token_end: 0,
        }
    }

    pub fn should_insert_end_milestones(&mut self, should_insert: bool) {
        self.should_insert_end_milestones = should_insert;
    }

    fn emit(&mut self, code: Code, span: Span, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic::new(code, span, message));
    }

    fn rule(&self, marker: usize) -> &StyleRule {
        self.style_sheet.get_rule(marker)
    }

    /// The marker name, owned so it can be held across an `emit` call.
    /// Only diagnostic paths need this.
    fn marker_name(&self, marker: usize) -> String {
        self.rule(marker).marker.clone()
    }

    fn resolve_marker(&self, marker_word: &str) -> Option<usize> {
        self.style_sheet.get_marker_index(marker_word).copied()
    }

    /// Report a marker that is not in the stylesheet. User-defined `\z`
    /// markers are valid USFM, so they are a warning rather than an error.
    fn emit_unknown_marker(&mut self, name: &str, closing: bool, span: Span) {
        let star = if closing { "*" } else { "" };
        if name.starts_with('z') {
            self.emit(
                Code::UnknownCustomMarker,
                span,
                format!("custom marker `\\{name}{star}` is not in the stylesheet; dropped"),
            );
        } else {
            self.emit(
                Code::UnknownMarker,
                span,
                format!("unknown marker `\\{name}{star}`"),
            );
        }
    }

    /// Whether `style` is open in the current scope. Notes are their own
    /// scope: a closing marker inside a note cannot close something outside
    /// it, but can close the note itself.
    fn is_open(&self, style: usize) -> bool {
        for open in self.open.iter().rev() {
            if open.style == style {
                return true;
            }
            if open.is_note {
                return false;
            }
        }
        false
    }

    // ----- block level -------------------------------------------------

    pub fn parse(mut self) -> ParseResult<'a> {
        let mut blocks = vec![];
        let closer = self.parse_blocks(&mut blocks, None, BlockListHead::Free, |_| false);
        debug_assert!(closer.is_none(), "nothing stops the top-level block loop");
        // End of input closes the open verse and chapter.
        self.end_verse_before_block(&mut blocks);
        if let Some(number) = self.open_chapter.take() {
            blocks.push(Block::ChapterEnd(ChapterEnd { number, span: SPAN }));
        }
        let style_sheet = Arc::clone(&self.style_sheet);
        let source = Span::new(0, self.source_text.len() as u32);
        let document = Document::new(blocks, style_sheet).with_span(source);
        self.diagnostics.sort_by_key(|d| d.span.start);
        ParseResult {
            document,
            diagnostics: self.diagnostics,
        }
    }

    /// The block loop. Parses blocks into `blocks` until end of input, or
    /// until a block marker `stop` accepts, which is returned for the caller
    /// to handle (it has been consumed, like any pending marker). `pending` is
    /// a marker an earlier parse already consumed.
    fn parse_blocks(
        &mut self,
        blocks: &mut Vec<Block<'a>>,
        mut pending: Option<(usize, Span)>,
        head: BlockListHead,
        stop: impl Fn(usize) -> bool,
    ) -> Option<(usize, Span)> {
        loop {
            let (marker, span) = match pending.take() {
                Some(pending) => pending,
                None => match self.parse_block_start() {
                    BlockStart::Marker(marker, span) => (marker, span),
                    BlockStart::Milestone(milestone) => {
                        self.place_block_milestone(blocks, milestone, head);
                        continue;
                    }
                    BlockStart::Eof => return None,
                    BlockStart::Implicit => {
                        let span = self.cur_span();
                        if self.p == usize::MAX {
                            self.emit(
                                Code::ContentDropped,
                                span,
                                "content outside a paragraph dropped (stylesheet has no `p` marker)",
                            );
                            self.skip_to_paragraph_marker();
                            continue;
                        }
                        self.emit(
                            Code::ContentOutsideParagraph,
                            span,
                            "content outside a paragraph; an implicit `\\p` was opened",
                        );
                        (self.p, span)
                    }
                },
            };
            if stop(marker) {
                return Some((marker, span));
            }
            if marker == self.id {
                self.parse_id(blocks, span);
            } else if marker == self.c {
                self.parse_chapter(blocks, span);
            } else if marker == self.tr {
                pending = self.parse_table(blocks, span);
            } else if marker == self.esb {
                pending = self.parse_sidebar(blocks, span);
            } else if marker == self.periph {
                pending = self.parse_periph(blocks, span, &stop);
            } else {
                if marker == self.esbe {
                    // Inside a sidebar `stop` accepts it, so this one is stray.
                    self.emit(
                        Code::UnmatchedSidebarEnd,
                        span,
                        "`\\esbe` with no open `\\esb`",
                    );
                }
                pending = self.parse_paragraph(blocks, marker, span);
            }
        }
    }

    /// A milestone found between blocks, placed where a writer could put it
    /// back.
    ///
    /// USX writes `<ms>` next to `<para>`, so a milestone with no paragraph
    /// open is normally a `Block::Milestone`. But USFM has no marker that ends
    /// a line, and several block constructs are written as a line that runs to
    /// the next *paragraph* marker: a `\p` and its content, an `\esbe`, a `\tr`
    /// row, a `\periph` title. Write a milestone on the line after one of those
    /// and the parser reads it back as part of that line — so a
    /// `Block::Milestone` standing there is a tree no USFM spells. The rule is
    /// on [`Block::Milestone`]: only a `Book`, a `ChapterStart`, a
    /// `ChapterEnd`, another block milestone or the head of the document's own
    /// block list may precede one.
    ///
    /// Everything else takes the milestone in, which is what the written form
    /// says:
    ///
    /// * after a `Para` it is that paragraph's last child. The paragraph is
    ///   only ever *last* here because the marker that closed it left no node —
    ///   an `\id` with no book code, an unknown marker, anything the block loop
    ///   skipped (`r\id\-\*`, ticket 27).
    /// * after a `Table`, a `Sidebar` or a `Periph`, and at the head of a
    ///   sidebar's or a periph's own list, it opens an implicit `\p`, reported
    ///   like any other content outside a paragraph. Those spellings also need
    ///   a dropped marker to arise — `\esb\c\sh\*`, `\tr \tc1 y\c` then
    ///   `\zaln-s\*`, `\periph\id\e\*` (ticket 35).
    fn place_block_milestone(
        &mut self,
        blocks: &mut Vec<Block<'a>>,
        milestone: Milestone<'a>,
        head: BlockListHead,
    ) {
        if let Some(Block::Para(para)) = blocks.last_mut() {
            para.span.end = milestone.span.end;
            para.add_child(Inline::Milestone(milestone));
            return;
        }
        // The marker that ends the line the writer would put the milestone
        // on — which is the line that would swallow it. It names the *written*
        // form, so a sidebar the source never closed is still `\esbe`: that is
        // what `usfm_codegen` writes for it. `Block::Para` is not among them,
        // being handled above.
        let runs_on: Option<&'static str> = match blocks.last() {
            Some(Block::Table(_)) => Some("tr"),
            Some(Block::Sidebar(_)) => Some("esbe"),
            Some(Block::Periph(_)) => Some("periph"),
            // A `Book`, a `ChapterStart`, a `ChapterEnd` and a block milestone
            // each end their own line, so a milestone may follow one.
            Some(_) => None,
            None => match head {
                BlockListHead::Free => None,
                BlockListHead::AfterOpeningLine(marker) => Some(marker),
            },
        };
        let Some(runs_on) = runs_on else {
            blocks.push(Block::Milestone(milestone));
            return;
        };
        let span = milestone.span;
        let mut para = Para {
            style: StyleId::new(self.p as u32),
            children: vec![],
            span,
        };
        para.add_child(Inline::Milestone(milestone));
        self.content_after_marker(blocks, para, runs_on);
    }

    /// `\esb` … `\esbe`. The `\esb` line may carry a `\cat` category and
    /// nothing else; the blocks up to `\esbe` are the sidebar's. A `\c`, a
    /// second `\esb` or end of input closes an unclosed sidebar, since none
    /// of those can be inside one.
    fn parse_sidebar(&mut self, blocks: &mut Vec<Block<'a>>, esb_span: Span) -> Option<(usize, Span)> {
        let mut sidebar = Sidebar {
            style: StyleId::new(self.esb as u32),
            category: None,
            blocks: vec![],
            span: esb_span,
        };
        // Verses are not tracked inside: a verse open before `\esb` is still
        // open after `\esbe`, and its end never goes inside the sidebar.
        self.verses_suspended += 1;
        let mut head = vec![];
        let mut pending = self.parse_paragraph(&mut head, self.esb, esb_span);
        if let Some(Block::Para(mut para)) = head.pop() {
            sidebar.category = self.take_category(&mut para.children);
            if !para.children.is_empty() {
                self.content_after_marker(&mut sidebar.blocks, para, "esb");
            }
        }

        let (esb, esbe, c) = (self.esb, self.esbe, self.c);
        pending = self.parse_blocks(
            &mut sidebar.blocks,
            pending,
            BlockListHead::AfterOpeningLine("esb"),
            |marker| marker == esbe || marker == esb || marker == c,
        );
        self.verses_suspended -= 1;
        match pending {
            Some((marker, span)) if marker == esbe => {
                sidebar.span.end = span.end;
                // The sidebar goes in *before* the `\esbe` line is parsed,
                // because that line is a paragraph like any other and a `\v`
                // on it ends the verse that was open before `\esb`. That end
                // cannot go inside the sidebar (see `end_verse_in_last_block`),
                // so `parse_paragraph` hands it to the block list it is given —
                // and a list of its own would be empty and drop it, which is
                // what ticket 28 was. With `blocks`, the end lands in the last
                // verse-text block before the sidebar, exactly where it lands
                // when a `\p` follows `\esbe` and the `\v` is that paragraph's.
                blocks.push(Block::Sidebar(sidebar));
                // The text type is `\p`'s, not `\esbe`'s: what is on this line
                // becomes an implicit `\p` below. See `parse_paragraph_as`.
                let text_type_of = if self.p == usize::MAX { esbe } else { self.p };
                pending = self.parse_paragraph_as(blocks, esbe, span, text_type_of);
                // Take back the paragraph `\esbe` opened: its content, if any,
                // belongs to an implicit `\p` after the sidebar, not to a
                // paragraph styled `\esbe`. Guarded rather than assumed —
                // nothing here may panic on an input.
                if matches!(blocks.last(), Some(Block::Para(para)) if para.style.index() == esbe)
                    && let Some(Block::Para(para)) = blocks.pop()
                    && !para.children.is_empty()
                {
                    self.content_after_marker(blocks, para, "esbe");
                }
                return pending;
            }
            Some((marker, span)) => {
                let by = self.marker_name(marker);
                self.emit(
                    Code::SidebarNotClosed,
                    esb_span,
                    format!("`\\esb` is not closed by `\\esbe` before `\\{by}`"),
                );
                sidebar.span.end = span.start;
            }
            None => {
                self.emit(
                    Code::SidebarNotClosed,
                    esb_span,
                    "`\\esb` is not closed by `\\esbe`",
                );
                sidebar.span.end = self.prev_token_end();
            }
        }
        blocks.push(Block::Sidebar(sidebar));
        pending
    }

    /// `\periph Title|id="x"` and every block up to the next `\periph`, the
    /// next `\id`, or end of input. The line is read as a character style so
    /// that `|` introduces attributes; its text is the title.
    fn parse_periph(
        &mut self,
        blocks: &mut Vec<Block<'a>>,
        marker_span: Span,
        outer_stop: &dyn Fn(usize) -> bool,
    ) -> Option<(usize, Span)> {
        self.eat_whitespace();
        let mut head = ParserInlineContext::Char(Char {
            style: StyleId::new(self.periph as u32),
            children: vec![],
            attributes: None,
            span: marker_span,
        });
        // A division runs to the next `\periph` or `\id`, so nothing written
        // after this line is outside it: the verse and the chapter open
        // before it end here, before the periph (ticket 44). Inside, verses
        // and chapters are tracked as at the top level — `usx.rnc`'s
        // `PeripheralContent` allows `Chapter`, and every chapter in USX is
        // closed — and whatever is still open when the division ends ends
        // inside it, below. A `\v` on this line opens its verse like any
        // other: the line's non-text content becomes the division's implicit
        // `\p` (ticket 35), which is where that verse's end will go.
        if self.tracking_verses() {
            self.end_verse_before_block(blocks);
            if let Some(number) = self.open_chapter.take() {
                blocks.push(Block::ChapterEnd(ChapterEnd { number, span: SPAN }));
            }
        }
        self.in_periph_title = true;
        let closer = self.parse_inner_list(&mut head);
        self.in_periph_title = false;
        let ParserInlineContext::Char(head) = head else {
            unreachable!("context variant does not change");
        };
        // Only the line's *text* is the title. Anything else it holds — a
        // character style, a note, a milestone — used to be read and then
        // thrown away without a word, which "recovery is never silent"
        // (`docs/plans/hardening.md` D1) does not allow, and which no writer
        // could reproduce: written back it would be gone. Take it out now and
        // give it an implicit `\p` at the head of the division, which is where
        // the writer puts a milestone that belongs there anyway
        // (`Block::Milestone`'s rule, ticket 35).
        let (runs, over): (Vec<Inline<'a>>, Vec<Inline<'a>>) = head
            .children
            .into_iter()
            .partition(|inline| matches!(inline, Inline::Text(_)));
        // The title's span is its text run, up to the `|`. Only text read from
        // the source counts: a node the line synthesized carries `SPAN`,
        // which would put the end of the title at offset 0.
        let title_end = runs
            .iter()
            .rev()
            .find_map(|inline| match inline {
                Inline::Text(text) if text.span != SPAN => Some(text.span.end),
                _ => None,
            })
            .unwrap_or(marker_span.end);
        let title: String = runs
            .iter()
            .filter_map(|inline| match inline {
                Inline::Text(text) => Some(text.content.as_ref()),
                _ => None,
            })
            .collect();
        // Rule 1 on `Text`: the title is the line's text runs joined, and a
        // child that contributes none — a character style, a milestone —
        // leaves the whitespace on both sides of it next to each other. Each
        // run is normalised on its own, so only the seams double up; collapse
        // them, as the lexer would have in a single run.
        let title = collapse_ascii_whitespace(
            title.trim_matches(|c: char| c.is_ascii_whitespace()),
        );
        let mut periph = Periph {
            style: StyleId::new(self.periph as u32),
            title: (!title.is_empty()).then(|| {
                let start = runs
                    .iter()
                    .find_map(|inline| match inline {
                        Inline::Text(text) if text.span != SPAN => Some(text.span.start),
                        _ => None,
                    })
                    .unwrap_or(marker_span.end);
                Text::new(title.clone(), Span::new(start, title_end))
            }),
            attributes: head.attributes,
            blocks: vec![],
            span: marker_span,
        };
        self.place_periph_title_leftovers(&mut periph.blocks, over, marker_span);
        let pending = match closer {
            InnerListCloser::Paragraph(marker, span) => Some((marker, span)),
            InnerListCloser::Eof => None,
            // Only a paragraph marker or EOF can end a title line: a closing
            // marker would need an open style, and a cell marker a table.
            InnerListCloser::ClosedMarker(_)
            | InnerListCloser::OtherOpenedMarker(..)
            | InnerListCloser::TableCell(..) => {
                self.emit(
                    Code::Internal,
                    self.cur_span(),
                    "`\\periph` title closed by an inline closer; please report this input",
                );
                None
            }
        };
        let (periph_marker, id) = (self.periph, self.id);
        // The division runs to the next `\periph` or `\id`. An `\id` ends it
        // only if it *is* one: `parse_id` drops a `\id` with no book code or
        // with a code it cannot read, and a marker that is dropped ends
        // nothing — the blocks after it are still the division's. Written out,
        // a block left beside the periph is swallowed by it anyway, so this
        // is also the only reading a writer can reproduce (ticket 27).
        let mut book = None;
        let mut id_span = None;
        let mut pending = pending;
        loop {
            pending = self.parse_blocks(
                &mut periph.blocks,
                pending,
                BlockListHead::AfterOpeningLine("periph"),
                |marker| {
                    // `outer_stop` too: whatever ends the container this
                    // division is in ends the division. Without it a `\periph`
                    // inside a sidebar swallowed the `\esbe` that closes it —
                    // and a sidebar the writer cannot close is a tree it
                    // cannot write (ticket 27). At the top level `outer_stop`
                    // is never true.
                    marker == periph_marker || marker == id || outer_stop(marker)
                },
            );
            let Some((marker, span)) = pending else { break };
            if marker != id {
                break;
            }
            let mut parsed = vec![];
            self.parse_id(&mut parsed, span);
            pending = None;
            if let Some(block) = parsed.pop() {
                id_span = Some(span);
                book = Some(block);
                break;
            }
        }
        // Whatever the division opened ends with it.
        if self.tracking_verses() {
            self.end_verse_before_block(&mut periph.blocks);
            if let Some(number) = self.open_chapter.take() {
                periph.blocks.push(Block::ChapterEnd(ChapterEnd { number, span: SPAN }));
            }
        }
        periph.span.end = match (id_span, pending) {
            (Some(span), _) | (None, Some((_, span))) => span.start,
            (None, None) => self.prev_token_end(),
        };
        blocks.push(Block::Periph(periph));
        if let Some(block) = book {
            blocks.push(block);
        }
        pending
    }

    /// Everything on a `\periph` line that the title is not. The line is read
    /// as a character style so that `|` introduces attributes, so it can hold
    /// any inline node; only the text runs become the title. The rest opens an
    /// implicit `\p` at the head of the division, in source order — the same
    /// place `place_block_milestone` gives a milestone that follows the line,
    /// so the two spellings of `\periph` + `\qt-s\*` read the same and the
    /// writer can put either back.
    fn place_periph_title_leftovers(
        &mut self,
        blocks: &mut Vec<Block<'a>>,
        over: Vec<Inline<'a>>,
        marker_span: Span,
    ) {
        if over.is_empty() {
            return;
        }
        // The implicit `\p` has no marker of its own, so its span is the
        // content it holds (`span_check`'s `MarkerOrImplicit`). A node the
        // line synthesized carries `SPAN` and is not source to point at.
        let read = || over.iter().map(inline_span).filter(|span| *span != SPAN);
        let span = match (read().next(), read().next_back()) {
            (Some(first), Some(last)) => Span::new(first.start, last.end),
            _ => marker_span,
        };
        let mut para = Para {
            style: StyleId::new(self.p as u32),
            children: vec![],
            span,
        };
        for inline in over {
            para.add_child(inline);
        }
        self.content_after_marker(blocks, para, "periph");
    }

    /// Content on a marker line that takes none (`\esb`, `\esbe`): keep it
    /// in an implicit `\p` rather than drop it.
    fn content_after_marker(&mut self, blocks: &mut Vec<Block<'a>>, mut para: Para<'a>, name: &str) {
        if self.p == usize::MAX {
            self.emit(
                Code::ContentDropped,
                para.span,
                format!("content after `\\{name}` dropped (stylesheet has no `p` marker)"),
            );
            return;
        }
        self.emit(
            Code::ContentOutsideParagraph,
            para.span,
            format!("content after `\\{name}` is outside a paragraph; an implicit `\\p` was opened"),
        );
        para.style = StyleId::new(self.p as u32);
        blocks.push(Block::Para(para));
    }

    /// Lift a leading `\cat …\cat*` out of `children` as a category. Leading
    /// whitespace after it belongs to the marker, like any marker's.
    fn take_category(&mut self, children: &mut Vec<Inline<'a>>) -> Option<Text<'a>> {
        let is_cat = matches!(children.first(), Some(Inline::Char(char)) if char.style.index() == self.cat);
        if !is_cat {
            return None;
        }
        let Inline::Char(char) = children.remove(0) else {
            unreachable!("checked above");
        };
        // Rule 1 on `Text`, as in `parse_periph`: the category is the `\cat`
        // style's text runs joined, so a child that contributes none — a
        // character style, a milestone — leaves the whitespace on both sides
        // of itself side by side. Each run is normalised on its own, so only
        // the seams double up.
        let content: String = collapse_ascii_whitespace(
            &char
                .children
                .iter()
                .filter_map(|inline| match inline {
                    Inline::Text(text) => Some(text.content.as_ref()),
                    _ => None,
                })
                .collect::<String>(),
        );
        if let Some(Inline::Text(text)) = children.first_mut() {
            let trimmed = text.content.trim_start_matches(|c: char| c.is_ascii_whitespace());
            if trimmed.is_empty() {
                children.remove(0);
            } else if trimmed.len() != text.content.len() {
                text.content = Cow::Owned(trimmed.to_string());
            }
        }
        Some(Text::new(content, char.span))
    }

    /// Find the next block marker, reporting and skipping anything that
    /// cannot start a block. Stops without consuming at inline content.
    fn parse_block_start(&mut self) -> BlockStart<'a> {
        loop {
            self.eat_whitespace();
            match self.cur_kind() {
                Kind::Eof => return BlockStart::Eof,
                Kind::Marker { closing: false, .. } => {
                    let name = self.cur_marker_name();
                    match self.resolve_marker(name) {
                        None => {
                            let name = name.to_string();
                            let span = self.bump_span();
                            match self.take_unknown_milestone(&name, span) {
                                Some(attributes) => {
                                    let style = self.register_milestone(&name, span);
                                    return BlockStart::Milestone(Milestone {
                                        style: StyleId::new(style as u32),
                                        attributes,
                                        span: Span::new(span.start, self.prev_token_end()),
                                    });
                                }
                                None => self.emit_unknown_marker(&name, false, span),
                            }
                        }
                        Some(marker) if self.rule(marker).is_paragraph() => {
                            let span = self.bump_span();
                            self.eat_whitespace();
                            return BlockStart::Marker(marker, span);
                        }
                        // A milestone with no paragraph open belongs between
                        // blocks (USX puts it beside `<para>`), so it must not
                        // drag an implicit `\p` into existence around it.
                        Some(marker) if self.rule(marker).is_milestone() => {
                            let span = self.bump_span();
                            let milestone = self.parse_milestone_node(marker, span);
                            return BlockStart::Milestone(milestone);
                        }
                        Some(_) => return BlockStart::Implicit,
                    }
                }
                Kind::Marker { closing: true, .. } => {
                    let name = self.cur_marker_name();
                    let span = self.bump_span();
                    match self.resolve_marker(name) {
                        None => self.emit_unknown_marker(name, true, span),
                        Some(marker) if self.rule(marker).is_paragraph() => self.emit(
                            Code::ParagraphMarkerClosed,
                            span,
                            format!("paragraph marker `\\{name}` cannot be closed with `*`"),
                        ),
                        Some(_) => self.emit(
                            Code::UnmatchedClosingMarker,
                            span,
                            format!("closing marker `\\{name}*` has no matching `\\{name}`"),
                        ),
                    }
                }
                Kind::MilestoneEnd => {
                    let span = self.bump_span();
                    self.emit(
                        Code::UnmatchedMilestoneEnd,
                        span,
                        "`\\*` with no open milestone",
                    );
                }
                _ => return BlockStart::Implicit,
            }
        }
    }

    /// Skip tokens until a paragraph marker or end of input.
    fn skip_to_paragraph_marker(&mut self) {
        loop {
            match self.cur_kind() {
                Kind::Eof => return,
                Kind::Marker { closing: false, .. } => {
                    if let Some(marker) = self.resolve_marker(self.cur_marker_name())
                        && self.rule(marker).is_paragraph()
                    {
                        return;
                    }
                    self.bump_any();
                }
                _ => self.bump_any(),
            }
        }
    }

    fn parse_id(&mut self, blocks: &mut Vec<Block<'a>>, marker_span: Span) {
        let code_span = self.cur_span();
        let Some(word) = self.eat_word() else {
            self.emit(
                Code::MissingBookCode,
                marker_span,
                "`\\id` must be followed by a book code",
            );
            return;
        };
        let mut end = self.prev_token_end();
        self.eat_whitespace();
        let description = if self.cur_kind().is_text() {
            let text = self.parse_text();
            // The run as read includes the trailing newline; the `\id` node
            // should stop at the description it actually holds.
            let run = &self.source_text[text.span.start as usize..text.span.end as usize];
            end = text.span.start + run.trim_end_matches(|c: char| c.is_ascii_whitespace()).len() as u32;
            Cow::Owned(
                text.trim_end_matches(|c: char| c.is_ascii_whitespace())
                    .to_string(),
            )
        } else {
            Cow::Borrowed("")
        };
        match BookCode::from_str(word) {
            Ok(code) => {
                // Nothing is reported about a `\id` that parses: a code USX
                // accepts but this parser does not name (`unlisted-book-code`,
                // ticket 19), a `\id` after other content (`id-not-first`) and
                // a file with nothing else in it (`empty-book`, ticket 21) are
                // all judgements about the finished document, which the tree
                // carries — the `Book` block and where it sits in the block
                // list — so `usfm_semantic` makes them. `unknown-book-code`
                // below stays, because that one drops the `\id` line.
                blocks.push(Block::Book(Book {
                    code,
                    description,
                    span: Span::new(marker_span.start, end),
                }));
            }
            Err(_) => self.emit(
                Code::UnknownBookCode,
                code_span,
                format!("`{word}` is not a book code"),
            ),
        }
    }

    fn parse_chapter(&mut self, blocks: &mut Vec<Block<'a>>, marker_span: Span) {
        let number_span = self.cur_span();
        let Some(word) = self.eat_word() else {
            self.emit(
                Code::MissingChapterNumber,
                marker_span,
                "`\\c` must be followed by a chapter number",
            );
            return;
        };
        let Ok(number) = word.parse::<usize>() else {
            self.emit(
                Code::MalformedChapterNumber,
                number_span,
                format!("`{word}` is not a valid chapter number"),
            );
            return;
        };
        if has_leading_zero(word) {
            self.emit(
                Code::NumberHasLeadingZero,
                number_span,
                format!("chapter number `{word}` has a leading zero"),
            );
        }
        let mut chapter_start = ChapterStart {
            number,
            alt_number: None,
            pub_number: None,
            // Extended below, but only if a `\ca`/`\cp` actually follows: the
            // whitespace eaten while looking for one is not part of the node.
            span: Span::new(marker_span.start, self.prev_token_end()),
        };
        self.eat_whitespace();
        loop {
            if self.at_opening_marker("ca") {
                let ca_span = self.bump_span();
                self.eat_whitespace();
                let alt_span = self.cur_span();
                match self.eat_word().map(NumberList::parse_str) {
                    Some(Ok(alt)) => chapter_start.alt_number = Some(alt),
                    Some(Err(_)) => self.emit(
                        Code::MalformedChapterNumber,
                        alt_span,
                        "`\\ca` must be followed by a chapter number",
                    ),
                    None => self.emit(
                        Code::MalformedChapterNumber,
                        ca_span,
                        "`\\ca` must be followed by a chapter number",
                    ),
                }
                self.eat_whitespace();
                if !self.eat_closing_marker("ca") {
                    self.emit(
                        Code::AlternateChapterNotClosed,
                        ca_span,
                        "`\\ca` is not closed by `\\ca*`",
                    );
                }
                chapter_start.span.end = self.prev_token_end();
                self.eat_whitespace();
            } else if self.at_opening_marker("cp") {
                self.bump_any();
                self.eat_whitespace();
                chapter_start.pub_number = self.eat_word().map(Cow::Borrowed);
                chapter_start.span.end = self.prev_token_end();
                self.eat_whitespace();
            } else {
                break;
            }
        }
        // A chapter boundary closes the open verse and the open chapter.
        if self.tracking_verses() {
            self.end_verse_before_block(blocks);
            if let Some(number) = self.open_chapter.take() {
                blocks.push(Block::ChapterEnd(ChapterEnd { number, span: SPAN }));
            }
            self.open_chapter = Some(number);
        }
        blocks.push(Block::ChapterStart(chapter_start));
    }

    /// Parse one paragraph. Returns the paragraph marker that closed it, if any.
    fn parse_paragraph(
        &mut self,
        blocks: &mut Vec<Block<'a>>,
        marker: usize,
        span: Span,
    ) -> Option<(usize, Span)> {
        self.parse_paragraph_as(blocks, marker, span, marker)
    }

    /// [`parse_paragraph`](Self::parse_paragraph), with the text type taken
    /// from a different marker than the style.
    ///
    /// The one caller that needs this is the `\esbe` line: whatever stands on
    /// it is not a paragraph styled `\esbe` in the end — `content_after_marker`
    /// turns it into an implicit `\p` — so a verse there has to place the
    /// previous verse's end the way a `\p` would, inline after the text
    /// already in the paragraph. The round-trip fuzz target found the
    /// disagreement (ticket 27): the writer turns that line into a real `\p`,
    /// and the two spellings must read back the same.
    fn parse_paragraph_as(
        &mut self,
        blocks: &mut Vec<Block<'a>>,
        marker: usize,
        span: Span,
        text_type_of: usize,
    ) -> Option<(usize, Span)> {
        // The text type decides where a verse end goes (see `place_verse_end`),
        // which is the one thing the paragraph's style changes about the tree.
        // Whether a verse-text paragraph may stand here at all
        // (`verse-text-before-chapter`) is `usfm_semantic`'s: the tree holds
        // the paragraph, its style and the chapter it does or does not follow.
        self.para_text_type = self.rule(text_type_of).text_type.clone();
        self.start_block();
        let mut context = ParserInlineContext::Para(Para {
            style: StyleId::new(marker as u32),
            children: vec![],
            span,
        });
        let closer = self.parse_inner_list(&mut context);
        let ParserInlineContext::Para(mut para) = context else {
            unreachable!("context variant does not change");
        };
        para.span.end = self.container_end(&closer);
        self.place_pending_verse_end_before_block(blocks);
        // A `\cp` paragraph cannot follow a chapter start: `\c` absorbs a
        // `\cp` that comes after it, so a writer's `\c 3` and `\cp A` on the
        // next line read back as the chapter's published number, not as a
        // paragraph. It only ever gets here because a marker between the two
        // was dropped (`\c 3\c` and then `\cp A`: the second `\c` has no
        // number and goes). So hand it to the chapter, reading it exactly as
        // `parse_chapter` would have. The round-trip fuzz target found this
        // (ticket 27).
        if marker == self.cp && matches!(blocks.last(), Some(Block::ChapterStart(_))) {
            self.fold_published_number_into_chapter(blocks, para);
            return self.block_closer(closer);
        }
        blocks.push(Block::Para(para));
        self.block_closer(closer)
    }

    /// Read a `\cp` paragraph as the preceding chapter's published number, the
    /// way `parse_chapter`'s own loop reads one: the first word is the number,
    /// and anything after it is content outside a paragraph.
    fn fold_published_number_into_chapter(
        &mut self,
        blocks: &mut Vec<Block<'a>>,
        mut para: Para<'a>,
    ) {
        let mut pub_number = None;
        if let Some(Inline::Text(text)) = para.children.first_mut() {
            let content = text.content.as_ref();
            let end = content
                .find(|c: char| c.is_ascii_whitespace())
                .unwrap_or(content.len());
            let word = &content[..end];
            // `\cp`'s own reader takes one `Word` token, so a published number
            // is only ever a word the lexer would hand back whole — and
            // `usfm_codegen` writes it verbatim for the same reason. This
            // paragraph's text can hold what a word cannot: a stray `\`, a
            // `"`, a `|`. Written as `\cp <that>`, the line would read back
            // differently or not at all, so such a first word stays text and
            // the whole paragraph becomes the implicit `\p` below (ticket 35).
            let readable_as_a_word = !word.is_empty()
                && !word.bytes().any(is_word_byte_terminator);
            if readable_as_a_word {
                pub_number = Some(Cow::Owned(word.to_string()));
                let rest = content[end..]
                    .trim_start_matches(|c: char| c.is_ascii_whitespace())
                    .to_string();
                if rest.is_empty() {
                    para.children.remove(0);
                } else {
                    text.content = Cow::Owned(rest);
                }
            }
        }
        if let Some(Block::ChapterStart(chapter)) = blocks.last_mut() {
            chapter.pub_number = pub_number;
            chapter.span.end = para.span.end;
        }
        if !para.children.is_empty() {
            self.content_after_marker(blocks, para, "cp");
        }
    }

    /// `tr_span` is the `\tr` marker that opened the table; the block loop has
    /// already consumed it.
    fn parse_table(&mut self, blocks: &mut Vec<Block<'a>>, tr_span: Span) -> Option<(usize, Span)> {
        self.start_block();
        let mut rows: Vec<TableRow<'a>> = vec![];
        let mut row_span = tr_span;
        let mut before_table = None;
        let closer = loop {
            let (row, closer, before_row) = self.parse_table_row(row_span);
            // A verse starting in the first cell of this row ends the
            // previous one at the end of the previous row's last cell, or
            // before the table when this is the first row.
            if let Some(open) = before_row {
                match rows.last_mut().and_then(|row| row.cells.last_mut()) {
                    Some(cell) => cell.children.push(verse_end(open.number)),
                    None => before_table = Some(open),
                }
            }
            rows.push(row);
            match closer {
                InnerListCloser::Paragraph(marker, span) if marker == self.tr => {
                    row_span = span;
                    continue;
                }
                closer => break closer,
            }
        };
        let span = match (rows.first(), rows.last()) {
            (Some(first), Some(last)) => Span::new(first.span.start, last.span.end),
            _ => SPAN,
        };
        if let Some(open) = before_table {
            self.end_verse_in_last_block(blocks, open.number);
        }
        // Two tables with nothing between them are one table: consecutive
        // `\tr` rows belong to the same `<table>`, so a writer cannot put
        // them side by side. They only get here when the marker that split
        // them was dropped (`\tr \tc1 x\c` and then another `\tr`: the `\c`
        // has no number and goes), and the round-trip fuzz target found that
        // (ticket 27). Join them, which is what the source that is left says.
        match blocks.last_mut() {
            Some(Block::Table(table)) => {
                table.span.end = span.end;
                table.rows.extend(rows);
            }
            _ => blocks.push(Block::Table(Table { rows, span })),
        }
        self.block_closer(closer)
    }

    /// Where a container closed by `closer` ends.
    ///
    /// A marker that closes a container from outside it (a paragraph marker, a
    /// sibling character style, the next table cell) is not part of the
    /// container, so the container ends where that marker starts. Everything
    /// else — an explicit closing marker, end of input — ended at the last
    /// token consumed.
    fn container_end(&self, closer: &InnerListCloser) -> u32 {
        match closer {
            InnerListCloser::Paragraph(_, span)
            | InnerListCloser::OtherOpenedMarker(_, span)
            | InnerListCloser::TableCell(_, span) => span.start,
            InnerListCloser::ClosedMarker(_) | InnerListCloser::Eof => self.prev_token_end(),
        }
    }

    /// Interpret the closer of a block-level container.
    fn block_closer(&mut self, closer: InnerListCloser) -> Option<(usize, Span)> {
        match closer {
            InnerListCloser::Paragraph(marker, span) => Some((marker, span)),
            InnerListCloser::Eof => None,
            InnerListCloser::TableCell(..)
            | InnerListCloser::ClosedMarker(_)
            | InnerListCloser::OtherOpenedMarker(..) => {
                // Cell markers only close cells, and the other two are
                // consumed by the character style or note that owns them.
                self.emit(
                    Code::Internal,
                    self.cur_span(),
                    "block closed by an inline closer; please report this input",
                );
                None
            }
        }
    }

    /// Also returns the end of a verse that a `\v` at the start of the
    /// row's first cell left for the caller to place before the row.
    fn parse_table_row(
        &mut self,
        tr_span: Span,
    ) -> (TableRow<'a>, InnerListCloser, Option<OpenVerse>) {
        let row_start = tr_span.start;
        let mut row = TableRow {
            cells: vec![],
            span: SPAN,
        };
        let mut before_row = None;
        let mut cell_span = self.cur_span();
        let mut info = if self.cur_kind().is_opening_marker()
            && let Ok(info) = TableCellInfo::from_str(self.cur_marker_name())
        {
            self.bump_any();
            self.eat_whitespace();
            info
        } else {
            self.emit(
                Code::ExpectedTableCell,
                self.cur_span(),
                "`\\tr` must be followed by a table cell marker; an implicit `\\tc1` was opened",
            );
            TableCellInfo::first_column()
        };
        // Every cell keeps the column its marker names, gap or not, so a row's
        // columns are in the tree as written and `unexpected-table-column` is
        // `usfm_semantic`'s to report (ticket 21).
        loop {
            let (cell, closer) = self.parse_table_cell(info, cell_span);
            // A verse starting at the beginning of this cell ends the
            // previous one at the end of the previous cell.
            if let Some(open) = self.take_pending_verse_end(0) {
                match row.cells.last_mut() {
                    Some(previous) => previous.children.push(verse_end(open.number)),
                    None => before_row = Some(open),
                }
            }
            row.cells.push(cell);
            match closer {
                InnerListCloser::TableCell(next_info, span) => {
                    info = next_info;
                    cell_span = span;
                }
                closer => {
                    row.span = Span::new(row_start, self.container_end(&closer));
                    return (row, closer, before_row);
                }
            }
        }
    }

    /// `marker_span` is the cell's own marker, already consumed by the caller.
    fn parse_table_cell(
        &mut self,
        info: TableCellInfo,
        marker_span: Span,
    ) -> (TableCell<'a>, InnerListCloser) {
        let cell_start = marker_span.start;
        let mut context = ParserInlineContext::TableCell(TableCell {
            children: vec![],
            header: info.header,
            alignment: info.alignment,
            column: info.column,
            colspan: info.colspan,
            span: SPAN,
        });
        self.in_cell = true;
        let closer = self.parse_inner_list(&mut context);
        self.in_cell = false;
        let ParserInlineContext::TableCell(mut table_cell) = context else {
            unreachable!("context variant does not change");
        };
        table_cell.span = Span::new(cell_start, self.container_end(&closer));
        (table_cell, closer)
    }

    // ----- verse and chapter ends (plan D4) ----------------------------
    //
    // See `OpenVerse` for the placement rules. The parser cannot reach a
    // container's parent while parsing the container, so an end that belongs
    // before the current container is left in `pending_verse_end` and placed
    // by whoever adds that container to its parent: `add_char` for character
    // styles, the row and table loops for cells and rows, and
    // `place_pending_verse_end_before_block` for blocks.

    /// Whether verse and chapter ends are being emitted here at all.
    fn tracking_verses(&self) -> bool {
        self.should_insert_end_milestones && self.verses_suspended == 0
    }

    /// A new block is starting: the open verse, if any, did not start in it.
    fn start_block(&mut self) {
        if let Some(open) = &mut self.open_verse {
            open.started_in_current_block = false;
        }
    }

    /// `\v number` was found in `context`: end the open verse and open this
    /// one.
    fn open_verse(&mut self, context: &mut ParserInlineContext<'a>, number: &NumberList) {
        if !self.tracking_verses() {
            return;
        }
        if let Some(open) = self.open_verse.take() {
            self.place_verse_end(context, open);
        }
        self.open_verse = Some(OpenVerse {
            number: number.clone(),
            started_in_current_block: true,
        });
    }

    /// Place the end of `open` before whatever is added to `context` next,
    /// or hand it outward when it belongs before `context` itself.
    fn place_verse_end(&mut self, context: &mut ParserInlineContext<'a>, open: OpenVerse) {
        let in_non_verse_text_para = matches!(context, ParserInlineContext::Para(_))
            && self.para_text_type != TextType::VerseText
            && !open.started_in_current_block;
        if in_non_verse_text_para || context.children().is_empty() {
            self.pending_verse_end = Some((open, self.open.len()));
            return;
        }
        // The whitespace before `\v` moves after the end: `text<eid/> <v/>`.
        let children = context.children_mut();
        let mut space = false;
        if let Some(Inline::Text(text)) = children.last_mut()
            && text.ends_with(|c: char| c.is_ascii_whitespace())
        {
            space = true;
            let trimmed = text.trim_end_matches(|c: char| c.is_ascii_whitespace());
            if trimmed.is_empty() {
                children.pop();
            } else {
                // Span unchanged: it records the source run, not the
                // content. See `Text`.
                text.content = Cow::Owned(trimmed.to_string());
            }
        }
        children.push(verse_end(open.number));
        if space {
            children.push(Inline::Text(Text::synthesized(" ")));
        }
    }

    /// The pending verse end, if it was left by a container at `depth`.
    fn take_pending_verse_end(&mut self, depth: usize) -> Option<OpenVerse> {
        match self.pending_verse_end.take() {
            Some((open, at)) if at == depth => Some(open),
            other => {
                self.pending_verse_end = other;
                None
            }
        }
    }

    /// Add a parsed character style to its parent, placing a verse end
    /// that could not go inside it first. The style's own `open` entry has
    /// been popped, so it was one deeper than the current depth.
    fn add_char(&mut self, context: &mut ParserInlineContext<'a>, char: Char<'a>) {
        if let Some(open) = self.take_pending_verse_end(self.open.len() + 1) {
            self.place_verse_end(context, open);
        }
        if let Some(char) = self.fold_verse_number_style(context, char) {
            context.add_child(Inline::Char(char));
        }
    }

    /// `\va` and `\vp` directly after `\v N` are the verse's alternate and
    /// published numbers, not character styles: [`parse_verse`](Self::parse_verse)
    /// absorbs them, and so does a re-parse of anything a writer puts there —
    /// it has nowhere else to write `alt_number` and `pub_number` than right
    /// after the number. One that reaches here got past `parse_verse` because
    /// a marker between it and the `\v` was dropped (`\v 1\v\vp`: the second
    /// `\v` has no number and goes), so it is read the same way here. The
    /// round-trip fuzz target found it (ticket 27).
    ///
    /// Returns the style when it is not one of those, or when it is a `\vp`
    /// holding formatting — `parse_verse` keeps that one as a `Char` too, so
    /// it is written and read back as one.
    fn fold_verse_number_style(
        &mut self,
        context: &mut ParserInlineContext<'a>,
        char: Char<'a>,
    ) -> Option<Char<'a>> {
        let style = char.style.index();
        let alternate = style == self.va;
        if !alternate && style != self.vp {
            return Some(char);
        }
        if !matches!(context.children().last(), Some(Inline::VerseStart(_))) {
            return Some(char);
        }
        let span = char.span;
        if !alternate {
            // `\vp`: plain text is the published number, anything else stays
            // a `Char`, exactly as `parse_verse` reads it.
            let plain: Option<String> = char
                .children
                .iter()
                .map(|inline| match inline {
                    Inline::Text(text) => Some(text.content.as_ref()),
                    _ => None,
                })
                .collect();
            let Some(value) = plain else {
                return Some(char);
            };
            let value = value.trim_end_matches(|c: char| c.is_ascii_whitespace());
            if let Some(Inline::VerseStart(verse)) = context.children_mut().last_mut()
                && !value.is_empty()
            {
                verse.pub_number = Some(Cow::Owned(value.to_string()));
            }
            return None;
        }
        // `\va`: one word is the alternate number and the rest is ordinary
        // content, which is what `parse_verse`'s `eat_word` leaves behind.
        let mut children = char.children;
        let mut number = None;
        match children.first_mut() {
            Some(Inline::Text(text)) => {
                let content = text.content.as_ref();
                let end = content
                    .find(|c: char| c.is_ascii_whitespace())
                    .unwrap_or(content.len());
                match NumberList::parse_str(&content[..end]) {
                    Ok(parsed) => number = Some(parsed),
                    Err(_) => self.emit(
                        Code::MalformedVerseNumber,
                        span,
                        "`\\va` must be followed by a verse number",
                    ),
                }
                let rest = content[end..]
                    .trim_start_matches(|c: char| c.is_ascii_whitespace())
                    .to_string();
                if rest.is_empty() {
                    children.remove(0);
                } else {
                    text.content = Cow::Owned(rest);
                }
            }
            _ => self.emit(
                Code::MalformedVerseNumber,
                span,
                "`\\va` must be followed by a verse number",
            ),
        }
        if let Some(Inline::VerseStart(verse)) = context.children_mut().last_mut()
            && let Some(number) = number
        {
            verse.alt_number = Some(number);
        }
        for child in children {
            context.add_child(child);
        }
        None
    }

    /// A block is about to be pushed: a verse end that belongs before it
    /// goes in the last block before it that can hold one.
    fn place_pending_verse_end_before_block(&mut self, blocks: &mut [Block<'a>]) {
        if let Some(open) = self.take_pending_verse_end(0) {
            self.end_verse_in_last_block(blocks, open.number);
        }
    }

    /// Close the open verse at a block boundary (a chapter, end of input).
    fn end_verse_before_block(&mut self, blocks: &mut [Block<'a>]) {
        if !self.tracking_verses() {
            return;
        }
        // A pending end is always placed by the time its block is pushed;
        // one still here would be a parser bug, and the last block is the
        // best place for it.
        if let Some((open, _)) = self.pending_verse_end.take() {
            self.end_verse_in_last_block(blocks, open.number);
        }
        if let Some(open) = self.open_verse.take() {
            self.end_verse_in_last_block(blocks, open.number);
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
                    let starts_a_verse = para
                        .children
                        .iter()
                        .any(|inline| matches!(inline, Inline::VerseStart(_)));
                    if !para.children.is_empty()
                        && (starts_a_verse || self.rule(para.style.index()).is_verse_text())
                    {
                        para.children.push(verse_end(number));
                        return;
                    }
                }
                Block::Table(table) => {
                    if let Some(cell) = table
                        .rows
                        .last_mut()
                        .and_then(|row| row.cells.last_mut())
                    {
                        cell.children.push(verse_end(number));
                        return;
                    }
                }
                _ => {}
            }
        }
    }

    // ----- inline level ------------------------------------------------

    fn parse_inner_list(&mut self, context: &mut ParserInlineContext<'a>) -> InnerListCloser {
        loop {
            match self.cur_kind() {
                Kind::Eof => {
                    context.trim_trailing_whitespace();
                    return InnerListCloser::Eof;
                }
                Kind::Escape => {
                    // `\\`, `\|`, or `\"`: the literal is the second byte.
                    let literal = &self.cur_src()[1..];
                    let span = self.bump_span();
                    context.add_child(Inline::Text(Text::new(Cow::Borrowed(literal), span)));
                }
                Kind::Backslash => {
                    let text = self.cur_src();
                    let span = self.bump_span();
                    self.emit(
                        Code::StrayBackslash,
                        span,
                        "`\\` does not start a marker; kept as text",
                    );
                    context.add_child(Inline::Text(Text::new(Cow::Borrowed(text), span)));
                }
                Kind::MilestoneEnd => {
                    let span = self.bump_span();
                    self.emit(
                        Code::UnmatchedMilestoneEnd,
                        span,
                        "`\\*` with no open milestone",
                    );
                }
                Kind::Marker { nested, closing } => {
                    if let Some(closer) = self.parse_marker(context, nested, closing) {
                        // Trailing whitespace is dropped only where the text
                        // run ends at a paragraph-level marker (or EOF, above);
                        // before an inline marker it is content. Only the
                        // innermost context holds that run, and this trim is
                        // not recursive, so the containers this closer
                        // propagates through are untouched.
                        if closer.ends_block() {
                            context.trim_trailing_whitespace();
                        }
                        return closer;
                    }
                }
                Kind::Pipe => {
                    let span = self.bump_span();
                    if context.char_style().is_some() {
                        let attributes = self.parse_attributes(span);
                        context.set_attributes(attributes);
                    } else {
                        self.emit(
                            Code::UnexpectedPipe,
                            span,
                            "`|` outside a character style; kept as text",
                        );
                        context.add_child(Inline::Text(Text::new(Cow::Borrowed("|"), span)));
                    }
                }
                kind if kind.is_text() => {
                    context.add_child(Inline::Text(self.parse_text()));
                }
                Kind::OptBreak => {
                    let span = self.bump_span();
                    context.add_child(Inline::OptBreak(OptBreak { span }));
                }
                kind => unreachable!("token kind {kind:?} not handled"),
            }
        }
    }

    /// Handle a marker token at the current position. Returns a closer if
    /// the marker ends `context`.
    fn parse_marker(
        &mut self,
        context: &mut ParserInlineContext<'a>,
        nested: bool,
        closing: bool,
    ) -> Option<InnerListCloser> {
        let name = self.cur_marker_name();
        let span = self.bump_span();
        if !closing {
            self.eat_whitespace();
            if self.in_cell
                && let Ok(info) = TableCellInfo::from_str(name)
            {
                return Some(InnerListCloser::TableCell(info, span));
            }
        }
        let Some(marker) = self.resolve_marker(name) else {
            let name = name.to_string();
            if !closing && let Some(attributes) = self.take_unknown_milestone(&name, span) {
                let style = self.register_milestone(&name, span);
                context.add_child(Inline::Milestone(Milestone {
                    style: StyleId::new(style as u32),
                    attributes,
                    span: Span::new(span.start, self.prev_token_end()),
                }));
                return None;
            }
            self.emit_unknown_marker(&name, closing, span);
            return None;
        };
        let style_type = self.rule(marker).style_type.clone();

        if closing {
            match style_type {
                StyleType::Paragraph => self.emit(
                    Code::ParagraphMarkerClosed,
                    span,
                    format!("paragraph marker `\\{name}` cannot be closed with `*`"),
                ),
                StyleType::Character | StyleType::Note if self.is_open(marker) => {
                    return Some(InnerListCloser::ClosedMarker(marker));
                }
                StyleType::Character | StyleType::Note | StyleType::Milestone => self.emit(
                    Code::UnmatchedClosingMarker,
                    span,
                    format!("closing marker `\\{name}*` has no matching `\\{name}`"),
                ),
            }
            return None;
        }

        match style_type {
            StyleType::Paragraph => Some(InnerListCloser::Paragraph(marker, span)),
            StyleType::Character if name == "v" => self.parse_verse(context, span),
            StyleType::Character => self.parse_char(context, marker, span, nested),
            StyleType::Note => self.parse_note(context, marker, span),
            StyleType::Milestone => {
                self.parse_milestone(context, marker, span);
                None
            }
        }
    }

    /// `\v N`, with an optional `\va N\va*` (alternate number) and
    /// `\vp X\vp*` (published number) after it. Returns a closer only when
    /// an unclosed `\vp` ran into one.
    fn parse_verse(
        &mut self,
        context: &mut ParserInlineContext<'a>,
        marker_span: Span,
    ) -> Option<InnerListCloser> {
        // The one placement rule that changes the tree, and so the one the
        // parser keeps: a verse inside a note is dropped, marker and number
        // both, and what is not in the tree cannot be reported from it. Where
        // the verse *is* kept — before any `\c`, in a heading, inside a
        // character style — `usfm_semantic` reports it off the `VerseStart`
        // and the node it sits in (ticket 21).
        if self.note_depth > 0 {
            self.emit(
                Code::VerseInNote,
                marker_span,
                "`\\v` is not allowed inside a note",
            );
            self.eat_word();
            self.eat_whitespace();
            return None;
        }
        let number_span = self.cur_span();
        let Some(word) = self.eat_word() else {
            self.emit(
                Code::MissingVerseNumber,
                marker_span,
                "`\\v` must be followed by a verse number",
            );
            return None;
        };
        let number = match NumberList::parse_str(word) {
            Ok(number) => number,
            Err(_) => {
                self.emit(
                    Code::MalformedVerseNumber,
                    number_span,
                    format!("`{word}` is not a valid verse number"),
                );
                self.eat_whitespace();
                return None;
            }
        };
        if has_leading_zero(word) {
            self.emit(
                Code::NumberHasLeadingZero,
                number_span,
                format!("verse number `{word}` has a leading zero"),
            );
        }
        self.open_verse(context, &number);
        let mut verse = VerseStart {
            number,
            alt_number: None,
            pub_number: None,
            // Extended below, but only if a `\va`/`\vp` actually follows: the
            // whitespace eaten while looking for one is not part of the node.
            span: Span::new(marker_span.start, self.prev_token_end()),
        };
        let mut formatted_pub_number: Option<Char<'a>> = None;
        self.eat_whitespace();
        loop {
            if self.at_opening_marker("va") {
                let va_span = self.bump_span();
                self.eat_whitespace();
                let alt_span = self.cur_span();
                match self.eat_word().map(NumberList::parse_str) {
                    Some(Ok(alt)) => verse.alt_number = Some(alt),
                    Some(Err(_)) => self.emit(
                        Code::MalformedVerseNumber,
                        alt_span,
                        "`\\va` must be followed by a verse number",
                    ),
                    None => self.emit(
                        Code::MalformedVerseNumber,
                        va_span,
                        "`\\va` must be followed by a verse number",
                    ),
                }
                self.eat_whitespace();
                if !self.eat_closing_marker("va") {
                    self.emit(
                        Code::AlternateVerseNotClosed,
                        va_span,
                        "`\\va` is not closed by `\\va*`",
                    );
                }
                verse.span.end = self.prev_token_end();
                self.eat_whitespace();
            } else if self.at_opening_marker("vp")
                && let Some(vp) = self.resolve_marker("vp")
            {
                // `\vp` may carry formatting (`\vp \+it 21\+it*\vp*`), so it
                // is parsed as a character style. Plain text becomes the
                // published number; anything else stays a `Char`, which is
                // how Paratext writes it.
                let vp_span = self.bump_span();
                self.eat_whitespace();
                let mut inner = ParserInlineContext::Char(Char {
                    style: StyleId::new(vp as u32),
                    children: vec![],
                    attributes: None,
                    span: vp_span,
                });
                self.open.push(Open {
                    style: vp,
                    is_note: false,
                });
                let closer = self.parse_inner_list(&mut inner);
                self.open.pop();
                let ParserInlineContext::Char(mut char) = inner else {
                    unreachable!("context variant does not change");
                };
                char.span.end = self.container_end(&closer);
                let closed = matches!(closer, InnerListCloser::ClosedMarker(closed) if closed == vp);
                if !closed {
                    self.emit(
                        Code::AlternateVerseNotClosed,
                        vp_span,
                        "`\\vp` is not closed by `\\vp*`",
                    );
                }
                // An unclosed `\vp` is lifted too, though it has swallowed
                // whatever followed it and that is not really a published
                // number. The alternative was to leave it beside the verse as
                // a `Char`, and that is a tree no USFM spells: `\vp …\vp*`
                // after `\v N` *is* the published number, so the only source
                // that gives the `Char` is the unclosed one, and a writer
                // closing it — which every writer must — turns it back into a
                // published number. The round-trip fuzz target found that
                // (`\v 1\vp x`). Either way the swallowed text is lost from
                // the verse; this way the tree can be written.
                let plain: Option<String> = char
                    .children
                    .iter()
                    .map(|inline| match inline {
                        Inline::Text(text) => Some(text.content.as_ref()),
                        _ => None,
                    })
                    .collect();
                match plain {
                    Some(value) => {
                        let value = value.trim_end_matches(|c: char| c.is_ascii_whitespace());
                        if !value.is_empty() {
                            verse.pub_number = Some(Cow::Owned(value.to_string()));
                        }
                        verse.span.end = char.span.end;
                    }
                    None => formatted_pub_number = Some(char),
                }
                if !closed {
                    context.add_child(Inline::VerseStart(verse));
                    if let Some(char) = formatted_pub_number {
                        self.add_char(context, char);
                    }
                    return match closer {
                        // A sibling opened inside `\vp`: open it here instead.
                        InnerListCloser::OtherOpenedMarker(next, next_span) => {
                            self.parse_char(context, next, next_span, false)
                        }
                        closer => Some(closer),
                    };
                }
                self.eat_whitespace();
            } else {
                break;
            }
        }
        context.add_child(Inline::VerseStart(verse));
        if let Some(char) = formatted_pub_number {
            self.add_char(context, char);
        }
        None
    }

    /// Open a character style and parse its content. Handles implicit
    /// closure: a non-nested character marker inside a character style
    /// closes it and opens as a sibling.
    fn parse_char(
        &mut self,
        context: &mut ParserInlineContext<'a>,
        marker: usize,
        span: Span,
        nested: bool,
    ) -> Option<InnerListCloser> {
        let name = self.marker_name(marker);
        if nested && !context.is_char() {
            self.emit(
                Code::NestedMarkerNotNested,
                span,
                format!("`\\+{name}` is nested but no character style is open"),
            );
        }
        if !nested
            && !matches!(name.as_str(), "ref" | "fig")
            && context.is_implicitly_closed_by_char()
        {
            // Three conditions, all derived from what Paratext writes for
            // tcdocs: the style must be allowed to nest (`NEST` in its
            // `OccursUnder`, so never `\fr`/`\ft`/`\xo` …), the style it
            // would nest *into* must be one that holds more than plain text
            // ([`ParserImpl::parent_holds_plain_text`]), *and* it must be
            // closed by its own marker within the container. `\bk … \nd
            // Lord\nd* …\bk*` nests; `\xo 1.1 \xt Gen 1\x*` is a sibling
            // even though `\xt` may nest, because nothing closes it.
            if self.rule(marker).nest
                && !self.parent_holds_plain_text(context)
                && self.char_is_closed_ahead(&name)
            {
                self.emit(
                    Code::CharacterStyleNestedWithoutPlus,
                    span,
                    format!("`\\{name}` is nested without `+`; treated as `\\+{name}`"),
                );
            } else {
                return Some(InnerListCloser::OtherOpenedMarker(marker, span));
            }
        }

        let mut marker = marker;
        let mut span = span;
        loop {
            let closer = self.parse_char_body(context, marker, span);
            match closer {
                InnerListCloser::ClosedMarker(closed) if closed == marker => return None,
                InnerListCloser::OtherOpenedMarker(next_marker, next_span) => {
                    // Implicitly closed by a sibling opening. If we are
                    // ourselves inside a character style, that sibling
                    // closes it too.
                    if context.is_implicitly_closed_by_char() {
                        return Some(InnerListCloser::OtherOpenedMarker(next_marker, next_span));
                    }
                    marker = next_marker;
                    span = next_span;
                }
                other => return Some(other),
            }
        }
    }

    /// Whether the character style now open holds plain text, so nothing
    /// nests inside it without `\+`, whatever lies ahead. One of the three
    /// parts of the nesting decision in `parse_char`, and the only one that
    /// looks at the style being nested *into* rather than the one opening.
    ///
    /// `\xo`, a cross reference's origin reference, is the one such style the
    /// conformance roots name, and they name it in as many words:
    /// `paratextTests/NestingInCrossReferencesInvalid` is `\x + \xo 1.1 \em
    /// \+pn name\+pn* stuff \em*\x*`, its `metadata.xml` says "Grammar is
    /// accepting nesting of character styles under `\xo` - this is normally
    /// just text", and its reference USX closes the `\xo` at the `\em` and
    /// makes the two siblings. `paratextTests/CrossReferencesQuoteOutsideNote`
    /// (`\xo 1.1 \em stuff\em*`) and
    /// `paratextTests/CrossReferencesInsideCharacterMarker` (`\xo 1.1 \xq
    /// stuff\xq*`) read the same way, and no reference file in either root
    /// puts a `<char>` inside a `<char style="xo">`.
    ///
    /// It is deliberately this one style and not "a note-internal style
    /// inside a note-internal style": `biblica/CategoriesOnNotes` nests a
    /// closed `\xt` inside `\ft` and a `\ref` inside that `\xt`,
    /// `specExamples/extended/contentCatogories1` nests `\sc BC\sc*` inside
    /// `\ft`, and `usfmjsTests/usfmBodyTestD` nests `\dc` inside both `\ft`
    /// and `\xt` — the references want nesting everywhere except here
    /// (ticket 36).
    ///
    /// `\+xt` is untouched: an explicit `\+` is the author saying what they
    /// mean, and this path only decides the unmarked spelling.
    fn parent_holds_plain_text(&self, context: &ParserInlineContext<'a>) -> bool {
        context
            .char_style()
            .is_some_and(|style| self.rule(style.index()).marker == "xo")
    }

    /// Whether a `\name*` for the character style just opened lies ahead in
    /// the current container. The last of the three parts of the nesting
    /// decision in `parse_char`, and the only one that reads ahead.
    /// The scan stops at whatever would close the enclosing style first — a
    /// paragraph or cell marker, a closing marker of any open style, another
    /// `\name` (which would claim the closer) — or end of input, and leaves
    /// the lexer where it was.
    fn char_is_closed_ahead(&mut self, name: &str) -> bool {
        let checkpoint = self.lexer.checkpoint();
        let prev_token_end = self.prev_token_end;
        let closed = loop {
            self.bump_any();
            match self.cur_kind() {
                Kind::Eof => break false,
                Kind::Marker { closing: true, .. } => {
                    let cur = self.cur_marker_name();
                    let closes_open_style = self
                        .resolve_marker(cur)
                        .is_some_and(|style| self.open.iter().any(|open| open.style == style));
                    if closes_open_style {
                        break false;
                    }
                    if cur == name {
                        break true;
                    }
                }
                Kind::Marker {
                    closing: false,
                    nested: false,
                } => {
                    let cur = self.cur_marker_name();
                    if cur == name || (self.in_cell && TableCellInfo::from_str(cur).is_ok()) {
                        break false;
                    }
                    let is_paragraph = self
                        .resolve_marker(cur)
                        .is_some_and(|style| matches!(self.rule(style).style_type, StyleType::Paragraph));
                    if is_paragraph {
                        break false;
                    }
                }
                _ => {}
            }
        };
        self.lexer.rewind(checkpoint);
        self.prev_token_end = prev_token_end;
        closed
    }

    /// Parse the content of one character style and record how it ended.
    fn parse_char_body(
        &mut self,
        context: &mut ParserInlineContext<'a>,
        marker: usize,
        span: Span,
    ) -> InnerListCloser {
        let mut inner = ParserInlineContext::Char(Char {
            style: StyleId::new(marker as u32),
            children: vec![],
            attributes: None,
            span,
        });
        self.open.push(Open {
            style: marker,
            is_note: false,
        });
        let closer = self.parse_inner_list(&mut inner);
        self.open.pop();
        let ParserInlineContext::Char(mut char) = inner else {
            unreachable!("context variant does not change");
        };
        char.span.end = self.container_end(&closer);
        // A `\w` with attributes and no text (`empty-word`) is the node as
        // written, so `usfm_semantic` reads it off the tree (ticket 21).
        self.add_char(context, char);

        let name = self.marker_name(marker);
        let not_closed_code = if name == "fig" {
            Code::FigureNotClosed
        } else {
            Code::CharacterStyleNotClosed
        };
        match &closer {
            InnerListCloser::ClosedMarker(closed) if *closed == marker => {}
            // Closed by the enclosing note's end marker: the normal idiom.
            InnerListCloser::ClosedMarker(closed) if self.rule(*closed).is_note() => {}
            InnerListCloser::ClosedMarker(closed) => {
                let by = self.marker_name(*closed);
                self.emit(
                    not_closed_code,
                    span,
                    format!("`\\{name}` was closed by `\\{by}*` instead of `\\{name}*`"),
                );
            }
            // Implicit closure by a sibling: the normal idiom inside notes.
            InnerListCloser::OtherOpenedMarker(..) if self.note_depth > 0 => {}
            InnerListCloser::OtherOpenedMarker(next, _) => {
                let by = self.marker_name(*next);
                self.emit(
                    Code::CharacterStyleImplicitlyClosed,
                    span,
                    format!("`\\{name}` was implicitly closed by `\\{by}`"),
                );
            }
            InnerListCloser::Paragraph(next, _) => {
                let by = self.marker_name(*next);
                self.emit(
                    not_closed_code,
                    span,
                    format!("`\\{name}` was still open at `\\{by}`"),
                );
            }
            InnerListCloser::TableCell(..) => self.emit(
                not_closed_code,
                span,
                format!("`\\{name}` was still open at the next table cell"),
            ),
            InnerListCloser::Eof => self.emit(
                not_closed_code,
                span,
                format!("`\\{name}` was still open at end of input"),
            ),
        }
        closer
    }

    fn parse_note(
        &mut self,
        context: &mut ParserInlineContext<'a>,
        marker: usize,
        span: Span,
    ) -> Option<InnerListCloser> {
        let name = self.marker_name(marker);
        let caller = match self.eat_note_caller() {
            Some(word) => Caller::from(word),
            None => {
                self.emit(
                    Code::MissingNoteCaller,
                    span,
                    format!(
                        "`\\{name}` must be followed by a caller (`+`, `-`, or custom); `+` assumed"
                    ),
                );
                Caller::Plus
            }
        };
        self.eat_whitespace();
        let mut inner = ParserInlineContext::Note(Note {
            style: StyleId::new(marker as u32),
            caller,
            category: None,
            children: vec![],
            span,
        });
        self.open.push(Open {
            style: marker,
            is_note: true,
        });
        self.note_depth += 1;
        let closer = self.parse_inner_list(&mut inner);
        self.note_depth -= 1;
        self.open.pop();
        let ParserInlineContext::Note(mut note) = inner else {
            unreachable!("context variant does not change");
        };
        note.category = self.take_category(&mut note.children);
        note.span.end = self.container_end(&closer);
        context.add_child(Inline::Note(note));

        match closer {
            InnerListCloser::ClosedMarker(closed) if closed == marker => None,
            closer => {
                self.emit(
                    Code::NoteNotClosed,
                    span,
                    format!("`\\{name}` is not closed by `\\{name}*`"),
                );
                Some(closer)
            }
        }
    }

    /// Parse the body of a known milestone: `\marker |attr="value"\*` or
    /// `\marker\*`. The marker itself has already been consumed.
    ///
    /// Milestones occur both inside a paragraph and between blocks, so this
    /// returns the node and leaves placing it to the caller.
    fn parse_milestone_node(&mut self, marker: usize, span: Span) -> Milestone<'a> {
        // USFM 3 spells the list `\qt-s |who="…"\*`, with a space before the
        // pipe, and that space is the marker's own whitespace like any other
        // marker's. The inline path has eaten it by now (`parse_marker`); the
        // block path calls straight in, so eat it here and the two agree
        // (ticket 29).
        let checkpoint = self.lexer.checkpoint();
        let diagnostics_len = self.diagnostics.len();
        self.eat_whitespace();
        let pipe_span = self.cur_span();
        let attributes = self
            .eat(Kind::Pipe)
            .then(|| self.parse_attributes(pipe_span));
        self.eat_whitespace();
        if !self.eat(Kind::MilestoneEnd) {
            let name = self.marker_name(marker);
            // A marker registered by an earlier unclosed `\zaln-s` is known
            // by now, so the old format's later lines arrive here rather
            // than at `take_unknown_milestone`: read them the same way.
            if attributes.is_some() {
                let parsed = self.lexer.checkpoint();
                let parsed_end = self.prev_token_end;
                self.lexer.rewind(checkpoint);
                let rest = self.diagnostics.split_off(diagnostics_len);
                if let Some(line_bounded) = self.take_unclosed_milestone(&name, span) {
                    return Milestone {
                        style: StyleId::new(marker as u32),
                        attributes: line_bounded,
                        span: Span::new(span.start, self.prev_token_end()),
                    };
                }
                self.lexer.rewind(parsed);
                self.prev_token_end = parsed_end;
                self.diagnostics.extend(rest);
            }
            self.emit(
                Code::MilestoneNotClosed,
                span,
                format!("milestone `\\{name}` is not closed by `\\*`"),
            );
        }
        Milestone {
            style: StyleId::new(marker as u32),
            attributes,
            span: Span::new(span.start, self.prev_token_end()),
        }
    }

    fn parse_milestone(
        &mut self,
        context: &mut ParserInlineContext<'a>,
        marker: usize,
        span: Span,
    ) {
        let milestone = self.parse_milestone_node(marker, span);
        context.add_child(Inline::Milestone(milestone));
    }

    /// After an opening marker the stylesheet does not contain: if what
    /// follows is `[|attributes] [whitespace] \*`, the marker is a milestone
    /// whatever the stylesheet says. Consume it and return its attributes.
    /// Otherwise consume nothing and return `None`.
    ///
    /// The two `Option`s answer different questions: the outer one is "was
    /// this a milestone at all", the inner one is `Milestone::attributes` —
    /// `None` for a marker with no `|`, `Some` with no pairs for `\zaln-s |\*`.
    /// Ticket 18 found that collapsing them lost the second distinction, which
    /// is the one `empty-milestone-attribute-list` reads.
    ///
    /// Milestones are an open set in practice (`\zaln-s` from unfoldingWord
    /// alignment, `\ts` from tStudio), and the syntax alone says what the
    /// marker is, so the parser can keep the node and all its attributes
    /// rather than dropping them.
    fn take_unknown_milestone(&mut self, name: &str, span: Span) -> Option<Option<Attributes<'a>>> {
        let checkpoint = self.lexer.checkpoint();
        let diagnostics_len = self.diagnostics.len();
        // As in `parse_milestone_node`: the space before the pipe belongs to
        // the marker (ticket 29). The checkpoint is taken first, so a marker
        // that turns out not to be a milestone gives its whitespace back.
        self.eat_whitespace();
        let pipe_span = self.cur_span();
        let attributes = self
            .eat(Kind::Pipe)
            .then(|| self.parse_attributes(pipe_span));
        self.eat_whitespace();
        if self.eat(Kind::MilestoneEnd) {
            return Some(attributes);
        }
        self.lexer.rewind(checkpoint);
        self.diagnostics.truncate(diagnostics_len);
        self.take_unclosed_milestone(name, span)
    }

    /// The pre-USFM-3 spelling of a start milestone that usfm-js's
    /// "old format" writes (ticket 43): `\zaln-s |x-strong="G35880"` with the
    /// attribute list running to the end of the line and no `\*` at all.
    /// A `-s` marker followed by `|` whose list reaches a line break (or the
    /// end of input) is read as closed there, keeps its attributes, and is
    /// `milestone-not-closed` on the marker. The line break is left alone,
    /// so the tree is the one the closed spelling gives.
    ///
    /// Without this the start was dropped while its `-e` was kept, and the
    /// list became verse text, `|` and all: 19 140 `unexpected-pipe` Errors
    /// over unfoldingWord's `large.usfm` as upstream wrote it. Anything else
    /// — no `|`, a list that stops at a marker on the same line, a name
    /// without `-s` — consumes nothing and returns `None`.
    fn take_unclosed_milestone(&mut self, name: &str, span: Span) -> Option<Option<Attributes<'a>>> {
        if !Style::new(name).is_start() {
            return None;
        }
        let checkpoint = self.lexer.checkpoint();
        let prev_token_end = self.prev_token_end;
        let diagnostics_len = self.diagnostics.len();
        self.eat_whitespace();
        let pipe_span = self.cur_span();
        if self.eat(Kind::Pipe) {
            let attributes = self.parse_line_bounded_attributes(pipe_span);
            if matches!(self.cur_kind(), Kind::Eof | Kind::Whitespace { has_newline: true }) {
                self.emit(
                    Code::MilestoneNotClosed,
                    span,
                    format!("milestone `\\{name}` is not closed by `\\*`; closed at the end of the line"),
                );
                return Some(Some(attributes));
            }
        }
        self.lexer.rewind(checkpoint);
        self.prev_token_end = prev_token_end;
        self.diagnostics.truncate(diagnostics_len);
        None
    }

    fn parse_line_bounded_attributes(&mut self, pipe: Span) -> Attributes<'a> {
        self.line_bounded_attributes = true;
        let attributes = self.parse_attributes(pipe);
        self.line_bounded_attributes = false;
        attributes
    }

    /// Register a milestone style the base stylesheet does not contain and
    /// return its id, reporting the registration once per marker name.
    ///
    /// This is what D3 buys: the document owns the extended sheet, so an id
    /// for a style the caller's sheet never had still resolves.
    fn register_milestone(&mut self, name: &str, span: Span) -> usize {
        if let Some(index) = self.style_sheet.get_marker_index(name) {
            return *index;
        }

        // `\k-s`/`\k-e` are the milestone forms of a marker the stylesheet
        // does define (`\k`). Those are ordinary USFM and not worth a
        // diagnostic; inherit the base marker's text type.
        let style = Style::new(name);
        let base = style.without_milestone();
        let derived_from_base = (style.is_start() || style.is_end())
            .then(|| self.style_sheet.get_rule_by_marker(base))
            .flatten();

        let text_type = match derived_from_base {
            Some(rule) => rule.text_type.clone(),
            None => {
                if name.starts_with('z') {
                    // The `\z` namespace is how USFM sanctions custom markers.
                    self.emit(
                        Code::UnknownCustomMilestone,
                        span,
                        format!("custom milestone `\\{name}` is not in the stylesheet"),
                    );
                } else {
                    self.emit(
                        Code::UnknownMilestone,
                        span,
                        format!("milestone `\\{name}` is not in the stylesheet"),
                    );
                }
                TextType::Other
            }
        };

        let rule = StyleRule {
            marker: name.to_string(),
            // A derived rule has no sheet entry to take documentation from.
            name: None,
            description: None,
            style_type: StyleType::Milestone,
            text_type,
            text_properties: TextProperties::default(),
            nest: false,
            occurs_under: vec![],
        };
        Arc::make_mut(&mut self.style_sheet).add_rule(rule)
    }

    /// Parse a run of text tokens. Whitespace runs other than a single
    /// space are normalised to a single space, and `~` becomes a no-break
    /// space (U+00A0), which is what it means in USFM. The content stays
    /// borrowed from the source unless one of those rewrites applies.
    fn parse_text(&mut self) -> Text<'a> {
        let start = self.cur_span().start;
        loop {
            match self.cur_kind() {
                Kind::Whitespace { .. } if self.cur_src() != " " => {
                    let end = self.cur_span().start;
                    let mut owned = self.source_text[start as usize..end as usize].to_string();
                    self.bump_any();
                    owned.push(' ');
                    return self.parse_text_owned(start, owned);
                }
                Kind::Word if self.cur_src().contains('~') => {
                    let end = self.cur_span().start;
                    let mut owned = self.source_text[start as usize..end as usize].to_string();
                    owned.push_str(&self.cur_src().replace('~', NO_BREAK_SPACE));
                    self.bump_any();
                    return self.parse_text_owned(start, owned);
                }
                kind if kind.is_text() => {
                    self.bump_any();
                }
                _ => break,
            }
        }
        let end = self.cur_span().start;
        Text::new(
            Cow::Borrowed(&self.source_text[start as usize..end as usize]),
            Span::new(start, end),
        )
    }

    fn parse_text_owned(&mut self, run_start: u32, mut owned: String) -> Text<'a> {
        let mut start = self.cur_span().start;
        loop {
            match self.cur_kind() {
                Kind::Whitespace { .. } if self.cur_src() != " " => {
                    let end = self.cur_span().start;
                    owned.push_str(&self.source_text[start as usize..end as usize]);
                    owned.push(' ');
                    self.bump_any();
                    start = self.cur_span().start;
                }
                Kind::Word if self.cur_src().contains('~') => {
                    let end = self.cur_span().start;
                    owned.push_str(&self.source_text[start as usize..end as usize]);
                    owned.push_str(&self.cur_src().replace('~', NO_BREAK_SPACE));
                    self.bump_any();
                    start = self.cur_span().start;
                }
                kind if kind.is_text() => {
                    self.bump_any();
                }
                _ => break,
            }
        }
        let end = self.cur_span().start;
        owned.push_str(&self.source_text[start as usize..end as usize]);
        // The content is normalised, so it is shorter than the source it came
        // from; the span still covers the whole run that was read.
        Text::new(Cow::Owned(owned), Span::new(run_start, end))
    }

    /// Parse attributes after a `|`, whose span the caller has already
    /// consumed and passes in: an empty list has nothing else to point at.
    /// Format: `name="value"` or just `value` for the default attribute.
    /// Multiple attributes are whitespace-separated.
    fn parse_attributes(&mut self, pipe: Span) -> Attributes<'a> {
        let mut pairs = Vec::new();
        let mut newline_reported = false;

        loop {
            if let Kind::Whitespace { has_newline: true } = self.cur_kind()
                && self.in_periph_title
            {
                // A paragraph-level attribute list ends with its line, and
                // the line break belongs to it, not to the title text.
                self.bump_any();
                break;
            }
            if let Kind::Whitespace { has_newline: true } = self.cur_kind()
                && self.line_bounded_attributes
            {
                break;
            }
            if let Kind::Whitespace { has_newline: true } = self.cur_kind()
                && !newline_reported
            {
                newline_reported = true;
                let span = self.cur_span();
                self.emit(
                    Code::NewlineInAttributes,
                    span,
                    "line break inside an attribute list",
                );
            }
            self.eat_whitespace();

            match self.cur_kind() {
                Kind::Word if self.lexer.peek().kind == Kind::Equal => {
                    // Named attribute: name="value". Whether the name is one
                    // USX can carry, and whether it repeats one already in the
                    // list, are `usfm_semantic`'s to report: both keep the
                    // pair as written, so nothing here depends on the answer.
                    let word = self.cur_src();
                    let name_span = self.cur_span();
                    self.bump_any();
                    self.bump_any();
                    let value = self.parse_attribute_value();
                    pairs.push(Attribute {
                        name: Cow::Borrowed(word),
                        value,
                        span: name_span,
                    });
                }
                Kind::Word | Kind::DoubleQuote | Kind::OptBreak => {
                    // Default attribute: the bare value, verbatim, up to a
                    // `word=` that starts a named attribute. Quotes are part
                    // of it (`\w x|"y"\w*` has `lemma="&quot;y&quot;"` in
                    // Paratext), as is trailing whitespace.
                    let start = self.cur_span().start;
                    while matches!(
                        self.cur_kind(),
                        Kind::Whitespace { .. } | Kind::Word | Kind::DoubleQuote | Kind::OptBreak
                    ) {
                        if (self.in_periph_title || self.line_bounded_attributes)
                            && matches!(self.cur_kind(), Kind::Whitespace { has_newline: true })
                        {
                            break;
                        }
                        if self.at(Kind::Word) && self.lexer.peek().kind == Kind::Equal {
                            break;
                        }
                        self.bump_any();
                    }
                    let end = self.cur_span().start;
                    pairs.push(Attribute {
                        name: Cow::Borrowed(""),
                        value: Cow::Borrowed(&self.source_text[start as usize..end as usize]),
                        span: Span::new(start, end),
                    });
                }
                _ => break,
            }
        }

        // A `\periph` attribute list ends with its line, so trailing ASCII
        // whitespace in a default value that ends the list belongs to the
        // line, not to the value — and no writer can put it back: what it
        // writes ends in a line break, which reads back without it. Every
        // other list ends at a marker (`\w*`, `\*`) with the whitespace
        // safely inside the line, and a default value that is *not* last keeps
        // the space that separates it from the pair after it, which is why
        // only the last one is trimmed. The round-trip fuzz target found both
        // spellings this bites on, `\periph|: ` and `\periph|s \`
        // (ticket 27).
        if self.in_periph_title
            && let Some(last) = pairs.last_mut()
            && last.name.is_empty()
        {
            let trimmed = last
                .value
                .trim_end_matches(|c: char| c.is_ascii_whitespace())
                .len() as u32;
            let end = last.span.start + trimmed;
            last.value = Cow::Borrowed(&self.source_text[last.span.start as usize..end as usize]);
            last.span.end = end;
        }
        Attributes { pairs, pipe }
    }

    /// The text of a quoted attribute value, with `\\`, `\|`, and `\"`
    /// escapes resolved when present.
    fn attribute_value_text(&self, start: u32, end: u32, has_escapes: bool) -> Cow<'a, str> {
        let raw = &self.source_text[start as usize..end as usize];
        if !has_escapes {
            return Cow::Borrowed(raw);
        }
        let mut out = String::with_capacity(raw.len());
        let mut chars = raw.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\\' && matches!(chars.peek(), Some('\\' | '|' | '"')) {
                continue;
            }
            out.push(c);
        }
        Cow::Owned(out)
    }

    /// Parse an attribute value, handling quoted strings.
    fn parse_attribute_value(&mut self) -> Cow<'a, str> {
        if self.at(Kind::DoubleQuote) {
            let quote_span = self.bump_span();
            let start = self.cur_span().start;
            let mut has_escapes = false;
            loop {
                match self.cur_kind() {
                    Kind::DoubleQuote => {
                        let end = self.cur_span().start;
                        self.bump_any();
                        return self.attribute_value_text(start, end, has_escapes);
                    }
                    Kind::Escape => {
                        has_escapes = true;
                        self.bump_any();
                    }
                    Kind::Eof | Kind::Marker { .. } | Kind::MilestoneEnd | Kind::Backslash => {
                        let end = self.cur_span().start;
                        self.emit(
                            Code::UnterminatedAttributeValue,
                            quote_span,
                            "attribute value is missing its closing `\"`",
                        );
                        return self.attribute_value_text(start, end, has_escapes);
                    }
                    _ => {
                        self.bump_any();
                    }
                }
            }
        } else {
            let span = self.cur_span();
            match self.eat_word() {
                Some(word) => {
                    // Unquoted value: read the one word, but say so.
                    self.emit(
                        Code::AttributeValueNotQuoted,
                        span,
                        "attribute value must be in double quotes",
                    );
                    Cow::Borrowed(word)
                }
                None => {
                    self.emit(
                        Code::MissingAttributeValue,
                        Span::empty(span.start),
                        "`=` is not followed by a value",
                    );
                    Cow::Borrowed("")
                }
            }
        }
    }
}
