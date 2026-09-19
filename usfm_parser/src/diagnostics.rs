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
//! `tests/recovery.rs`.

use std::fmt;

use crate::ast::Document;
use crate::lexer::span::Span;

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
    /// the next marker or end of input.
    /// **Recovery:** the milestone is closed where the `\*` should have been.
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
    MarkerNotAllowedHere,
    /// **Trigger:** any other character style or note opened under a marker
    /// its `OccursUnder` does not list, such as `\f` under `\cl`. The
    /// stylesheet lists are conservative and Paratext accepts these, so this
    /// only informs. Nesting inside a character style is governed by `NEST`
    /// instead, and a note's parent is its paragraph whatever character
    /// styles are open around it.
    /// **Recovery:** the style is parsed where it is.
    /// **Severity:** Info.
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
    VerseInCharacterStyle,
    /// **Trigger:** `\v` in a title or section heading paragraph. The
    /// unfoldingWord chunk marker `\s5` is exempt: by convention it is an
    /// empty heading placed directly before a verse.
    /// **Recovery:** none; the verse is kept where it is.
    /// **Severity:** Error.
    VerseInHeading,
    /// **Trigger:** `\v` before any `\c`.
    /// **Recovery:** none; the verse is kept where it is.
    /// **Severity:** Error.
    VerseOutsideChapter,
    /// **Trigger:** in a scripture book, a verse-text paragraph (`\p`,
    /// `\q1`, …) before the first `\c`.
    /// **Recovery:** none.
    /// **Severity:** Error.
    VerseTextBeforeChapter,
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
    /// **Severity:** Error.
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
    /// **Trigger:** `\id` followed by a code that is not a known book code.
    /// **Recovery:** the `\id` line is dropped.
    /// **Severity:** Error.
    UnknownBookCode,
    /// **Trigger:** the document does not start with `\id`.
    /// **Recovery:** none.
    /// **Severity:** Error.
    MissingId,
    /// **Trigger:** `\id` after other content.
    /// **Recovery:** the book is kept.
    /// **Severity:** Error.
    IdNotFirst,
    /// **Trigger:** a document containing only an `\id` line.
    /// **Recovery:** none.
    /// **Severity:** Error.
    EmptyBook,
    /// **Trigger:** `\esb` while a sidebar is already open, or still open
    /// at end of input.
    /// **Recovery:** none.
    /// **Severity:** Error.
    SidebarNotClosed,
    /// **Trigger:** `\esbe` with no open sidebar.
    /// **Recovery:** none.
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
    /// **Trigger:** `|` followed by no attribute at all (`\w word| \w*`).
    /// **Recovery:** the style carries an empty attribute list.
    /// **Severity:** Error.
    EmptyAttributeList,
    /// **Trigger:** a bare value (`|value`) on a marker that has no default
    /// attribute, such as `\em` or `\fig`.
    /// **Recovery:** kept in the tree with an empty name; USX serializers
    /// drop it, since there is no attribute name to write.
    /// **Severity:** Error.
    NoDefaultAttribute,
    /// **Trigger:** a bare value alongside named attributes
    /// (`|grace strong="H1234"`). USFM allows the unnamed form only when
    /// it is the sole attribute.
    /// **Recovery:** the bare value is kept as the default attribute.
    /// **Severity:** Error.
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
    MalformedAttributeName,
    /// **Trigger:** the same attribute name twice in one list, including two
    /// bare values, which are both the marker's default attribute
    /// (`\rb b|"h=c"`).
    /// **Recovery:** every occurrence is kept in the tree; USX serializers
    /// write the first and drop the rest, since XML has no repeated
    /// attribute.
    /// **Severity:** Error.
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
        Code::MissingChapterNumber,
        Code::MalformedChapterNumber,
        Code::AlternateChapterNotClosed,
        Code::AlternateVerseNotClosed,
        Code::MissingBookCode,
        Code::UnknownBookCode,
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
            Code::MissingChapterNumber => "missing-chapter-number",
            Code::MalformedChapterNumber => "malformed-chapter-number",
            Code::AlternateChapterNotClosed => "alternate-chapter-not-closed",
            Code::AlternateVerseNotClosed => "alternate-verse-not-closed",
            Code::MissingBookCode => "missing-book-code",
            Code::UnknownBookCode => "unknown-book-code",
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
            | Code::VerseInCharacterStyle => Severity::Warning,
            Code::CharacterStyleImplicitlyClosed
            | Code::CharacterStyleNestedWithoutPlus
            | Code::MarkerNotListedHere
            | Code::UnknownCustomMilestone => Severity::Info,
            _ => Severity::Error,
        }
    }
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
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
pub fn line_col(source: &str, offset: u32) -> (usize, usize) {
    let offset = (offset as usize).min(source.len());
    let before = &source[..offset];
    let line = before.matches('\n').count() + 1;
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    let col = before[line_start..].chars().count() + 1;
    (line, col)
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

    #[test]
    fn every_code_has_a_unique_name() {
        let mut names: Vec<_> = Code::ALL.iter().map(|c| c.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), Code::ALL.len());
    }
}
