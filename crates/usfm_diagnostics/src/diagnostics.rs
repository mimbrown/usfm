//! Parse diagnostics and the parse result type.
//!
//! The parser always completes and always produces a document. Anything it
//! had to guess about, repair, or drop is reported as a [`Diagnostic`] in the
//! [`ParseResult`]. Strictness is a policy applied to the result
//! ([`ParseResult::strict`]), not a parser mode.
//!
//! Each [`Code`] documents its trigger, the recovery the parser performs, and
//! the severity. That documentation is the recovery table: if the parser
//! repairs something, there is a code for it here and a test for it in
//! `usfm_parser/tests/recovery.rs`.
//!
//! Not every code is the parser's. A check that reads the finished tree and
//! repairs nothing lives in `usfm_semantic` (M4), and its codes are the ones
//! [`Code::is_semantic`] names; their tests are in
//! `usfm_semantic/tests/checks.rs`. Every code has a test in one file or the
//! other, and each file's coverage test uses `is_semantic` to know which are
//! its own.
//!
//! # Which side a code is on
//!
//! **A parser code repairs; a semantic code reports.** Ticket 21 audited every
//! variant against two questions, and the answer to either one puts a code in
//! the parser:
//!
//! 1. would deleting the check change the tree? Then the check *is* how the
//!    tree got its shape — a dropped marker, an invented caller, an implicit
//!    paragraph — and it belongs where the shape is decided;
//! 2. is what the *author wrote* still visible in the finished tree? A token
//!    that was dropped, a leading zero that parsed to a number, a `+` that is
//!    not recorded, a `\ca*` that was never written: a pass over the tree
//!    cannot see any of them. Nor can it read a repair as if it were the
//!    input — a stray `\esbe` stands in the tree as an empty paragraph, which
//!    is what the parser made of it, not what was written.
//!
//! Everything else moves: the input parsed exactly as written, deleting the
//! check would only make the diagnostics shorter, and the tree holds what the
//! rule reads.
//!
//! | Code | Sev | What the parser does with it | Side, and why |
//! |---|---|---|---|
//! | `unknown-marker` | E | drops the marker | parser: the marker is gone |
//! | `unknown-custom-marker` | W | drops the marker | parser: the marker is gone |
//! | `unknown-milestone` | W | registers the style, keeps the node | parser: the tree does not say which styles the sheet lacked, and this is reported once per name, not per use |
//! | `unknown-custom-milestone` | I | registers the style, keeps the node | parser: as above |
//! | `unmatched-closing-marker` | E | drops the marker | parser: the marker is gone |
//! | `paragraph-marker-closed` | E | drops the marker | parser: the marker is gone |
//! | `unmatched-milestone-end` | E | drops the `\*` | parser: the token is gone |
//! | `milestone-not-closed` | E | closes the milestone early | parser: it decides where the node ends |
//! | `stray-backslash` | E | keeps the `\` as text | parser: in the tree it is a backslash like any other, escaped or not |
//! | `marker-not-allowed-here` | E | nothing | **semantic** (20): the style, its parent and the sheet are all in the tree |
//! | `marker-not-listed-here` | I | nothing | **semantic** (20): as above |
//! | `nested-marker-not-nested` | W | reads `\+x` as `\x` | parser: whether the `+` was written is not in the tree |
//! | `character-style-not-closed` | W | closes the style there | parser: it decides where the node ends |
//! | `character-style-implicitly-closed` | I | closes one style, opens a sibling | parser: it is the sibling-or-child decision |
//! | `character-style-nested-without-plus` | I | nests the style | parser: it is the nesting decision, and the `+` is not in the tree |
//! | `figure-not-closed` | E | closes the figure there | parser: it decides where the node ends |
//! | `empty-word` | E | nothing | **semantic** (21): a `\w` with attributes and no children is what the rule asks for |
//! | `note-not-closed` | E | closes the note there | parser: it decides where the node ends |
//! | `missing-note-caller` | E | assumes `+` | parser: it invents the caller |
//! | `missing-verse-number` | E | drops `\v` | parser: the marker is gone |
//! | `malformed-verse-number` | E | drops `\v` and the number | parser: the marker is gone |
//! | `verse-in-note` | E | drops `\v` and the number | parser: the verse is not in the tree to report on |
//! | `verse-in-character-style` | W | nothing | **semantic** (21): a `VerseStart` inside a `Char` |
//! | `verse-in-heading` | E | nothing | **semantic** (21): a `VerseStart` in a `Title`/`Section` paragraph |
//! | `verse-outside-chapter` | E | nothing | **semantic** (21): a `VerseStart` before any `ChapterStart` |
//! | `verse-text-before-chapter` | E | nothing | **semantic** (21): a verse-text `Para` before any `ChapterStart` |
//! | `duplicate-verse-number` | W | nothing | **semantic** (23): the chapter's verse numbers, in the order the walk passes them |
//! | `verse-out-of-order` | W | nothing | **semantic** (23): as above |
//! | `duplicate-chapter-number` | W | nothing | **semantic** (23): the book's chapter numbers, likewise |
//! | `chapter-out-of-order` | W | nothing | **semantic** (23): as above |
//! | `missing-chapter-number` | E | drops `\c` | parser: the marker is gone |
//! | `malformed-chapter-number` | E | drops `\c` and the number | parser: the marker is gone |
//! | `number-has-leading-zero` | E | reads the number without the zero | parser: `01` is the number 1 in the tree, and nothing remembers the zero |
//! | `alternate-chapter-not-closed` | E | keeps the number, carries on | parser: a closed and an unclosed `\ca` give the same `alt_number` |
//! | `alternate-verse-not-closed` | E | keeps the number, carries on | parser: as above |
//! | `missing-book-code` | E | drops the `\id` line | parser: there is no `Book` in the tree |
//! | `unknown-book-code` | E | drops the `\id` line | parser: there is no `Book` in the tree |
//! | `unlisted-book-code` | W | nothing | **semantic** (19): the `Book` is kept as written |
//! | `missing-id` | E | nothing | **semantic** (21): the document's first block is not a `Book` |
//! | `id-not-first` | E | nothing | **semantic** (21): a `Book` that is not the first block of its list |
//! | `empty-book` | E | nothing | **semantic** (21): the document's only block is a `Book` |
//! | `sidebar-not-closed` | E | closes the sidebar at `\c`, the next `\esb` or EOF | parser: it decides where the node ends |
//! | `unmatched-sidebar-end` | E | keeps the `\esbe` as an empty paragraph | parser: that paragraph is the repair, not anything the author wrote |
//! | `content-outside-paragraph` | E | opens an implicit `\p` | parser: it invents the node |
//! | `content-dropped` | E | drops the content | parser: the content is gone |
//! | `expected-table-cell` | E | opens an implicit `\tc1` | parser: it invents the node |
//! | `unexpected-table-column` | E | keeps the column the marker named | **semantic** (21): `TableCell::column` in row order |
//! | `unexpected-pipe` | E | keeps the `|` as text | parser: an escaped `\|` reaches the tree as the same character |
//! | `unterminated-attribute-value` | E | ends the value at the marker | parser: it decides what the value is |
//! | `newline-in-attributes` | E | reads the break as a space | parser: the break is not in the tree |
//! | `empty-attribute-list` | E | keeps an empty list | **semantic** (20): `Attributes` with no pairs, and its `|` |
//! | `empty-milestone-attribute-list` | W | keeps an empty list | **semantic** (20): as above |
//! | `no-default-attribute` | E | keeps the bare value | **semantic** (20): a pair with an empty name |
//! | `default-attribute-with-others` | E | keeps the bare value | **semantic** (20): as above |
//! | `attribute-value-not-quoted` | E | takes the one word after `=` | parser: it decides what the value is; the quotes are not in the tree |
//! | `missing-attribute-value` | E | keeps an empty value | parser: `name=` and `name=""` are the same pair in the tree |
//! | `malformed-attribute-name` | E | keeps the name as written | **semantic** (20): the pair is in the tree, name and span |
//! | `duplicate-attribute` | E | keeps every occurrence | **semantic** (20): as above |
//! | `internal` | E | stops parsing | parser: it is the parser's own invariant |
//!
//! The executable form of the last column is [`Code::is_semantic`], which both
//! crates' coverage tests read; the table is the reasoning behind it.

use std::fmt;
use std::str::FromStr;

use usfm_ast::Document;
use usfm_span::{LineIndex, Span};

/// How serious a diagnostic is. Ordered so that `Error > Warning > Info`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Worth surfacing in an editor; never a reason to fail a build.
    Info,
    /// Valid USFM, but almost certainly not what was meant, or a known
    /// Paratext-ism that stricter USFM versions reject.
    Warning,
    /// The input violates USFM and the parser guessed. The tree around this
    /// span is a repair, not a faithful reading of the input.
    Error,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Error => "error",
        })
    }
}

/// Stable identifiers for every situation the parser recovers from.
///
/// Each variant's documentation is one row of the recovery table:
/// **Trigger** (what was seen), **Recovery** (what the tree contains
/// instead), **Severity**.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Code {
    /// **Trigger:** a marker name not in the stylesheet.
    /// **Recovery:** the marker is dropped. If it has the shape of a
    /// milestone (`\name |attrs\*`), its attributes and `\*` are dropped with
    /// it. Any following text is kept.
    /// **Severity:** Error.
    UnknownMarker,
    /// **Trigger:** a user-defined marker (`\z...`) not in the stylesheet.
    /// These are valid USFM, but without a stylesheet entry the parser
    /// cannot tell what kind of marker it is.
    /// **Recovery:** as for `UnknownMarker`.
    /// **Severity:** Warning.
    UnknownCustomMarker,
    /// **Trigger:** a marker not in the stylesheet whose syntax shows it is a
    /// milestone (`\name\*` or `\name |attrs\*`), and which is not a `\z`
    /// custom marker or the `-s`/`-e` form of a marker the stylesheet does
    /// define. `\ts` (tStudio translator sections) is the common case.
    /// **Recovery:** the style is registered on the document's stylesheet as a
    /// milestone and the node is kept with all its attributes. Nothing is
    /// dropped, so this never rises to an error.
    /// **Severity:** Warning.
    UnknownMilestone,
    /// **Trigger:** as `UnknownMilestone`, for a `\z` custom marker
    /// (`\zaln-s` from unfoldingWord alignment files, `\zms`).
    /// **Recovery:** as `UnknownMilestone`.
    /// **Severity:** Info. The `\z` namespace is how USFM sanctions custom
    /// markers, so there is nothing to warn about.
    UnknownCustomMilestone,
    /// **Trigger:** a closing marker (`\name*`) with no matching open marker
    /// in the current paragraph, cell, or note.
    /// **Recovery:** the closing marker is dropped; open styles are unchanged.
    /// **Severity:** Error.
    UnmatchedClosingMarker,
    /// **Trigger:** a paragraph-style marker used in closing form (`\p*`).
    /// **Recovery:** the marker is dropped.
    /// **Severity:** Error.
    ParagraphMarkerClosed,
    /// **Trigger:** `\*` with no open milestone.
    /// **Recovery:** dropped.
    /// **Severity:** Error.
    UnmatchedMilestoneEnd,
    /// **Trigger:** a milestone (`\qt-s |...`) not terminated by `\*` before
    /// the next marker or end of input; or a start milestone (`\zaln-s`,
    /// `\k-s`, known to the stylesheet or not) whose attribute list runs to
    /// the end of its line with no `\*`, usfm-js's pre-USFM-3 "old format"
    /// (ticket 43).
    /// **Recovery:** the milestone is closed where the `\*` should have been:
    /// at the end of its line, in the second case, with the line break left
    /// as the text it would be after `\*`.
    /// **Severity:** Error.
    MilestoneNotClosed,
    /// **Trigger:** a `\` that does not begin a marker or an escape
    /// (`\\`, `\|`, `\"`): followed by whitespace, other punctuation, or
    /// end of input.
    /// **Recovery:** kept as literal text.
    /// **Severity:** Error.
    StrayBackslash,
    /// **Trigger:** a note-content marker (one whose `OccursUnder` lists
    /// only note styles: `\fr`, `\xq`, `\xo` …) opened outside such a note,
    /// e.g. `\xq` in a paragraph rather than in `\x`.
    /// **Recovery:** the style is parsed where it is.
    /// **Severity:** Error. Paratext marks these `status="invalid"`.
    /// **Reported by:** `usfm_semantic` (ticket 20). The span is the style's
    /// node, from its opening marker to wherever it closed.
    MarkerNotAllowedHere,
    /// **Trigger:** any other character style or note opened under a marker
    /// its `OccursUnder` does not list, such as `\f` under `\cl`. The
    /// stylesheet lists are conservative and Paratext accepts these, so this
    /// only informs. Nesting inside a character style is governed by `NEST`
    /// instead, and a note's parent is its paragraph whatever character
    /// styles are open around it.
    /// **Recovery:** the style is parsed where it is.
    /// **Severity:** Info.
    /// **Reported by:** `usfm_semantic` (ticket 20). The span is the style's
    /// node, from its opening marker to wherever it closed.
    MarkerNotListedHere,
    /// **Trigger:** a nested marker (`\+name`) used where no character style
    /// is open.
    /// **Recovery:** treated as the un-nested marker.
    /// **Severity:** Warning.
    NestedMarkerNotNested,
    /// **Trigger:** a character style still open when a paragraph marker,
    /// table cell marker, an enclosing character style's closing marker, or
    /// end of input is reached.
    /// **Recovery:** the style is closed at that point.
    /// **Severity:** Warning. Valid in USFM 3.0 (Paratext implicit closure);
    /// rejected by USFM 3.1. Not reported for note content (`\ft` closed by
    /// `\f*`), which the spec documents as acceptable.
    CharacterStyleNotClosed,
    /// **Trigger:** outside a note, a character style marker opened while
    /// another character style is open, without the `+` nesting prefix, that
    /// either may not nest (no `NEST` in its `OccursUnder`: `\fr`, `\ft`,
    /// `\xo`, `\iqt` …) or has no closing marker of its own ahead. One that
    /// satisfies both nests instead; see `CharacterStyleNestedWithoutPlus`.
    /// **Recovery:** the open style is closed and the new one becomes its
    /// sibling (implicit closure).
    /// **Severity:** Info. Inside notes this is the normal idiom and is not
    /// reported.
    CharacterStyleImplicitlyClosed,
    /// **Trigger:** a character style marker opened while another character
    /// style is open, without the `+` nesting prefix, whose stylesheet entry
    /// allows nesting (`NEST` in `OccursUnder`: `\nd`, `\sc`, `\xt`, `\w` …)
    /// *and* whose own closing marker lies ahead in the container, e.g.
    /// `\bk The \nd Lord\nd* Battles\bk*` or `\ft see \xt Gen 1\xt* …\f*`.
    /// (`\xo 1.1 \xt Gen 1\x*` is a sibling: nothing closes the `\xt`.)
    /// **Recovery:** treated as nested, exactly as `\+name` would be. This is
    /// what Paratext writes for tcdocs; USFM 3 asks for the `+`.
    /// **Severity:** Info.
    CharacterStyleNestedWithoutPlus,
    /// **Trigger:** `\fig` still open when a paragraph marker or end of
    /// input is reached.
    /// **Recovery:** the figure is closed at that point.
    /// **Severity:** Error.
    FigureNotClosed,
    /// **Trigger:** `\w` with attributes but no text (`\w |lemma="x"\w*`).
    /// Other attributed styles such as `\jmp` may legitimately be empty.
    /// **Recovery:** none; the empty node is kept.
    /// **Severity:** Error.
    /// **Reported by:** `usfm_semantic` (ticket 21), from the `Char` itself:
    /// style `w`, an attribute list, no children. The span is the node, which
    /// starts at the marker the parser reported.
    EmptyWord,
    /// **Trigger:** a note (`\f`, `\x`, …) still open when a paragraph marker
    /// or end of input is reached.
    /// **Recovery:** the note is closed at that point.
    /// **Severity:** Error.
    NoteNotClosed,
    /// **Trigger:** a note marker not followed by a caller (`+`, `-`, or a
    /// custom caller).
    /// **Recovery:** the caller `+` is assumed.
    /// **Severity:** Error.
    MissingNoteCaller,
    /// **Trigger:** `\v` not followed by a verse number.
    /// **Recovery:** the verse marker is dropped.
    /// **Severity:** Error.
    MissingVerseNumber,
    /// **Trigger:** `\v` followed by something that is not a verse number,
    /// range, or list (`1`, `1a`, `1-3`, `1,3`).
    /// **Recovery:** the verse marker and the malformed number are dropped.
    /// **Severity:** Error.
    MalformedVerseNumber,
    /// **Trigger:** `\v` inside a footnote or cross reference.
    /// **Recovery:** the verse marker and its number are dropped.
    /// **Severity:** Error.
    VerseInNote,
    /// **Trigger:** `\v` inside an open character style.
    /// **Recovery:** none; the verse is kept where it is.
    /// **Severity:** Warning. Paratext reports this as an error; texts in the
    /// wild commonly let `\wj` span verses.
    /// **Reported by:** `usfm_semantic` (ticket 21), at the `VerseStart`,
    /// which is inside a `Char` in the tree.
    VerseInCharacterStyle,
    /// **Trigger:** `\v` in a title or section heading paragraph. The
    /// unfoldingWord chunk marker `\s5` is exempt: by convention it is an
    /// empty heading placed directly before a verse.
    /// **Recovery:** none; the verse is kept where it is.
    /// **Severity:** Error.
    /// **Reported by:** `usfm_semantic` (ticket 21), at the `VerseStart`,
    /// against the paragraph it is in — where the parser read the style of the
    /// last paragraph marker it had passed, so a verse in a table cell after a
    /// heading is no longer reported.
    VerseInHeading,
    /// **Trigger:** `\v` before any `\c`.
    /// **Recovery:** none; the verse is kept where it is.
    /// **Severity:** Error.
    /// **Reported by:** `usfm_semantic` (ticket 21), at the `VerseStart` the
    /// walk reaches before any `ChapterStart`.
    VerseOutsideChapter,
    /// **Trigger:** in a scripture book, a verse-text paragraph (`\p`,
    /// `\q1`, …) before the first `\c`.
    /// **Recovery:** none.
    /// **Severity:** Error.
    /// **Reported by:** `usfm_semantic` (ticket 21). The span is the
    /// paragraph node, which starts at the marker the parser reported.
    VerseTextBeforeChapter,
    /// **Trigger:** a verse number already covered by an earlier verse of the
    /// same chapter (`\v 6` twice; `\v 3-5` then `\v 4`; `\v 4` then `\v 4a`).
    /// Coverage is by number *with* its segment, so `\v 4a` and `\v 4b` are
    /// distinct while an unsegmented `\v 4` covers both.
    /// **Recovery:** none. Both verses are in the tree as written.
    /// **Severity:** Warning. The document is well-formed USFM; whether its
    /// verses are unique is a judgement about the text.
    /// **Reported by:** `usfm_semantic` (ticket 23), on its walk, at the later
    /// verse's `VerseStart`.
    DuplicateVerseNumber,
    /// **Trigger:** a verse whose number starts lower than the previous verse
    /// of the same chapter ended (`\v 7a` then `\v 5`). A verse that repeats a
    /// number is [`Code::DuplicateVerseNumber`] instead, never both.
    /// **Recovery:** none. The verses are in the tree in the order written.
    /// **Severity:** Warning.
    /// **Reported by:** `usfm_semantic` (ticket 23), on its walk, at the later
    /// verse's `VerseStart`.
    VerseOutOfOrder,
    /// **Trigger:** a chapter number that an earlier `\c` of the same book
    /// already used. Per book, not per document: a second `\id` starts a book
    /// whose chapters number from 1 again, and the CLI makes such a document
    /// whenever it is given several files.
    /// **Recovery:** none. Both chapters are in the tree as written.
    /// **Severity:** Warning.
    /// **Reported by:** `usfm_semantic` (ticket 23), on its walk, at the later
    /// `ChapterStart`.
    DuplicateChapterNumber,
    /// **Trigger:** a chapter numbered lower than the `\c` before it in the
    /// same book (`\c 2` then `\c 1`). A chapter that repeats a number is
    /// [`Code::DuplicateChapterNumber`] instead, never both.
    /// **Recovery:** none.
    /// **Severity:** Warning.
    /// **Reported by:** `usfm_semantic` (ticket 23), on its walk, at the later
    /// `ChapterStart`.
    ChapterOutOfOrder,
    /// **Trigger:** `\c` not followed by a chapter number.
    /// **Recovery:** the chapter marker is dropped.
    /// **Severity:** Error.
    MissingChapterNumber,
    /// **Trigger:** `\c` followed by something that is not an integer, or
    /// `\ca` followed by something that is not a chapter number.
    /// **Recovery:** for `\c` the chapter marker and the malformed number are
    /// dropped; for `\ca` the alternate number is dropped.
    /// **Severity:** Error.
    MalformedChapterNumber,
    /// **Trigger:** a chapter or verse number written with a leading zero
    /// (`\c 091`, `\v 01`), which USFM 3.1 rejects.
    /// **Recovery:** the number is read without the zero.
    /// **Severity:** Error. The parser's, although it repairs nothing worth
    /// the name: `\v 01` parses to the number 1, so the zero is gone from the
    /// tree and a pass over the tree could not tell it had ever been there.
    NumberHasLeadingZero,
    /// **Trigger:** `\ca` not terminated by `\ca*`.
    /// **Recovery:** the alternate number is kept; parsing continues.
    /// **Severity:** Error.
    AlternateChapterNotClosed,
    /// **Trigger:** `\va` or `\vp` after a verse number not terminated by
    /// its closing marker.
    /// **Recovery:** the alternate or published number is kept; parsing
    /// continues.
    /// **Severity:** Error.
    AlternateVerseNotClosed,
    /// **Trigger:** `\id` not followed by a book code.
    /// **Recovery:** the `\id` line is dropped.
    /// **Severity:** Error.
    MissingBookCode,
    /// **Trigger:** `\id` followed by a code that is not a book code at all:
    /// one that matches neither a known book nor `book@code`'s catch-all
    /// pattern `[A-Z][A-Z0-9]{2}|[0-9][A-Z][0-9]|[0-9]{2}[A-Z]` in `usx.rnc`
    /// (`\id zzz`, `\id GENESIS`).
    /// **Recovery:** the `\id` line is dropped.
    /// **Severity:** Error.
    UnknownBookCode,
    /// **Trigger:** `\id` followed by a well-formed code that is not one of
    /// the books `BookCode` lists (`\id TST`, `\id ZZZ`). USX allows a project
    /// its own code, so the document is valid; it is reported because a typo
    /// in a real code looks exactly like this.
    /// **Recovery:** none. The book is kept as `BookCode::Other` and written
    /// out verbatim (`<book code="TST">`).
    /// **Severity:** Warning.
    /// **Reported by:** `usfm_semantic` (there is no repair to report, so the
    /// parser is silent about it; ticket 19). The span is the whole `\id`
    /// line, which is the span the AST's `Book` carries.
    UnlistedBookCode,
    /// **Trigger:** the document does not start with `\id`.
    /// **Recovery:** none.
    /// **Severity:** Error.
    /// **Reported by:** `usfm_semantic` (ticket 21), at offset 0 — where the
    /// `\id` should have been.
    MissingId,
    /// **Trigger:** `\id` after other content.
    /// **Recovery:** the book is kept.
    /// **Severity:** Error.
    /// **Reported by:** `usfm_semantic` (ticket 21): a `Book` that is not the
    /// first block of the list it is in. The span is the whole `\id` line.
    IdNotFirst,
    /// **Trigger:** a document containing only an `\id` line.
    /// **Recovery:** none.
    /// **Severity:** Error.
    /// **Reported by:** `usfm_semantic` (ticket 21), at the end of the source,
    /// which is what `Document::span` is for.
    EmptyBook,
    /// **Trigger:** `\esb` while a sidebar is already open, or still open
    /// at end of input.
    /// **Recovery:** the sidebar is closed where its content ran out — at the
    /// `\c`, the second `\esb`, or end of input. A closed and an unclosed
    /// sidebar are the same node afterwards, which is why this stays with the
    /// parser.
    /// **Severity:** Error.
    SidebarNotClosed,
    /// **Trigger:** `\esbe` with no open sidebar.
    /// **Recovery:** the marker is kept as an empty paragraph of its own
    /// style, which is a repair rather than anything the author wrote, so the
    /// parser is the one to report it.
    /// **Severity:** Error.
    UnmatchedSidebarEnd,
    /// **Trigger:** text, a verse, a character style, a note, or a milestone
    /// appearing where a paragraph marker is required (before the first
    /// paragraph, or directly after `\c` or `\id`).
    /// **Recovery:** an implicit `\p` paragraph is opened to hold the content.
    /// **Severity:** Error.
    ContentOutsideParagraph,
    /// **Trigger:** content outside a paragraph when the stylesheet has no
    /// `p` marker to use for the implicit paragraph.
    /// **Recovery:** the content is dropped up to the next paragraph marker.
    /// **Severity:** Error.
    ContentDropped,
    /// **Trigger:** a cell marker naming a column other than the next one
    /// (`\th1 … \th3`, or `\tc2` first in a row).
    /// **Recovery:** the cell keeps the column it names.
    /// **Severity:** Error.
    /// **Reported by:** `usfm_semantic` (ticket 21), from `TableCell::column`
    /// in row order. The span is the cell, which starts at its marker; the
    /// message names the column rather than the marker, whose text is in the
    /// source and not in the tree.
    UnexpectedTableColumn,
    /// **Trigger:** `\tr` not followed by a table cell marker (`\tc1`,
    /// `\th1`, `\tcr2`, …).
    /// **Recovery:** an implicit `\tc1` cell is opened to hold the content.
    /// **Severity:** Error.
    ExpectedTableCell,
    /// **Trigger:** `|` outside a character style that can carry attributes.
    /// **Recovery:** kept as literal text.
    /// **Severity:** Error.
    UnexpectedPipe,
    /// **Trigger:** an attribute value opened with `"` and not closed before
    /// the next marker or end of input.
    /// **Recovery:** the value ends where the marker begins.
    /// **Severity:** Error.
    UnterminatedAttributeValue,
    /// **Trigger:** a line break inside an attribute list.
    /// **Recovery:** treated as a space.
    /// **Severity:** Error.
    NewlineInAttributes,
    /// **Trigger:** `|` followed by no attribute at all on a character style
    /// (`\w word| \w*`). The `|` announces an attribute list, so the value
    /// the author meant to write is missing.
    /// **Recovery:** the style carries an empty attribute list.
    /// **Severity:** Error. On a milestone the same shape is
    /// [`Code::EmptyMilestoneAttributeList`] instead.
    /// **Reported by:** `usfm_semantic` (ticket 20), at the `|`.
    EmptyAttributeList,
    /// **Trigger:** `|` followed by no attribute at all on a milestone
    /// (`\ts-s |\*`). unfoldingWord's aligned texts write their translation
    /// sections this way, and a milestone carries nothing but its
    /// attributes, so there is no value to have lost: `\ts-s |\*` and
    /// `\ts-s\*` describe the same milestone.
    /// **Recovery:** none. The milestone is kept, with no attributes, and
    /// the output is the same either way (`<ms style="ts-s"/>`).
    /// **Severity:** Warning. The empty list is not USFM, but nothing about
    /// the document is a guess.
    /// **Reported by:** `usfm_semantic` (ticket 20), at the `|`. It reads
    /// `Milestone::attributes`, which is `Some` with no pairs for exactly
    /// this shape, so an unknown milestone (`\zaln-s |\*`) reports it too.
    EmptyMilestoneAttributeList,
    /// **Trigger:** a bare value (`|value`) on a marker that has no default
    /// attribute, such as `\em` or `\fig`.
    /// **Recovery:** kept in the tree with an empty name; USX serializers
    /// drop it, since there is no attribute name to write.
    /// **Severity:** Error.
    /// **Reported by:** `usfm_semantic` (ticket 20), at the `|`.
    NoDefaultAttribute,
    /// **Trigger:** a bare value alongside named attributes
    /// (`|grace strong="H1234"`). USFM allows the unnamed form only when
    /// it is the sole attribute.
    /// **Recovery:** the bare value is kept as the default attribute.
    /// **Severity:** Error.
    /// **Reported by:** `usfm_semantic` (ticket 20), at the `|`.
    DefaultAttributeWithOthers,
    /// **Trigger:** a named attribute whose value is not in double quotes
    /// (`lemma=grace`).
    /// **Recovery:** the single word after `=` is the value.
    /// **Severity:** Error.
    AttributeValueNotQuoted,
    /// **Trigger:** `name=` followed by no value (`|lemma= strong="l"`).
    /// **Recovery:** the attribute is kept with an empty value.
    /// **Severity:** Error.
    MissingAttributeValue,
    /// **Trigger:** a named attribute whose name is not an identifier
    /// (`\w a|b<c="1"\w*`). USFM attribute names are ASCII letters, digits,
    /// `-`, `_` and `.`, starting with a letter or `_`.
    /// **Recovery:** kept in the tree with its name as written; USX
    /// serializers drop it, since the name is not a valid XML name.
    /// **Severity:** Error.
    /// **Reported by:** `usfm_semantic` (ticket 20), at the name.
    MalformedAttributeName,
    /// **Trigger:** the same attribute name twice in one list, including two
    /// bare values, which are both the marker's default attribute
    /// (`\rb b|"h=c"`).
    /// **Recovery:** every occurrence is kept in the tree; USX serializers
    /// write the first and drop the rest, since XML has no repeated
    /// attribute.
    /// **Severity:** Error.
    /// **Reported by:** `usfm_semantic` (ticket 20), at the second and each
    /// later occurrence of the name.
    DuplicateAttribute,
    /// **Trigger:** the parser reached a state its own invariants say is
    /// impossible. Parsing stops at this point.
    /// **Recovery:** none; the document is whatever was parsed so far.
    /// **Severity:** Error. Please report the input.
    Internal,
}

impl Code {
    /// Every code, for exhaustive tests.
    pub const ALL: &'static [Code] = &[
        Code::UnknownMarker,
        Code::UnknownCustomMarker,
        Code::UnknownMilestone,
        Code::UnknownCustomMilestone,
        Code::UnmatchedClosingMarker,
        Code::ParagraphMarkerClosed,
        Code::UnmatchedMilestoneEnd,
        Code::MilestoneNotClosed,
        Code::StrayBackslash,
        Code::MarkerNotAllowedHere,
        Code::MarkerNotListedHere,
        Code::NestedMarkerNotNested,
        Code::CharacterStyleNotClosed,
        Code::CharacterStyleImplicitlyClosed,
        Code::CharacterStyleNestedWithoutPlus,
        Code::FigureNotClosed,
        Code::EmptyWord,
        Code::NoteNotClosed,
        Code::MissingNoteCaller,
        Code::MissingVerseNumber,
        Code::MalformedVerseNumber,
        Code::VerseInNote,
        Code::VerseInCharacterStyle,
        Code::VerseInHeading,
        Code::VerseOutsideChapter,
        Code::VerseTextBeforeChapter,
        Code::DuplicateVerseNumber,
        Code::VerseOutOfOrder,
        Code::DuplicateChapterNumber,
        Code::ChapterOutOfOrder,
        Code::MissingChapterNumber,
        Code::MalformedChapterNumber,
        Code::AlternateChapterNotClosed,
        Code::AlternateVerseNotClosed,
        Code::MissingBookCode,
        Code::UnknownBookCode,
        Code::UnlistedBookCode,
        Code::MissingId,
        Code::IdNotFirst,
        Code::EmptyBook,
        Code::SidebarNotClosed,
        Code::UnmatchedSidebarEnd,
        Code::ContentOutsideParagraph,
        Code::ContentDropped,
        Code::ExpectedTableCell,
        Code::UnexpectedTableColumn,
        Code::UnexpectedPipe,
        Code::UnterminatedAttributeValue,
        Code::NewlineInAttributes,
        Code::EmptyAttributeList,
        Code::EmptyMilestoneAttributeList,
        Code::NoDefaultAttribute,
        Code::DefaultAttributeWithOthers,
        Code::AttributeValueNotQuoted,
        Code::MissingAttributeValue,
        Code::MalformedAttributeName,
        Code::DuplicateAttribute,
        Code::NumberHasLeadingZero,
        Code::Internal,
    ];

    /// Stable, machine-readable name in kebab case.
    pub fn as_str(self) -> &'static str {
        match self {
            Code::UnknownMarker => "unknown-marker",
            Code::UnknownCustomMarker => "unknown-custom-marker",
            Code::UnknownMilestone => "unknown-milestone",
            Code::UnknownCustomMilestone => "unknown-custom-milestone",
            Code::UnmatchedClosingMarker => "unmatched-closing-marker",
            Code::ParagraphMarkerClosed => "paragraph-marker-closed",
            Code::UnmatchedMilestoneEnd => "unmatched-milestone-end",
            Code::MilestoneNotClosed => "milestone-not-closed",
            Code::StrayBackslash => "stray-backslash",
            Code::MarkerNotAllowedHere => "marker-not-allowed-here",
            Code::MarkerNotListedHere => "marker-not-listed-here",
            Code::NestedMarkerNotNested => "nested-marker-not-nested",
            Code::CharacterStyleNotClosed => "character-style-not-closed",
            Code::CharacterStyleImplicitlyClosed => "character-style-implicitly-closed",
            Code::CharacterStyleNestedWithoutPlus => "character-style-nested-without-plus",
            Code::FigureNotClosed => "figure-not-closed",
            Code::EmptyWord => "empty-word",
            Code::NoteNotClosed => "note-not-closed",
            Code::MissingNoteCaller => "missing-note-caller",
            Code::MissingVerseNumber => "missing-verse-number",
            Code::MalformedVerseNumber => "malformed-verse-number",
            Code::VerseInNote => "verse-in-note",
            Code::VerseInCharacterStyle => "verse-in-character-style",
            Code::VerseInHeading => "verse-in-heading",
            Code::VerseOutsideChapter => "verse-outside-chapter",
            Code::VerseTextBeforeChapter => "verse-text-before-chapter",
            Code::DuplicateVerseNumber => "duplicate-verse-number",
            Code::VerseOutOfOrder => "verse-out-of-order",
            Code::DuplicateChapterNumber => "duplicate-chapter-number",
            Code::ChapterOutOfOrder => "chapter-out-of-order",
            Code::MissingChapterNumber => "missing-chapter-number",
            Code::MalformedChapterNumber => "malformed-chapter-number",
            Code::AlternateChapterNotClosed => "alternate-chapter-not-closed",
            Code::AlternateVerseNotClosed => "alternate-verse-not-closed",
            Code::MissingBookCode => "missing-book-code",
            Code::UnknownBookCode => "unknown-book-code",
            Code::UnlistedBookCode => "unlisted-book-code",
            Code::MissingId => "missing-id",
            Code::IdNotFirst => "id-not-first",
            Code::EmptyBook => "empty-book",
            Code::SidebarNotClosed => "sidebar-not-closed",
            Code::UnmatchedSidebarEnd => "unmatched-sidebar-end",
            Code::ContentOutsideParagraph => "content-outside-paragraph",
            Code::ContentDropped => "content-dropped",
            Code::ExpectedTableCell => "expected-table-cell",
            Code::UnexpectedTableColumn => "unexpected-table-column",
            Code::UnexpectedPipe => "unexpected-pipe",
            Code::UnterminatedAttributeValue => "unterminated-attribute-value",
            Code::NewlineInAttributes => "newline-in-attributes",
            Code::EmptyAttributeList => "empty-attribute-list",
            Code::EmptyMilestoneAttributeList => "empty-milestone-attribute-list",
            Code::NoDefaultAttribute => "no-default-attribute",
            Code::DefaultAttributeWithOthers => "default-attribute-with-others",
            Code::AttributeValueNotQuoted => "attribute-value-not-quoted",
            Code::MissingAttributeValue => "missing-attribute-value",
            Code::MalformedAttributeName => "malformed-attribute-name",
            Code::DuplicateAttribute => "duplicate-attribute",
            Code::NumberHasLeadingZero => "number-has-leading-zero",
            Code::Internal => "internal",
        }
    }

    /// The severity this code always carries.
    pub fn severity(self) -> Severity {
        match self {
            Code::UnknownCustomMarker
            | Code::UnknownMilestone
            | Code::NestedMarkerNotNested
            | Code::CharacterStyleNotClosed
            | Code::VerseInCharacterStyle
            | Code::EmptyMilestoneAttributeList
            | Code::UnlistedBookCode
            | Code::DuplicateVerseNumber
            | Code::VerseOutOfOrder
            | Code::DuplicateChapterNumber
            | Code::ChapterOutOfOrder => Severity::Warning,
            Code::CharacterStyleImplicitlyClosed
            | Code::CharacterStyleNestedWithoutPlus
            | Code::MarkerNotListedHere
            | Code::UnknownCustomMilestone => Severity::Info,
            _ => Severity::Error,
        }
    }

    /// Whether this code is reported by the semantic pass (`usfm_semantic`)
    /// rather than by the parser.
    ///
    /// The parser repairs and reports the repair; the semantic pass reads the
    /// finished tree and reports a judgement about it (M4 in
    /// `.scratch/oxc-layout/spec.md`). Which side a code falls on is not
    /// visible from the variant, so it is recorded here once and read by both
    /// crates' coverage tests: `usfm_parser/tests/recovery.rs` skips the codes
    /// it names, and `usfm_semantic/tests/checks.rs` requires a snapshot for
    /// each of them. A code moving from the parser to the semantic pass is
    /// therefore one line here plus a moved snapshot.
    ///
    /// ```
    /// use usfm_diagnostics::Code;
    /// assert!(Code::UnlistedBookCode.is_semantic());
    /// assert!(!Code::UnknownBookCode.is_semantic());
    /// ```
    pub fn is_semantic(self) -> bool {
        matches!(
            self,
            Code::UnlistedBookCode
                | Code::MarkerNotAllowedHere
                | Code::MarkerNotListedHere
                | Code::EmptyAttributeList
                | Code::EmptyMilestoneAttributeList
                | Code::NoDefaultAttribute
                | Code::DefaultAttributeWithOthers
                | Code::MalformedAttributeName
                | Code::DuplicateAttribute
                | Code::MissingId
                | Code::IdNotFirst
                | Code::EmptyBook
                | Code::VerseTextBeforeChapter
                | Code::VerseOutsideChapter
                | Code::VerseInHeading
                | Code::VerseInCharacterStyle
                | Code::UnexpectedTableColumn
                | Code::EmptyWord
                | Code::DuplicateVerseNumber
                | Code::VerseOutOfOrder
                | Code::DuplicateChapterNumber
                | Code::ChapterOutOfOrder
        )
    }

    /// The inverse of [`Code::as_str`]: the code with that name, if there is
    /// one.
    ///
    /// The language server and the CLI's `--deny <code>` key on these names,
    /// so this is a lookup over [`Code::ALL`] rather than a second `match`
    /// that could drift out of step with `as_str`.
    ///
    /// ```
    /// use usfm_diagnostics::Code;
    /// assert_eq!(Code::parse("unknown-marker"), Some(Code::UnknownMarker));
    /// assert_eq!(Code::parse("UnknownMarker"), None);
    /// ```
    pub fn parse(name: &str) -> Option<Code> {
        Code::ALL.iter().copied().find(|c| c.as_str() == name)
    }
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The name of a [`Code`] that is not one of ours.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownCode(pub String);

impl fmt::Display for UnknownCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown diagnostic code: {}", self.0)
    }
}

impl std::error::Error for UnknownCode {}

impl FromStr for Code {
    type Err = UnknownCode;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Code::parse(name).ok_or_else(|| UnknownCode(name.to_owned()))
    }
}

/// One thing the parser had to guess about, repair, or drop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Byte range in the source the diagnostic refers to.
    pub span: Span,
    pub severity: Severity,
    pub code: Code,
    /// Human-readable description including the specific marker or text.
    pub message: String,
}

impl Diagnostic {
    pub fn new(code: Code, span: Span, message: impl Into<String>) -> Self {
        Self {
            span,
            severity: code.severity(),
            code,
            message: message.into(),
        }
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }

    /// One line of human-readable output:
    /// `label:line:col: severity[code]: message`, with no trailing newline.
    ///
    /// `label` names the input the diagnostic came from (a path, or `input` /
    /// `diglot` when the CLI concatenated several files). `index` is the
    /// [`LineIndex`] of that input's source, built once for the whole file:
    /// the position printed is the start of [`Diagnostic::span`].
    ///
    /// ```
    /// use usfm_diagnostics::{Code, Diagnostic};
    /// use usfm_span::{LineIndex, Span};
    ///
    /// let source = "\\p one\n\\zz two\n";
    /// let diagnostic = Diagnostic::new(
    ///     Code::UnknownCustomMarker,
    ///     Span::new(7, 10),
    ///     "unknown marker \\zz",
    /// );
    /// assert_eq!(
    ///     diagnostic.render("book.usfm", &LineIndex::new(source)),
    ///     "book.usfm:2:1: warning[unknown-custom-marker]: unknown marker \\zz",
    /// );
    /// ```
    pub fn render(&self, label: &str, index: &LineIndex) -> String {
        let (line, col) = index.line_col(self.span.start);
        format!(
            "{label}:{line}:{col}: {}[{}]: {}",
            self.severity, self.code, self.message
        )
    }

    /// One JSON object on one line, for `--diagnostics json`: the same
    /// information as [`Diagnostic::render`] plus the byte span, as
    /// `{"file":…,"line":…,"col":…,"severity":…,"code":…,"message":…,"span":[start,end]}`.
    ///
    /// Written by hand — the key order is fixed and the strings are escaped by
    /// [`escape_json`] — so that neither this crate nor anything downstream
    /// needs `serde`. There is no trailing newline; a caller printing a stream
    /// of these writes one object per line.
    ///
    /// ```
    /// use usfm_diagnostics::{Code, Diagnostic};
    /// use usfm_span::{LineIndex, Span};
    ///
    /// let source = "\\p one\n\\zz two\n";
    /// let diagnostic = Diagnostic::new(
    ///     Code::UnknownCustomMarker,
    ///     Span::new(7, 10),
    ///     "unknown marker \\zz",
    /// );
    /// assert_eq!(
    ///     diagnostic.to_json_line("book.usfm", &LineIndex::new(source)),
    ///     r#"{"file":"book.usfm","line":2,"col":1,"severity":"warning","code":"unknown-custom-marker","message":"unknown marker \\zz","span":[7,10]}"#,
    /// );
    /// ```
    pub fn to_json_line(&self, label: &str, index: &LineIndex) -> String {
        let (line, col) = index.line_col(self.span.start);
        format!(
            concat!(
                r#"{{"file":"{}","line":{},"col":{},"severity":"{}","#,
                r#""code":"{}","message":"{}","span":[{},{}]}}"#,
            ),
            escape_json(label),
            line,
            col,
            self.severity,
            self.code,
            escape_json(&self.message),
            self.span.start,
            self.span.end,
        )
    }
}

/// Escape a string for a JSON string literal: `"` and `\`, the short forms
/// JSON gives `\n`, `\r`, `\t`, `\u0008` and `\u000c`, and every other control
/// character as `\uXXXX`. Everything else, including non-ASCII text, is copied
/// through as UTF-8, which JSON allows.
///
/// ```
/// use usfm_diagnostics::escape_json;
/// assert_eq!(escape_json(r#"say "hi"\"#), r#"say \"hi\"\\"#);
/// assert_eq!(escape_json("a\nb\u{1}"), r"a\nb\u0001");
/// assert_eq!(escape_json("héllo"), "héllo");
/// ```
pub fn escape_json(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}[{}] @{}..{}: {}",
            self.severity, self.code, self.span.start, self.span.end, self.message
        )
    }
}

/// The outcome of parsing: always a document, plus everything the parser
/// had to say about the input.
#[derive(Debug)]
pub struct ParseResult<'a> {
    pub document: Document<'a>,
    pub diagnostics: Vec<Diagnostic>,
}

impl<'a> ParseResult<'a> {
    /// True if any diagnostic is at or above `threshold`.
    pub fn has_at_least(&self, threshold: Severity) -> bool {
        self.diagnostics.iter().any(|d| d.severity >= threshold)
    }

    /// True if any diagnostic is an error.
    pub fn has_errors(&self) -> bool {
        self.has_at_least(Severity::Error)
    }

    /// Iterate diagnostics at or above `threshold`.
    pub fn diagnostics_at_least(
        &self,
        threshold: Severity,
    ) -> impl Iterator<Item = &Diagnostic> + '_ {
        self.diagnostics
            .iter()
            .filter(move |d| d.severity >= threshold)
    }

    /// Strict policy: succeed only if no diagnostic is an error.
    ///
    /// The parser recovers from everything; this is where a production
    /// pipeline chooses not to accept a repaired tree.
    pub fn strict(self) -> Result<Document<'a>, Vec<Diagnostic>> {
        self.strict_with(Severity::Error)
    }

    /// Strict policy with a custom threshold (for example
    /// `Severity::Warning` to also reject Paratext-isms).
    pub fn strict_with(self, threshold: Severity) -> Result<Document<'a>, Vec<Diagnostic>> {
        if self.has_at_least(threshold) {
            Err(self.diagnostics)
        } else {
            Ok(self.document)
        }
    }
}

/// Convert a byte offset into a 1-based (line, column) pair, counting
/// columns in characters. Offsets past the end of the source map to the
/// last position.
///
/// This builds a [`LineIndex`] and throws it away, so it scans the whole
/// source once per call. Converting more than one offset of the same source —
/// printing a list of diagnostics, say — should build the index once and call
/// [`LineIndex::line_col`] instead.
pub fn line_col(source: &str, offset: u32) -> (usize, usize) {
    LineIndex::new(source).line_col(offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_ordering() {
        assert!(Severity::Error > Severity::Warning);
        assert!(Severity::Warning > Severity::Info);
    }

    #[test]
    fn line_col_positions() {
        let source = "ab\ncdé\nf";
        assert_eq!(line_col(source, 0), (1, 1));
        assert_eq!(line_col(source, 2), (1, 3));
        assert_eq!(line_col(source, 3), (2, 1));
        assert_eq!(line_col(source, 7), (2, 4)); // after the two-byte é
        assert_eq!(line_col(source, 8), (3, 1));
        assert_eq!(line_col(source, 100), (3, 2));
    }

    /// The variant declared after `code`, or `None` at the end of the enum.
    ///
    /// An exhaustive `match` with no wildcard arm, so a new `Code` does not
    /// compile until it is given a place in this chain. [`all_variants`] walks
    /// the chain rather than repeating a hand-written list, which is what makes
    /// it complete: a variant left out of the chain has nowhere to hide.
    fn next_variant(code: Code) -> Option<Code> {
        Some(match code {
            Code::UnknownMarker => Code::UnknownCustomMarker,
            Code::UnknownCustomMarker => Code::UnknownMilestone,
            Code::UnknownMilestone => Code::UnknownCustomMilestone,
            Code::UnknownCustomMilestone => Code::UnmatchedClosingMarker,
            Code::UnmatchedClosingMarker => Code::ParagraphMarkerClosed,
            Code::ParagraphMarkerClosed => Code::UnmatchedMilestoneEnd,
            Code::UnmatchedMilestoneEnd => Code::MilestoneNotClosed,
            Code::MilestoneNotClosed => Code::StrayBackslash,
            Code::StrayBackslash => Code::MarkerNotAllowedHere,
            Code::MarkerNotAllowedHere => Code::MarkerNotListedHere,
            Code::MarkerNotListedHere => Code::NestedMarkerNotNested,
            Code::NestedMarkerNotNested => Code::CharacterStyleNotClosed,
            Code::CharacterStyleNotClosed => Code::CharacterStyleImplicitlyClosed,
            Code::CharacterStyleImplicitlyClosed => Code::CharacterStyleNestedWithoutPlus,
            Code::CharacterStyleNestedWithoutPlus => Code::FigureNotClosed,
            Code::FigureNotClosed => Code::EmptyWord,
            Code::EmptyWord => Code::NoteNotClosed,
            Code::NoteNotClosed => Code::MissingNoteCaller,
            Code::MissingNoteCaller => Code::MissingVerseNumber,
            Code::MissingVerseNumber => Code::MalformedVerseNumber,
            Code::MalformedVerseNumber => Code::VerseInNote,
            Code::VerseInNote => Code::VerseInCharacterStyle,
            Code::VerseInCharacterStyle => Code::VerseInHeading,
            Code::VerseInHeading => Code::VerseOutsideChapter,
            Code::VerseOutsideChapter => Code::VerseTextBeforeChapter,
            Code::VerseTextBeforeChapter => Code::DuplicateVerseNumber,
            Code::DuplicateVerseNumber => Code::VerseOutOfOrder,
            Code::VerseOutOfOrder => Code::DuplicateChapterNumber,
            Code::DuplicateChapterNumber => Code::ChapterOutOfOrder,
            Code::ChapterOutOfOrder => Code::MissingChapterNumber,
            Code::MissingChapterNumber => Code::MalformedChapterNumber,
            Code::MalformedChapterNumber => Code::NumberHasLeadingZero,
            Code::NumberHasLeadingZero => Code::AlternateChapterNotClosed,
            Code::AlternateChapterNotClosed => Code::AlternateVerseNotClosed,
            Code::AlternateVerseNotClosed => Code::MissingBookCode,
            Code::MissingBookCode => Code::UnknownBookCode,
            Code::UnknownBookCode => Code::UnlistedBookCode,
            Code::UnlistedBookCode => Code::MissingId,
            Code::MissingId => Code::IdNotFirst,
            Code::IdNotFirst => Code::EmptyBook,
            Code::EmptyBook => Code::SidebarNotClosed,
            Code::SidebarNotClosed => Code::UnmatchedSidebarEnd,
            Code::UnmatchedSidebarEnd => Code::ContentOutsideParagraph,
            Code::ContentOutsideParagraph => Code::ContentDropped,
            Code::ContentDropped => Code::UnexpectedTableColumn,
            Code::UnexpectedTableColumn => Code::ExpectedTableCell,
            Code::ExpectedTableCell => Code::UnexpectedPipe,
            Code::UnexpectedPipe => Code::UnterminatedAttributeValue,
            Code::UnterminatedAttributeValue => Code::NewlineInAttributes,
            Code::NewlineInAttributes => Code::EmptyAttributeList,
            Code::EmptyAttributeList => Code::EmptyMilestoneAttributeList,
            Code::EmptyMilestoneAttributeList => Code::NoDefaultAttribute,
            Code::NoDefaultAttribute => Code::DefaultAttributeWithOthers,
            Code::DefaultAttributeWithOthers => Code::AttributeValueNotQuoted,
            Code::AttributeValueNotQuoted => Code::MissingAttributeValue,
            Code::MissingAttributeValue => Code::MalformedAttributeName,
            Code::MalformedAttributeName => Code::DuplicateAttribute,
            Code::DuplicateAttribute => Code::Internal,
            Code::Internal => return None,
        })
    }

    /// Every variant, in declaration order, built by walking [`next_variant`]
    /// from the first one.
    fn all_variants() -> Vec<Code> {
        let mut chain = vec![Code::UnknownMarker];
        while let Some(next) = next_variant(*chain.last().unwrap()) {
            assert!(
                !chain.contains(&next),
                "{next:?} appears twice in next_variant's chain"
            );
            chain.push(next);
        }
        // `Code` is a fieldless enum with default discriminants, so
        // `code as usize` is its position in the declaration. Checking the
        // chain runs 0, 1, 2, … proves it visits consecutive variants: a
        // variant inserted anywhere but after the last one leaves a gap here.
        // (Which is why `Internal`, the catch-all, stays last.)
        for (i, code) in chain.iter().enumerate() {
            assert_eq!(
                *code as usize, i,
                "next_variant skips the variant declared before {code:?}"
            );
        }
        chain
    }

    #[test]
    fn all_is_every_variant() {
        let every = all_variants();
        for code in &every {
            assert!(
                Code::ALL.contains(code),
                "{code:?} is missing from Code::ALL"
            );
        }
        assert_eq!(
            Code::ALL.len(),
            every.len(),
            "Code::ALL has an entry that is not a variant, or a duplicate"
        );
    }

    #[test]
    fn every_code_has_a_unique_name() {
        let mut names: Vec<_> = Code::ALL.iter().map(|c| c.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), Code::ALL.len());
    }

    /// The names are the CLI's `--deny <code>` and the language server's
    /// `Diagnostic.code`, so they are an interface: kebab case, no
    /// underscores, no capitals, no leading or doubled `-`.
    #[test]
    fn every_code_name_is_kebab_case() {
        for code in Code::ALL {
            let name = code.as_str();
            assert!(!name.is_empty(), "{code:?} has an empty name");
            for (i, word) in name.split('-').enumerate() {
                assert!(
                    !word.is_empty(),
                    "{name:?} has an empty segment (leading, trailing or doubled `-`)"
                );
                assert!(
                    word.bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()),
                    "{name:?} has a segment that is not lowercase ASCII or digits: {word:?}"
                );
                if i == 0 {
                    assert!(
                        word.as_bytes()[0].is_ascii_lowercase(),
                        "{name:?} does not start with a letter"
                    );
                }
            }
        }
    }

    #[test]
    fn code_name_round_trips() {
        for code in Code::ALL {
            assert_eq!(Code::parse(code.as_str()), Some(*code));
            assert_eq!(code.as_str().parse::<Code>(), Ok(*code));
            // `Display` is the name too, so the round trip closes either way.
            assert_eq!(Code::parse(&code.to_string()), Some(*code));
        }
        assert_eq!(Code::parse("no-such-code"), None);
        assert_eq!(
            "no-such-code".parse::<Code>(),
            Err(UnknownCode("no-such-code".to_owned()))
        );
    }

    /// The exact line `main.rs` printed before this moved here.
    #[test]
    fn render_is_the_cli_line() {
        let source = "\\id GEN\n\\p text\n\\zz oops\n";
        let index = LineIndex::new(source);
        let start = source.find("\\zz").unwrap() as u32;
        let diagnostic = Diagnostic::new(
            Code::UnknownCustomMarker,
            Span::new(start, start + 3),
            "unknown marker \\zz",
        );
        assert_eq!(
            diagnostic.render("book.usfm", &index),
            "book.usfm:3:1: warning[unknown-custom-marker]: unknown marker \\zz"
        );
        assert!(!diagnostic.render("book.usfm", &index).ends_with('\n'));
    }

    #[test]
    fn render_counts_columns_in_characters() {
        let source = "\\p ré\\zz\n";
        let index = LineIndex::new(source);
        let start = source.find("\\zz").unwrap() as u32;
        let diagnostic =
            Diagnostic::new(Code::UnknownCustomMarker, Span::new(start, start + 3), "x");
        // `é` is two bytes at offset 4, so the marker is byte 6 but column 6.
        assert_eq!(start, 6);
        assert_eq!(
            diagnostic.render("book.usfm", &index),
            "book.usfm:1:6: warning[unknown-custom-marker]: x"
        );
    }

    #[test]
    fn json_line_shape() {
        let source = "\\id GEN\n\\p text\n";
        let index = LineIndex::new(source);
        let diagnostic = Diagnostic::new(Code::UnknownMarker, Span::new(8, 10), "unknown marker");
        assert_eq!(
            diagnostic.to_json_line("book.usfm", &index),
            r#"{"file":"book.usfm","line":2,"col":1,"severity":"error","code":"unknown-marker","message":"unknown marker","span":[8,10]}"#
        );
    }

    /// A quote, a backslash and a newline in the message, and a space in the
    /// label: the JSON stays one line and one object.
    #[test]
    fn json_line_escapes_message_and_label() {
        let index = LineIndex::new("x");
        let diagnostic = Diagnostic::new(
            Code::StrayBackslash,
            Span::new(0, 1),
            "stray \"\\\" before\na new line\tand a tab",
        );
        let line = diagnostic.to_json_line("my book.usfm", &index);
        assert_eq!(
            line,
            r#"{"file":"my book.usfm","line":1,"col":1,"severity":"error","code":"stray-backslash","message":"stray \"\\\" before\na new line\tand a tab","span":[0,1]}"#
        );
        // One line: the message's newline and tab are escape sequences, not
        // the characters themselves.
        assert_eq!(line.lines().count(), 1);
        assert!(!line.contains('\t'));
    }

    #[test]
    fn escape_json_control_characters() {
        assert_eq!(escape_json("\u{0}\u{1}\u{1f}"), r"\u0000\u0001\u001f");
        assert_eq!(escape_json("\u{8}\u{c}\r"), r"\b\f\r");
        // Not control characters: copied through, including non-ASCII.
        assert_eq!(escape_json("plain ünïcode /"), "plain ünïcode /");
    }
}
