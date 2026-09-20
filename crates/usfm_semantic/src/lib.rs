//! Semantic checks over a parsed USFM document.
//!
//! The split with the parser is a split of jobs, not of subject matter
//! (`.scratch/oxc-layout/spec.md`, M4):
//!
//! * the **parser** repairs. When the input is not USFM it has to put
//!   *something* in the tree — drop a marker, close a style, open an implicit
//!   `\p` — and it reports that repair so the caller can see the tree is a
//!   guess. A [`Code`] the parser owns always answers "what does the tree hold
//!   instead?".
//! * this crate **reports**. A check here reads the finished tree and the
//!   stylesheet it resolves against, and says something about a document that
//!   parsed exactly as written: a marker in a place its `OccursUnder` does not
//!   list, a table column out of order, a verse in a heading, a book code USX
//!   accepts but USFM does not list. Nothing here changes the tree, and none
//!   of these codes has a "recovery" to describe.
//!
//! The practical test for where a check belongs, and the audit of every code
//! against it, are in `usfm_diagnostics`'s module documentation (ticket 21). In
//! short: a check is the parser's if deleting it would change the tree, or if
//! what the author wrote is no longer visible in the tree — `verse-in-note`
//! drops the verse, `number-has-leading-zero` parses `01` to the number 1, and
//! neither is there to be found afterwards. Everything else is this crate's,
//! and [`EMITS`] is the list, kept in step with [`Code::is_semantic`] by a
//! test.
//!
//! [`ReferenceIndex`] is here for the same reason (ticket 22): the chapters and
//! verses of a document are *derived* from the tree, not part of it, and the
//! checks that talk about them are this crate's. It is the one thing here that
//! is not a check — a caller that only wants "the content of Genesis 1:2" can
//! build one and ignore [`analyze`] — and [`analyze`] does not build one: the
//! order checks (ticket 23) need a chapter number, a verse number and a span,
//! all of which the walk already passes, where the index costs a second
//! traversal and a [`usfm_ast::NodePath`] per verse.
//!
//! [`analyze`] is the whole API of the checks. Callers usually reach it through
//! `usfm::parse`, which merges these diagnostics with the parser's; a caller
//! that has a `Document` from somewhere else — an editor holding a cached
//! tree, a test — can call it directly.
//!
//! ```
//! // The parser alone reports nothing about `\id ZZZ`; the check is here.
//! let result = usfm::parser::parser::Parser::new("\\id ZZZ\n\\c 1\n\\p \\v 1 a\n")
//!     .parse(&usfm::DEFAULT_STYLESHEET);
//! assert!(result.diagnostics.is_empty());
//!
//! let diagnostics = usfm_semantic::analyze(&result.document);
//! assert_eq!(diagnostics[0].code, usfm::Code::UnlistedBookCode);
//! ```
//!
//! **Spans.** A check reports the span of the node it read. Where the node's
//! span is wider than the thing being reported — a [`Book`] carries one span
//! over the whole `\id` line, not one for the code; a [`Char`] runs from its
//! opening marker to wherever it closed — the node's span is what is
//! reported: the source text is not available here, and narrowing by
//! arithmetic (`span.start + 4`) would assume a single space after the marker
//! and point at whitespace whenever the input has two. Where the tree does
//! carry the narrower offset the narrower one is used, which is why
//! [`usfm_ast::Attributes`] keeps the `|` and each [`usfm_ast::Attribute`] its
//! name: an attribute diagnostic points at the attribute, exactly as it did
//! when the parser reported it mid-parse.

pub mod reference;

pub use reference::{ChapterRef, ReferenceIndex, VerseRef};

use std::collections::BTreeSet;

use usfm_ast::visit::{Visit, walk_note, walk_para, walk_table_cell};
use usfm_ast::{
    Attributes, Block, Book, BookCode, ChapterStart, Char, Document, Milestone, Note, NumberList,
    NumberRange, Para, Periph, Sidebar, StyleId, Table, TableCell, VerseStart,
    default_attribute_name, is_valid_attribute_name,
};
use usfm_diagnostics::{Code, Diagnostic};
use usfm_span::Span;
use usfm_style::{StyleSheet, TextType};

/// Every [`Code`] this crate can report.
///
/// The list is the crate's half of the split recorded in
/// [`Code::is_semantic`]: `analyze` emits these and nothing else, and
/// `semantic_emits_exactly_the_semantic_codes` asserts the two agree in both
/// directions, so a check moved here without its line in `is_semantic` — or a
/// code marked semantic that nothing here reports — fails the build.
pub const EMITS: &[Code] = &[
    // ticket 19
    Code::UnlistedBookCode,
    // ticket 20
    Code::MarkerNotAllowedHere,
    Code::MarkerNotListedHere,
    Code::EmptyAttributeList,
    Code::EmptyMilestoneAttributeList,
    Code::NoDefaultAttribute,
    Code::DefaultAttributeWithOthers,
    Code::MalformedAttributeName,
    Code::DuplicateAttribute,
    // ticket 21
    Code::MissingId,
    Code::IdNotFirst,
    Code::EmptyBook,
    Code::VerseTextBeforeChapter,
    Code::VerseOutsideChapter,
    Code::VerseInHeading,
    Code::VerseInCharacterStyle,
    Code::UnexpectedTableColumn,
    Code::EmptyWord,
    // ticket 23
    Code::DuplicateVerseNumber,
    Code::VerseOutOfOrder,
    Code::DuplicateChapterNumber,
    Code::ChapterOutOfOrder,
];

/// Run every semantic check over `document`.
///
/// The returned diagnostics are sorted by `span.start`, stably: two checks
/// that report at the same offset keep the order they were emitted in, which
/// is the order of the walk.
pub fn analyze(document: &Document) -> Vec<Diagnostic> {
    let mut analyzer = Analyzer::new(document.style_sheet());
    analyzer.visit_document(document);
    let mut diagnostics = analyzer.diagnostics;
    diagnostics.sort_by_key(|diagnostic| diagnostic.span.start);
    diagnostics
}

/// One walk of the tree, running every check.
///
/// The checks share a visitor rather than each taking their own pass: a check
/// is a few lines in a `visit_*` method, and the tree is walked once however
/// many there are. Each one is a private method named after the [`Code`] it
/// reports, or after the rule where one rule chooses between codes
/// (`check_placement`, `check_attributes`), so the mapping from the recovery
/// table to the code that implements it stays easy to follow.
///
/// [`Scope`] is the only state besides the diagnostics: what the walk is
/// inside, which is what the parser read off its stack of open markers.
struct Analyzer<'a> {
    /// The sheet the document's `StyleId`s resolve against — the parser's
    /// extended with anything it had to derive (hardening plan D3), never
    /// `DEFAULT_STYLESHEET`.
    style_sheet: &'a StyleSheet,
    /// What the walk is inside, innermost last; see [`Scope`].
    scopes: Vec<Scope>,
    /// Whether the block being visited is the first of the block list it is
    /// in. `id-not-first` is exactly "a `Book` for which this is false", which
    /// is the rule the parser applied to the list it was appending to.
    first_in_container: bool,
    /// The book in force, from the last `\id` the walk passed. `None` until
    /// the first one: a document with no `\id` has no book to be wrong about,
    /// which is how the parser read it too.
    book: Option<BookCode>,
    /// Whether a `\c` has been passed, anywhere, at any depth.
    chapter_seen: bool,
    /// The numbers the order checks compare against; see [`Order`].
    order: Order,
    diagnostics: Vec<Diagnostic>,
}

/// What the order checks (ticket 23) remember as the walk goes.
///
/// The four codes are the only ones here that are not about a node on its own:
/// a verse repeats or runs backwards *relative to the other verses of its
/// chapter*, and a chapter relative to the other chapters of its book. That is
/// all the state it takes, and the walk passes every `\id`, `\c` and `\v` in
/// document order already — which is why this rides on the walk rather than on
/// a second pass over a [`ReferenceIndex`] (the index is still the thing to
/// build to *read* verses; it costs a traversal and a path per verse, which
/// these checks have no use for).
#[derive(Default)]
struct Order {
    /// Chapter numbers passed in the current book, for the duplicate rule.
    chapters_seen: BTreeSet<usize>,
    /// The last chapter number passed in the current book, for the order rule.
    previous_chapter: Option<usize>,
    /// The chapter in force, or `None` before the first `\c` of the book —
    /// which is when a verse is not checked at all.
    chapter_number: Option<usize>,
    /// Verse numbers passed in the current chapter.
    covered: Coverage,
    /// The end of the last verse passed in the current chapter.
    previous_verse: Option<VerseKey>,
}

impl Order {
    /// A new book: its chapters are its own, and no chapter is open yet.
    fn book(&mut self) {
        *self = Order::default();
    }

    /// A new chapter: its verses are its own.
    fn chapter(&mut self, number: usize) {
        self.chapter_number = Some(number);
        self.previous_chapter = Some(number);
        self.covered = Coverage::default();
        self.previous_verse = None;
    }
}

/// One level of the containment the placement check reads.
///
/// Only the four kinds of container that change the answer are on the stack.
/// Blocks that hold blocks (a sidebar, a periph) are not: whatever they
/// contain is in a paragraph of its own, which is the parent that counts.
#[derive(Clone, Copy, PartialEq)]
enum Scope {
    /// A paragraph: the parent of the notes and of the outermost character
    /// styles it holds.
    Para(StyleId),
    /// A character style. Its content is governed by `NEST` rather than by
    /// `OccursUnder`, so a character style inside one is not checked.
    Char,
    /// A note: the parent of the character styles inside it, and transparent
    /// to a note, whose parent is the paragraph either way.
    Note(StyleId),
    /// A table cell, whose content is not placement-checked at all: the cell
    /// markers are not in any `OccursUnder` list.
    Cell,
}

impl<'a> Analyzer<'a> {
    fn new(style_sheet: &'a StyleSheet) -> Self {
        Self {
            style_sheet,
            scopes: Vec::new(),
            first_in_container: true,
            book: None,
            chapter_seen: false,
            order: Order::default(),
            diagnostics: Vec::new(),
        }
    }

    fn emit(&mut self, code: Code, span: Span, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic::new(code, span, message));
    }

    fn in_scope(&mut self, scope: Scope, f: impl FnOnce(&mut Self)) {
        self.scopes.push(scope);
        f(self);
        self.scopes.pop();
    }

    /// `unlisted-book-code`: a code that is well formed — `book@code` in
    /// `usx.rnc` allows `[A-Z][A-Z0-9]{2}` and two numeric shapes beyond the
    /// book list — but is not one of the books [`usfm_ast::BookCode`] names.
    ///
    /// Nothing is repaired: the parser keeps the book as `BookCode::Other` and
    /// every writer emits it verbatim (`<book code="TST">`). It is reported
    /// because a typo in a real code (`ZZZ` for `ZEC`) has exactly this shape,
    /// which is a judgement about the document and so belongs here rather than
    /// in the parser.
    ///
    /// The span is the whole `\id` line: see the note on spans in the module
    /// documentation.
    fn check_unlisted_book_code(&mut self, book: &Book<'_>) {
        if book.code.is_listed() {
            return;
        }
        self.diagnostics.push(Diagnostic::new(
            Code::UnlistedBookCode,
            book.span,
            format!(
                "`{}` is not one of the book codes USFM lists; kept as written",
                book.code
            ),
        ));
    }

    /// `missing-id` / `empty-book`: what the document's own block list says
    /// about it, which is all these two rules ever read.
    ///
    /// A document that starts with anything but `\id` is missing one, reported
    /// at offset 0 — where the `\id` should have been, not where the block that
    /// stands there begins. A document whose only block is the book is empty:
    /// that one is reported at the end of the source, which is why
    /// [`Document`] carries a span of its own (ticket 21). An empty document
    /// reports neither: there is no first block to be wrong about.
    ///
    /// The list is the top-level one. A `\id` nested in a sidebar is not a
    /// document's first block whatever it is doing there, and `id-not-first`
    /// below has it covered.
    fn check_document(&mut self, document: &Document<'_>) {
        match document.blocks.first() {
            Some(Block::Book(_)) => {
                if document.blocks.len() == 1 {
                    self.emit(
                        Code::EmptyBook,
                        Span::empty(document.span.end),
                        "book contains only an `\\id` line",
                    );
                }
            }
            Some(_) => self.emit(
                Code::MissingId,
                Span::empty(0),
                "document must start with `\\id`",
            ),
            None => {}
        }
    }

    /// `id-not-first`: a `\id` after other content.
    ///
    /// Nothing is repaired — the book is in the tree like any other — so the
    /// rule is just "this `Book` is not the first block of its list", which is
    /// what the parser asked of the list it was appending to.
    ///
    /// The span is the `Book` node: the whole `\id` line, where the parser
    /// reported the marker alone. Same start, wider end (see the note on spans
    /// above).
    fn check_id_not_first(&mut self, book: &Book<'_>) {
        if self.first_in_container {
            return;
        }
        self.emit(
            Code::IdNotFirst,
            book.span,
            "`\\id` must be the first marker in the file",
        );
    }

    /// `verse-text-before-chapter`: a paragraph of verse text standing before
    /// the first `\c` of a scripture book.
    ///
    /// Introductory matter (`\ip`, `\imt`) is not verse text and is where it
    /// belongs; a peripheral book (`\id FRT`) has no chapters at all, so the
    /// rule does not apply to one. Nothing is repaired: the paragraph is in
    /// the tree with the style it was written with, which with the book and
    /// the chapters is everything the rule reads.
    ///
    /// The span is the paragraph node; the parser reported its marker.
    fn check_verse_text_before_chapter(&mut self, para: &Para<'_>) {
        if self.chapter_seen || !self.book.is_some_and(|book| !book.is_non_scripture()) {
            return;
        }
        let rule = self.style_sheet.get_rule(para.style.index());
        if !rule.is_verse_text() {
            return;
        }
        let marker = rule.marker.clone();
        self.emit(
            Code::VerseTextBeforeChapter,
            para.span,
            format!("`\\{marker}` (verse text) before the first `\\c`"),
        );
    }

    /// `verse-outside-chapter` / `verse-in-heading` / `verse-in-character-style`:
    /// a verse marker the parser kept, somewhere a verse does not belong.
    ///
    /// All three read the same two things: the `VerseStart` in the tree and
    /// what it sits in. The fourth placement rule, `verse-in-note`, stays with
    /// the parser — a verse in a note is dropped, and a check here cannot
    /// report a node that is not there.
    ///
    /// The heading rule asks the *nearest enclosing paragraph*, so a verse in
    /// a table cell is not in a heading whatever paragraph came before the
    /// table; the parser, reading a field it set at the last paragraph marker,
    /// said otherwise. `\s5` is exempt: unfoldingWord's chunk marker is an
    /// empty heading written directly before a verse.
    ///
    /// The span is the `VerseStart`, which runs from `\v` through the number
    /// and any `\va`/`\vp`; the parser reported the `\v` alone.
    fn check_verse_placement(&mut self, verse: &VerseStart<'_>) {
        if !self.chapter_seen {
            self.emit(Code::VerseOutsideChapter, verse.span, "`\\v` before any `\\c`");
        }
        let heading = self.scopes.iter().rev().find_map(|scope| match scope {
            Scope::Para(style) => Some(*style),
            _ => None,
        });
        if let Some(style) = heading {
            let rule = self.style_sheet.get_rule(style.index());
            if matches!(rule.text_type, TextType::Title | TextType::Section) && rule.marker != "s5"
            {
                self.emit(
                    Code::VerseInHeading,
                    verse.span,
                    "`\\v` inside a title or section heading",
                );
            }
        }
        if self.scopes.contains(&Scope::Char) {
            self.emit(
                Code::VerseInCharacterStyle,
                verse.span,
                "`\\v` inside an open character style",
            );
        }
    }

    /// `unexpected-table-column`: a row whose cells skip a column or go
    /// backwards (`\th1 … \th3`, or `\tc2` first in a row).
    ///
    /// The cell keeps the column its marker named, so the row in the tree is
    /// the row as written and the rule is arithmetic over it: each cell starts
    /// where the previous one ended, the first at column 1.
    ///
    /// The span is the cell, which the parser opened at the same offset its
    /// marker did; the message names the column rather than the marker,
    /// because the marker's text is in the source and a check here has only
    /// the tree.
    fn check_table_columns(&mut self, table: &Table<'_>) {
        for row in &table.rows {
            let mut expected = 1u8;
            for cell in &row.cells {
                if cell.column != expected {
                    self.emit(
                        Code::UnexpectedTableColumn,
                        cell.span,
                        format!(
                            "a cell in column {} where column {expected} was expected",
                            cell.column
                        ),
                    );
                }
                expected = cell.column.saturating_add(cell.colspan);
            }
        }
    }

    /// `empty-word`: `\w |lemma="x"\w*`, a word with attributes and no word.
    ///
    /// Only `\w`: an empty `\jmp` is a legitimate link with nothing but its
    /// `link-href`, and `\fig` carries everything in its attributes too. The
    /// node is kept whatever this says, so the predicate is the tree's —
    /// exactly the one the parser used to run on the node it had just built.
    fn check_empty_word(&mut self, char: &Char<'_>) {
        let rule = self.style_sheet.get_rule(char.style.index());
        if rule.marker == "w" && char.attributes.is_some() && char.children.is_empty() {
            self.emit(
                Code::EmptyWord,
                char.span,
                "`\\w` has attributes but no text",
            );
        }
    }

    /// `duplicate-chapter-number` / `chapter-out-of-order`: a `\c` against the
    /// chapters of the same book that the walk has already passed.
    ///
    /// **Per book.** A document is normally one book, but the CLI concatenates
    /// its input files into one, and a second `\id` starts a second book whose
    /// chapters begin again at 1. So [`Order::book`] clears the chapter state
    /// at every `Book` block: two books each with `\c 1` say nothing, while
    /// `\c 1` twice inside one of them is the duplicate this reports.
    ///
    /// Versification is out of scope (ticket 23): nothing here knows which
    /// chapters a book is *supposed* to have, so a gap (`\c 1`, `\c 3`) is not
    /// reported. Only what the document says about itself is.
    ///
    /// The span is the later `ChapterStart`.
    fn check_chapter_order(&mut self, chapter: &ChapterStart<'_>) {
        let number = chapter.number;
        if !self.order.chapters_seen.insert(number) {
            self.emit(
                Code::DuplicateChapterNumber,
                chapter.span,
                format!("chapter {number} occurs more than once"),
            );
        } else if let Some(previous) = self
            .order
            .previous_chapter
            .filter(|previous| number < *previous)
        {
            self.emit(
                Code::ChapterOutOfOrder,
                chapter.span,
                format!("chapter {number} comes after chapter {previous}"),
            );
        }
        self.order.chapter(number);
    }

    /// `duplicate-verse-number` / `verse-out-of-order`: a `\v` against the
    /// verses of the same chapter that the walk has already passed.
    ///
    /// Coverage is by number *with* its segment ([`Coverage`]): `\v 4a` and
    /// `\v 4b` are two verses, while a bare `\v 4` is the whole of verse 4 and
    /// so collides with either. A range covers its endpoints as written and
    /// everything between them unsegmented, so `\v 3-4a` and `\v 4b` sit side
    /// by side — the shape `41MATTes.SFM` uses — while `\v 3-5` and a later
    /// `\v 4` do not.
    ///
    /// A verse reports at most one of the two codes, the duplicate first: a
    /// number that has already been used is a duplicate whether or not it also
    /// runs backwards, and saying both about one `\v` says nothing more.
    ///
    /// A verse before the first `\c` of its book is skipped: there is no
    /// chapter for it to repeat or run backwards in, and
    /// `verse-outside-chapter` has already said what there is to say.
    ///
    /// The span is the `VerseStart` of the *later* verse, the one a reader
    /// would have to move or renumber.
    fn check_verse_order(&mut self, verse: &VerseStart<'_>) {
        let Some(chapter) = self.order.chapter_number else {
            return;
        };
        let number = &verse.number;
        let start = start_key(number);
        // Both of these end their borrow of `self.order` before `emit` takes
        // `&mut self`: one returns an owned key, the other a copy.
        let repeat = self.order.covered.first_covered(number);
        let previous = self.order.previous_verse;
        if let Some(repeat) = repeat {
            self.emit(
                Code::DuplicateVerseNumber,
                verse.span,
                format!(
                    "verse {} occurs more than once in chapter {chapter}",
                    show(repeat)
                ),
            );
        } else if let Some(previous) = previous.filter(|previous| start < *previous) {
            self.emit(
                Code::VerseOutOfOrder,
                verse.span,
                format!(
                    "verse {} comes after verse {} in chapter {chapter}",
                    show(start),
                    show(previous)
                ),
            );
        }
        self.order.covered.add(number);
        self.order.previous_verse = Some(end_key(number));
    }

    /// The parent a character style or note is placed under, or `None` where
    /// nothing is checked.
    ///
    /// The three rules the parser applied to its open-marker stack, read off
    /// the tree instead (ticket 20):
    ///
    /// * inside a table cell nothing is checked. The cell markers occur in no
    ///   `OccursUnder` list, so every style in a table would be reported;
    /// * a note's parent is its paragraph, whatever character styles or notes
    ///   stand between. `\f` inside `\wj` is still `\f` under `\p`;
    /// * a character style inside another character style is not checked
    ///   here at all — `NEST` decides that, and the parser reports
    ///   `character-style-nested-without-plus` for it. Inside a note it is
    ///   the note; otherwise the paragraph.
    fn placement_parent(&self, is_note: bool) -> Option<StyleId> {
        if self.scopes.contains(&Scope::Cell) {
            return None;
        }
        if is_note {
            return self.scopes.iter().rev().find_map(|scope| match scope {
                Scope::Para(style) => Some(*style),
                _ => None,
            });
        }
        match self.scopes.last() {
            Some(Scope::Char) => None,
            Some(Scope::Note(style) | Scope::Para(style)) => Some(*style),
            Some(Scope::Cell) | None => None,
        }
    }

    /// `marker-not-allowed-here` / `marker-not-listed-here`: a character style
    /// or note whose stylesheet entry does not list the marker it sits under.
    ///
    /// The two codes are one rule with two severities. A marker whose whole
    /// `OccursUnder` list is note styles (`\xq`, `\fr`, `\xo` …) exists only
    /// inside a note, so anywhere else is an error and Paratext marks it
    /// `status="invalid"`. Every other list is advisory — Paratext accepts
    /// `\f` under `\cl`, which `\f` does not list — so it only informs.
    ///
    /// The span is the node's, which runs from the opening marker to wherever
    /// the style closed; the parser reported the opening marker alone. Both
    /// start at the same offset (see the note on spans above).
    fn check_placement(&mut self, style: StyleId, span: Span, is_note: bool) {
        let Some(parent) = self.placement_parent(is_note) else {
            return;
        };
        let sheet = self.style_sheet;
        let rule = sheet.get_rule(style.index());
        if rule.occurs_under.is_empty() {
            return;
        }
        let parent_name = &sheet.get_rule(parent.index()).marker;
        if rule.occurs_under.contains(parent_name) {
            return;
        }
        let note_only = rule.occurs_under.iter().all(|allowed| {
            sheet
                .get_rule_by_marker(allowed)
                .is_some_and(|allowed| allowed.is_note())
        });
        let (code, message) = if note_only {
            (
                Code::MarkerNotAllowedHere,
                format!("`\\{}` cannot occur under `\\{parent_name}`", rule.marker),
            )
        } else {
            (
                Code::MarkerNotListedHere,
                format!(
                    "`\\{}` is not listed as occurring under `\\{parent_name}`",
                    rule.marker
                ),
            )
        };
        self.emit(code, span, message);
    }

    /// The attribute list of a `\w`, a milestone or a `\periph` line against
    /// the marker that carries it. Nothing is repaired: the list is in the
    /// tree exactly as written whatever is reported here, which is what puts
    /// all six codes on this side of the split. The ones that decide what the
    /// list *is* — an unquoted value, a missing one, a line break inside the
    /// list — stay with the parser, which had to choose.
    ///
    /// `attributes` is `Some` iff the source had a `|`, so an empty list is
    /// `|` with nothing after it and not a marker written without one.
    fn check_attributes(&mut self, style: StyleId, attributes: &Attributes<'_>) {
        let sheet = self.style_sheet;
        let rule = sheet.get_rule(style.index());
        if attributes.pairs.is_empty() {
            // A milestone is nothing but its attributes, so `\ts-s |\*` says
            // the same as `\ts-s\*` and nothing was lost; unfoldingWord's
            // aligned texts write their translation sections that way. On a
            // character style the `|` announces a value that is missing.
            let code = if rule.is_milestone() {
                Code::EmptyMilestoneAttributeList
            } else {
                Code::EmptyAttributeList
            };
            self.emit(code, attributes.pipe, "`|` is not followed by any attribute");
            return;
        }
        for (index, pair) in attributes.pairs.iter().enumerate() {
            // Both of these keep the pair in the tree; it is the USX and HTML
            // writers that drop it, having no way to write it.
            if !pair.name.is_empty() && !is_valid_attribute_name(&pair.name) {
                self.emit(
                    Code::MalformedAttributeName,
                    pair.span,
                    format!(
                        "`{}` is not an attribute name; it is kept in the tree but \
                         cannot be written to USX",
                        pair.name
                    ),
                );
            }
            if attributes.pairs[..index]
                .iter()
                .any(|earlier| earlier.name == pair.name)
            {
                let message = if pair.name.is_empty() {
                    "the default attribute is given more than once".to_string()
                } else {
                    format!("`{}` is given more than once", pair.name)
                };
                self.emit(Code::DuplicateAttribute, pair.span, message);
            }
        }
        if !attributes.pairs.iter().any(|pair| pair.name.is_empty()) {
            return;
        }
        // `rule` is borrowed from the sheet, not from `self`, so it survives
        // the `emit` calls without a clone.
        let marker = &rule.marker;
        if default_attribute_name(marker).is_none() {
            self.emit(
                Code::NoDefaultAttribute,
                attributes.pipe,
                format!("`\\{marker}` has no default attribute; the value needs a name"),
            );
        }
        if attributes.pairs.len() > 1 {
            self.emit(
                Code::DefaultAttributeWithOthers,
                attributes.pipe,
                "a bare value must be the only attribute; give it a name",
            );
        }
    }
}

impl Analyzer<'_> {
    /// Visit a block list, keeping track of which block starts it.
    ///
    /// `first_in_container` is read by `visit_book`, which `walk_block` calls
    /// with nothing in between, so no deeper walk can have overwritten it by
    /// the time it is asked.
    fn blocks(&mut self, blocks: &[Block<'_>]) {
        for (index, block) in blocks.iter().enumerate() {
            self.first_in_container = index == 0;
            self.visit_block(block);
        }
    }
}

/// One verse number with its segment: `4a` is `(4, Some('a'))`, a bare `4` is
/// `(4, None)`.
///
/// The derived order is the one the checks want. `None` sorts before `Some`,
/// so a bare `4` comes before `4a` — which is how they are written — and
/// segments compare by letter, so `4b` after `4a` is forward and `4a` after
/// `4b` is backwards.
type VerseKey = (usize, Option<char>);

/// A key as USFM writes it, for a message: `4`, or `4a`.
fn show((number, segment): VerseKey) -> String {
    match segment {
        Some(segment) => format!("{number}{segment}"),
        None => number.to_string(),
    }
}

/// Where a verse begins: the start of its first range, `3` in `\v 3-5` and
/// `1` in `\v 1,3-5`.
fn start_key(number: &NumberList) -> VerseKey {
    let range = number.first_range();
    (range.start, range.start_modifier)
}

/// Where a verse ends: the end of its last range. A collapsed range carries
/// its only segment in `start_modifier` (`\v 4a` is 4..4 with `start_modifier`
/// `a`), so that is the segment the end takes.
fn end_key(number: &NumberList) -> VerseKey {
    let range = number.last_range();
    if range.is_collapsed() {
        (range.end, range.end_modifier.or(range.start_modifier))
    } else {
        (range.end, range.end_modifier)
    }
}

/// A piece of what a verse number covers.
///
/// A [`NumberRange`] is at most three of these: its start as written, its end
/// as written, and — because `\v 3-7` is a verse that contains verses 4, 5 and
/// 6 whole — everything strictly between them, unsegmented. Keeping the
/// interior as a run rather than as its numbers is what stops `\v 1-99999999`
/// from costing anything.
#[derive(Clone, Copy)]
enum Part {
    /// One number, with or without a segment.
    One(VerseKey),
    /// Every number from the first to the second inclusive, unsegmented.
    Run(usize, usize),
}

/// The parts of one range, in ascending order.
fn parts(range: &NumberRange) -> impl Iterator<Item = Part> {
    let start = Part::One((range.start, range.start_modifier));
    let interior = (range.end > range.start.saturating_add(1))
        .then(|| Part::Run(range.start + 1, range.end - 1));
    // A collapsed range has one number, and it is already `start` — unless the
    // two ends were written with different segments (`\v 4a-4b`).
    let end = (range.end != range.start
        || (range.end_modifier.is_some() && range.end_modifier != range.start_modifier))
        .then_some(Part::One((range.end, range.end_modifier)));
    [Some(start), interior, end].into_iter().flatten()
}

/// The verse numbers used so far in one chapter.
///
/// Two stores, because the two questions are different: whole numbers, which
/// cover every segment of themselves, are kept as sorted, disjoint,
/// non-adjacent inclusive intervals; segments are kept one by one. A number is
/// covered if an interval holds it, or — for a bare number — if any segment of
/// it was written, which is the rule that makes `\v 4a` then `\v 4` a
/// duplicate.
#[derive(Default)]
struct Coverage {
    whole: Vec<(usize, usize)>,
    segments: BTreeSet<(usize, char)>,
}

impl Coverage {
    /// The first number of `number` an earlier verse already covered, or
    /// `None` if the verse is new all through.
    fn first_covered(&self, number: &NumberList) -> Option<VerseKey> {
        number
            .ranges()
            .iter()
            .flat_map(parts)
            .find_map(|part| self.covered(part))
    }

    fn covered(&self, part: Part) -> Option<VerseKey> {
        match part {
            Part::One(key) => self.covers(key).then_some(key),
            Part::Run(low, high) => {
                let from_whole = self.overlapping(low, high).map(|(start, _)| start.max(low));
                let from_segment = self
                    .segments
                    .range((low, '\0')..=(high, char::MAX))
                    .next()
                    .map(|&(number, _)| number);
                match (from_whole, from_segment) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    (found, None) | (None, found) => found,
                }
                .map(|number| (number, None))
            }
        }
    }

    fn covers(&self, (number, segment): VerseKey) -> bool {
        if self.overlapping(number, number).is_some() {
            return true;
        }
        match segment {
            Some(segment) => self.segments.contains(&(number, segment)),
            // A bare number is the whole verse, so any of its segments is a
            // collision.
            None => self
                .segments
                .range((number, '\0')..=(number, char::MAX))
                .next()
                .is_some(),
        }
    }

    /// The first interval of `whole` that meets `low..=high`.
    fn overlapping(&self, low: usize, high: usize) -> Option<(usize, usize)> {
        let position = self.whole.partition_point(|&(_, end)| end < low);
        self.whole
            .get(position)
            .copied()
            .filter(|&(start, _)| start <= high)
    }

    /// Record everything `number` covers.
    fn add(&mut self, number: &NumberList) {
        for part in number.ranges().iter().flat_map(parts) {
            match part {
                Part::One((number, Some(segment))) => {
                    self.segments.insert((number, segment));
                }
                Part::One((number, None)) => self.add_run(number, number),
                Part::Run(low, high) => self.add_run(low, high),
            }
        }
    }

    /// Add `low..=high` to `whole`, merging with any interval it touches so
    /// the list stays sorted, disjoint and searchable by bisection however
    /// many verses a chapter has.
    fn add_run(&mut self, low: usize, high: usize) {
        if low > high {
            return;
        }
        // The intervals this one joins onto: those ending at or after `low - 1`
        // and starting at or before `high + 1`. Adjacency counts, so `1-2` and
        // `3-4` become `1-4` rather than two intervals that both hold 2-and-3.
        let first = self.whole.partition_point(|&(_, end)| end.saturating_add(1) < low);
        let last = self
            .whole
            .partition_point(|&(start, _)| start <= high.saturating_add(1));
        if first == last {
            self.whole.insert(first, (low, high));
            return;
        }
        let merged = (
            low.min(self.whole[first].0),
            high.max(self.whole[last - 1].1),
        );
        self.whole.splice(first..last, [merged]);
    }
}

impl Visit for Analyzer<'_> {
    fn visit_document(&mut self, document: &Document<'_>) {
        self.check_document(document);
        self.blocks(&document.blocks);
    }

    fn visit_book(&mut self, book: &Book<'_>) {
        self.check_unlisted_book_code(book);
        self.check_id_not_first(book);
        self.book = Some(book.code);
        // A second `\id` is a second book — `id-not-first` says so, and the
        // CLI produces one by concatenating files — and its chapters number
        // from 1 again.
        self.order.book();
    }

    fn visit_chapter_start(&mut self, chapter: &ChapterStart<'_>) {
        self.chapter_seen = true;
        self.check_chapter_order(chapter);
    }

    fn visit_para(&mut self, para: &Para<'_>) {
        self.check_verse_text_before_chapter(para);
        self.in_scope(Scope::Para(para.style), |analyzer| walk_para(analyzer, para));
    }

    fn visit_verse_start(&mut self, verse: &VerseStart<'_>) {
        self.check_verse_placement(verse);
        self.check_verse_order(verse);
    }

    fn visit_table(&mut self, table: &Table<'_>) {
        self.check_table_columns(table);
        for row in &table.rows {
            for cell in &row.cells {
                self.visit_table_cell(cell);
            }
        }
    }

    fn visit_table_cell(&mut self, cell: &TableCell<'_>) {
        self.in_scope(Scope::Cell, |analyzer| walk_table_cell(analyzer, cell));
    }

    fn visit_sidebar(&mut self, sidebar: &Sidebar<'_>) {
        self.blocks(&sidebar.blocks);
    }

    fn visit_char(&mut self, char: &Char<'_>) {
        self.check_placement(char.style, char.span, false);
        self.check_empty_word(char);
        if let Some(attributes) = &char.attributes {
            self.check_attributes(char.style, attributes);
        }
        // Not `walk_char`: it visits the attribute list after the children,
        // and this pass has just read that list itself.
        self.in_scope(Scope::Char, |analyzer| {
            for inline in &char.children {
                analyzer.visit_inline(inline);
            }
        });
    }

    fn visit_note(&mut self, note: &Note<'_>) {
        self.check_placement(note.style, note.span, true);
        self.in_scope(Scope::Note(note.style), |analyzer| walk_note(analyzer, note));
    }

    fn visit_milestone(&mut self, milestone: &Milestone<'_>) {
        if let Some(attributes) = &milestone.attributes {
            self.check_attributes(milestone.style, attributes);
        }
    }

    fn visit_periph(&mut self, periph: &Periph<'_>) {
        if let Some(attributes) = &periph.attributes {
            self.check_attributes(periph.style, attributes);
        }
        self.blocks(&periph.blocks);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;
    use std::str::FromStr;
    use usfm_ast::{Block, BookCode, ChapterEnd};
    use usfm_span::Span;

    /// A document holding `code` and one empty chapter, built by hand: this
    /// crate does not depend on the parser, and the integration tests in
    /// `tests/checks.rs` cover the real path through the facade.
    ///
    /// The chapter is there so the document is not an `empty-book` (a book and
    /// nothing else), which would report a second diagnostic and is its own
    /// test in `tests/checks.rs`.
    fn document_with_book(code: &str, span: Span) -> Document<'static> {
        Document::without_styles(vec![
            Block::Book(Book {
                code: BookCode::from_str(code).expect("a well-formed book code"),
                description: Cow::Borrowed(""),
                span,
            }),
            Block::ChapterEnd(ChapterEnd {
                number: 1,
                span: Span::new(span.end, span.end),
            }),
        ])
    }

    /// The table in `usfm_diagnostics`'s module documentation says which side
    /// each code is on; [`Code::is_semantic`] is its executable form and
    /// [`EMITS`] is this crate's half. They have to be the same set, both
    /// ways round.
    #[test]
    fn semantic_emits_exactly_the_semantic_codes() {
        let not_marked: Vec<&str> = EMITS
            .iter()
            .filter(|code| !code.is_semantic())
            .map(|code| code.as_str())
            .collect();
        assert!(
            not_marked.is_empty(),
            "this crate emits codes `Code::is_semantic` does not name: {not_marked:?}"
        );
        let not_emitted: Vec<&str> = Code::ALL
            .iter()
            .filter(|code| code.is_semantic() && !EMITS.contains(code))
            .map(|code| code.as_str())
            .collect();
        assert!(
            not_emitted.is_empty(),
            "`Code::is_semantic` names codes this crate does not emit: {not_emitted:?}"
        );
    }

    #[test]
    fn a_listed_book_code_reports_nothing() {
        let document = document_with_book("GEN", Span::new(0, 7));
        assert!(analyze(&document).is_empty());
    }

    #[test]
    fn an_unlisted_book_code_is_reported_at_the_id_line() {
        let document = document_with_book("ZZZ", Span::new(0, 7));
        let diagnostics = analyze(&document);
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, Code::UnlistedBookCode);
        assert_eq!(diagnostics[0].span, Span::new(0, 7));
        assert!(
            diagnostics[0].message.contains("ZZZ"),
            "{}",
            diagnostics[0].message
        );
    }

    /// A book that is not the first block of its list is `id-not-first`,
    /// whatever else is wrong with it: the second `\id` here reports that as
    /// well as its unlisted code.
    #[test]
    fn diagnostics_come_back_sorted_by_span_start() {
        let document = Document::without_styles(vec![
            Block::Book(Book {
                code: BookCode::from_str("ZZZ").unwrap(),
                description: Cow::Borrowed(""),
                span: Span::new(0, 7),
            }),
            Block::Book(Book {
                code: BookCode::from_str("YYY").unwrap(),
                description: Cow::Borrowed(""),
                span: Span::new(40, 47),
            }),
        ]);
        let reported: Vec<(u32, Code)> = analyze(&document)
            .iter()
            .map(|diagnostic| (diagnostic.span.start, diagnostic.code))
            .collect();
        assert_eq!(
            reported,
            vec![
                (0, Code::UnlistedBookCode),
                (40, Code::UnlistedBookCode),
                (40, Code::IdNotFirst),
            ]
        );
    }
}
