//! One test per recovery rule.
//!
//! Each test feeds the parser a minimal input that triggers one
//! [`Code`], asserts that code was emitted, and snapshots the exact
//! recovered tree plus diagnostics. The snapshot is the executable form of
//! the recovery table documented on `Code`.
//!
//! Snapshots live in `tests/snapshots/recovery__<code>.snap`.
//!
//! **Every [`Code`] has a test in this file or in
//! `usfm_semantic/tests/checks.rs`**, and [`Code::is_semantic`] says which.
//! A code the parser repairs and reports is tested here; a code the semantic
//! pass reports about a tree that parsed exactly as written is tested there,
//! through `usfm::parse`. [`recovery_table_is_covered`] skips the semantic
//! codes for that reason, and the check test file requires them.

mod common;

use usfm_parser::diagnostics::Code;

/// Snapshot named `recovery__<code>`.
fn check(code: Code, source: &str) {
    check_named(code, code.as_str().replace('-', "_"), source);
}

/// Snapshot named `recovery__<code>__<variant>`, for additional cases of
/// the same code.
fn check_variant(code: Code, variant: &str, source: &str) {
    check_named(
        code,
        format!("{}__{variant}", code.as_str().replace('-', "_")),
        source,
    );
}

/// Snapshot the recovered tree without requiring a diagnostic, for rules
/// whose correct behaviour is to report nothing.
fn snapshot(name: &str, source: &str) {
    let rendered = common::render(source);
    assert!(
        !rendered.starts_with("PANIC"),
        "parser panicked: {rendered}"
    );
    insta::with_settings!({
        snapshot_path => "snapshots",
        prepend_module_to_snapshot => false,
        description => source,
        omit_expression => true,
    }, {
        insta::assert_snapshot!(format!("recovery__{name}"), rendered);
    });
}

fn check_named(code: Code, name: String, source: &str) {
    let codes = common::codes(source);
    assert!(
        codes.contains(&code),
        "expected {code:?} in diagnostics, got {codes:?}\nsource: {source}"
    );
    let rendered = common::render(source);
    assert!(
        !rendered.starts_with("PANIC"),
        "parser panicked: {rendered}"
    );
    insta::with_settings!({
        snapshot_path => "snapshots",
        prepend_module_to_snapshot => false,
        description => source,
        omit_expression => true,
    }, {
        insta::assert_snapshot!(format!("recovery__{name}"), rendered);
    });
}

#[test]
fn unknown_marker() {
    check(
        Code::UnknownMarker,
        "\\id GEN\n\\c 1\n\\p \\v 1 a \\foo custom \\foo* b",
    );
}

/// `\k-s`/`\k-e` are the milestone forms of `\k`, which the stylesheet does
/// define. The parser derives them and registers them on the document's own
/// stylesheet, so this is ordinary USFM and emits nothing at all.
#[test]
fn derived_milestone_of_known_base() {
    let source = "\\id GEN\n\\c 1\n\\p \\v 1 a \\k-s |x-strong=\"G1\"\\* b \\k-e\\* c";
    assert_eq!(
        common::codes(source),
        Vec::<Code>::new(),
        "deriving a milestone from a known base marker should be silent"
    );
    snapshot("derived_milestone_of_known_base", source);
}

/// A milestone the stylesheet does not define and that is not in the `\z`
/// namespace. Registered and kept in full; reported so tooling can flag it.
#[test]
fn unknown_milestone() {
    check(
        Code::UnknownMilestone,
        "\\id GEN\n\\c 1\n\\ts\\*\n\\p \\v 1 a \\ts\\* b",
    );
}

/// A `\z` custom milestone: registered and kept, reported only as info.
#[test]
fn unknown_custom_milestone() {
    check(
        Code::UnknownCustomMilestone,
        "\\id GEN\n\\c 1\n\\p \\v 1 a \\zaln-s |x-strong=\"G1\"\\* b \\zaln-e\\* c",
    );
}

#[test]
fn unmatched_closing_marker() {
    check(
        Code::UnmatchedClosingMarker,
        "\\id GEN\n\\c 1\n\\p \\v 1 a \\em b\\em* \\em* c \\x* d",
    );
}

#[test]
fn unmatched_closing_marker_does_not_cross_note() {
    // `\em*` inside the note cannot close the `\em` outside it.
    check_variant(
        Code::UnmatchedClosingMarker,
        "does_not_cross_note",
        "\\id GEN\n\\c 1\n\\p \\v 1 \\em a \\f + \\ft b \\em* c\\f* d\\em*",
    );
}

#[test]
fn paragraph_marker_closed() {
    check(
        Code::ParagraphMarkerClosed,
        "\\id GEN\n\\c 1\n\\p \\v 1 a \\p* b",
    );
}

#[test]
fn unmatched_milestone_end() {
    check(
        Code::UnmatchedMilestoneEnd,
        "\\id GEN\n\\c 1\n\\p \\v 1 a \\* b",
    );
}

#[test]
fn milestone_not_closed() {
    check(
        Code::MilestoneNotClosed,
        "\\id GEN\n\\c 1\n\\p \\v 1 a \\qt-s |who=\"Pilate\" b \\qt-e\\* c",
    );
}

#[test]
fn stray_backslash() {
    check(
        Code::StrayBackslash,
        "\\id GEN\n\\c 1\n\\p \\v 1 a \\ b \\+ c \\",
    );
}

#[test]
fn nested_marker_not_nested() {
    check(
        Code::NestedMarkerNotNested,
        "\\id GEN\n\\c 1\n\\p \\v 1 a \\+nd Lord\\+nd* b",
    );
}

#[test]
fn character_style_not_closed_by_paragraph() {
    check(
        Code::CharacterStyleNotClosed,
        "\\id GEN\n\\c 1\n\\p \\v 1 a \\em b\n\\p \\v 2 c",
    );
}

#[test]
fn character_style_not_closed_at_eof() {
    check_variant(
        Code::CharacterStyleNotClosed,
        "at_eof",
        "\\id GEN\n\\c 1\n\\p \\v 1 a \\em b",
    );
}

#[test]
fn character_style_not_closed_at_table_cell() {
    check_variant(
        Code::CharacterStyleNotClosed,
        "at_table_cell",
        "\\id GEN\n\\c 1\n\\tr \\tc1 \\em a \\tc2 b",
    );
}

#[test]
fn character_style_not_closed_by_outer_closer() {
    check_variant(
        Code::CharacterStyleNotClosed,
        "by_outer_closer",
        "\\id GEN\n\\c 1\n\\p \\v 1 \\em a \\+nd b\\em* c",
    );
}

/// `\iqt` cannot nest (no `NEST` in its `OccursUnder`), so it is a sibling
/// that closes `\em`.
#[test]
fn character_style_implicitly_closed() {
    check(
        Code::CharacterStyleImplicitlyClosed,
        "\\id GEN\n\\c 1\n\\p \\v 1 \\em a \\iqt b\\iqt* c",
    );
}

/// `\nd` may nest (`NEST`), so inside `\bk … \bk*` it nests as Paratext reads
/// it, even without the `+` that USFM 3 asks for; reported at Info so tooling
/// can offer to add it.
#[test]
fn character_style_nested_without_plus() {
    let source = "\\id GEN\n\\c 1\n\\p \\v 1 \\bk The Book of the \\nd Lord\\nd*'s Battles\\bk* speaks";
    assert_eq!(
        common::codes(source),
        vec![Code::CharacterStyleNestedWithoutPlus],
        "nesting a NEST style is the only thing to report"
    );
    check(Code::CharacterStyleNestedWithoutPlus, source);
}

/// A style that may nest but is never closed is a sibling: `\xo 1.1 \xt
/// Gen 1\x*` is two children of the note, not `\xt` inside `\xo`.
#[test]
fn unclosed_nestable_style_is_a_sibling() {
    let source = "\\id GEN\n\\c 1\n\\p \\v 1 a\\x - \\xo 1.1 \\xt Gen 1\\x* b";
    assert_eq!(common::codes(source), Vec::<Code>::new());
    snapshot("unclosed_nestable_style_is_a_sibling", source);
}

/// The lookahead stops at the enclosing style's closing marker: here `\nd*`
/// belongs to nothing, so `\nd` is a sibling and `\nd*` is unmatched.
#[test]
fn character_style_not_nested_when_closer_is_outside() {
    check_variant(
        Code::CharacterStyleImplicitlyClosed,
        "closer_outside",
        "\\id GEN\n\\c 1\n\\p \\v 1 \\em a \\nd b\\em* c\\nd*",
    );
}

/// The same rule inside a note: `\xt …\xt*` nests in `\ft`, while `\fqa`
/// (no `NEST`) is a sibling that closes `\ft`.
#[test]
fn nesting_inside_notes_follows_the_stylesheet() {
    let source = "\\id GEN\n\\c 1\n\\p \\v 1 a\\f + \\ft see \\xt Gen 1\\xt* or \\fqa quote\\fqa*\\f* b";
    assert_eq!(common::codes(source), vec![Code::CharacterStyleNestedWithoutPlus]);
    snapshot("nesting_inside_notes_follows_the_stylesheet", source);
}

#[test]
fn alternate_verse_not_closed() {
    check(
        Code::AlternateVerseNotClosed,
        "\\id GEN\n\\c 1\n\\p \\v 1 \\va 3 text",
    );
}

/// `\va`/`\vp` after the verse number are the alternate and published
/// numbers, not character styles.
#[test]
fn verse_with_alternate_and_published_numbers() {
    let source = "\\id GEN\n\\c 1\n\\p \\v 1 \\va 3\\va* \\vp 1b\\vp* text";
    assert_eq!(common::codes(source), Vec::<Code>::new());
    snapshot("verse_with_alternate_and_published_numbers", source);
}

/// A `\vp` with formatting inside cannot be a plain published number; it is
/// kept as a character style, which is how Paratext writes it too.
#[test]
fn formatted_published_verse_number_stays_a_char() {
    let source = "\\id GEN\n\\c 1\n\\p \\v 21 \\vp \\+it 21\\+it*\\vp* text";
    assert_eq!(common::codes(source), Vec::<Code>::new());
    snapshot("formatted_published_verse_number_stays_a_char", source);
}

#[test]
fn published_verse_number_not_closed() {
    check_variant(
        Code::AlternateVerseNotClosed,
        "vp",
        "\\id GEN\n\\c 1\n\\p \\v 1 \\vp 1b text\n\\p \\v 2 next",
    );
}

/// Implicit closure inside notes (`\fr ... \ft ... \f*`) is the idiom the
/// spec documents; it must produce no diagnostics at all.
#[test]
fn implicit_closure_in_note_is_silent() {
    let codes =
        common::codes("\\id GEN\n\\c 1\n\\p \\v 1 a\\f + \\fr 1.1 \\ft note \\fq quote\\f* b");
    assert_eq!(codes, vec![]);
}

#[test]
fn note_not_closed_by_paragraph() {
    check(
        Code::NoteNotClosed,
        "\\id GEN\n\\c 1\n\\p \\v 1 a\\f + \\ft note\n\\p \\v 2 b",
    );
}

#[test]
fn note_not_closed_at_eof() {
    check_variant(
        Code::NoteNotClosed,
        "at_eof",
        "\\id GEN\n\\c 1\n\\p \\v 1 a\\f + \\ft note",
    );
}

#[test]
fn missing_note_caller() {
    check(
        Code::MissingNoteCaller,
        "\\id GEN\n\\c 1\n\\p \\v 1 a\\f \\ft note\\f* b",
    );
}

/// USFM 3 allows any custom caller character, `*` included. The lexer reads a
/// bare `*` as a star because everywhere else one closes a marker, so the
/// parser takes it as the caller itself: this is ordinary USFM, not a
/// recovery, and reports nothing.
#[test]
fn star_note_caller() {
    let source = "\\id GEN\n\\c 1\n\\p \\v 1 text\\f * \\ft note\\f* more";
    assert_eq!(
        common::codes(source),
        Vec::<Code>::new(),
        "`*` is a legitimate custom caller"
    );
    snapshot("star_note_caller", source);
}

/// A caller runs to the next space, so a word written straight after the `*`
/// is part of it — the way `+abc` already is.
#[test]
fn star_note_caller_with_trailing_word() {
    let source = "\\id GEN\n\\c 1\n\\p \\v 1 text\\f *abc \\ft note\\f* more";
    assert_eq!(common::codes(source), Vec::<Code>::new());
    snapshot("star_note_caller_with_trailing_word", source);
}

#[test]
fn missing_verse_number() {
    check(
        Code::MissingVerseNumber,
        "\\id GEN\n\\c 1\n\\p \\v \\em a\\em*",
    );
}

#[test]
fn malformed_verse_number() {
    check(
        Code::MalformedVerseNumber,
        "\\id GEN\n\\c 1\n\\p \\v one a \\v 2- b",
    );
}

#[test]
fn verse_in_note() {
    check(
        Code::VerseInNote,
        "\\id GEN\n\\c 1\n\\p \\v 1 a\\f + \\ft see \\v 2 there\\f* b",
    );
}

#[test]
fn missing_chapter_number() {
    check(Code::MissingChapterNumber, "\\id GEN\n\\c\n\\p \\v 1 a");
}

#[test]
fn malformed_chapter_number() {
    check(
        Code::MalformedChapterNumber,
        "\\id GEN\n\\c one\n\\p \\v 1 a",
    );
}

#[test]
fn malformed_alternate_chapter_number() {
    check_variant(
        Code::MalformedChapterNumber,
        "alternate",
        "\\id GEN\n\\c 1 \\ca x\\ca*\n\\p \\v 1 a",
    );
}

#[test]
fn alternate_chapter_not_closed() {
    check(
        Code::AlternateChapterNotClosed,
        "\\id GEN\n\\c 1 \\ca 2\n\\p \\v 1 a",
    );
}

#[test]
fn missing_book_code() {
    check(Code::MissingBookCode, "\\id\n\\c 1\n\\p \\v 1 a");
}

/// A code that is not a book code at all: `book@code` in `usx.rnc` is the
/// list of books *or* `[A-Z][A-Z0-9]{2}|[0-9][A-Z][0-9]|[0-9]{2}[A-Z]`, and a
/// seven-letter word matches neither. The `\id` line is dropped, which is why
/// `missing-id` follows.
#[test]
fn unknown_book_code() {
    check(
        Code::UnknownBookCode,
        "\\id GENESIS Some book\n\\c 1\n\\p \\v 1 a",
    );
}

/// `unlisted-book-code` — a well-formed code that is not one of the books
/// `BookCode` names — is not here: nothing is repaired, so the parser has
/// nothing to say about it, and the test lives in
/// `usfm_semantic/tests/checks.rs` (ticket 19).
#[test]
fn content_outside_paragraph_text() {
    check(
        Code::ContentOutsideParagraph,
        "text before any marker\n\\p a",
    );
}

#[test]
fn content_outside_paragraph_after_chapter() {
    check_variant(
        Code::ContentOutsideParagraph,
        "after_chapter",
        "\\id GEN\n\\c 1\n\\v 1 a\n\\p \\v 2 b",
    );
}

#[test]
fn content_outside_paragraph_after_id() {
    check_variant(
        Code::ContentOutsideParagraph,
        "after_id",
        "\\id GEN\n\\v 1 a",
    );
}

#[test]
fn expected_table_cell() {
    check(
        Code::ExpectedTableCell,
        "\\id GEN\n\\c 1\n\\tr a \\tc2 b\n\\tr \\em c\\em*",
    );
}

#[test]
fn attribute_value_not_quoted() {
    check(
        Code::AttributeValueNotQuoted,
        "\\id GEN\n\\c 1\n\\p \\v 1 \\w word|lemma=grace\\w*",
    );
}

#[test]
fn missing_attribute_value() {
    check(
        Code::MissingAttributeValue,
        "\\id GEN\n\\c 1\n\\p \\v 1 \\w word|lemma= strong=\"H1234\"\\w*",
    );
}

#[test]
fn number_has_leading_zero() {
    check(
        Code::NumberHasLeadingZero,
        "\\id GEN\n\\c 091\n\\p \\v 01 text",
    );
}

#[test]
fn unexpected_pipe() {
    check(Code::UnexpectedPipe, "\\id GEN\n\\c 1\n\\p \\v 1 a | b");
}

/// `\"` inside a quoted value is an escaped quote, not the end of the value.
#[test]
fn escaped_quote_in_attribute_value() {
    let source =
        "\\id GEN\n\\c 1\n\\p \\v 1 \\fig cap|alt=\"He said: \\\"hi\\\"\" src=\"a.jpg\"\\fig* b";
    assert_eq!(common::codes(source), vec![]);
    assert!(common::render(source).contains("alt=\"He said: \\\"hi\\\"\""));
}

#[test]
fn unterminated_attribute_value() {
    check(
        Code::UnterminatedAttributeValue,
        "\\id GEN\n\\c 1\n\\p \\v 1 \\w a|lemma=\"x\\w* b",
    );
}

#[test]
fn unknown_custom_marker() {
    check(
        Code::UnknownCustomMarker,
        "\\id GEN\n\\c 1\n\\p \\v 1 a \\zfoo c\\zfoo* d",
    );
}

#[test]
fn figure_not_closed() {
    check(
        Code::FigureNotClosed,
        "\\id GEN\n\\c 1\n\\p \\v 1 a \\fig caption|src=\"a.jpg\" size=\"col\"\n\\p \\v 2 b",
    );
}

#[test]
fn sidebar_not_closed() {
    check(
        Code::SidebarNotClosed,
        "\\id GEN\n\\c 1\n\\esb\n\\p a\n\\esb\n\\p b",
    );
}

#[test]
fn unmatched_sidebar_end() {
    check(
        Code::UnmatchedSidebarEnd,
        "\\id GEN\n\\c 1\n\\p a\n\\esbe\n\\p b",
    );
}

/// A chapter cannot be inside a sidebar, so `\c` closes an open one.
#[test]
fn sidebar_not_closed_before_chapter() {
    check_variant(
        Code::SidebarNotClosed,
        "before_chapter",
        "\\id GEN\n\\c 1\n\\esb\n\\p a\n\\c 2\n\\p b",
    );
}

/// `\esb` takes a `\cat` and nothing else; other content on that line is
/// kept in an implicit `\p`.
#[test]
fn content_after_sidebar_marker() {
    check_variant(
        Code::ContentOutsideParagraph,
        "after_esb",
        "\\id GEN\n\\c 1\n\\esb \\cat People\\cat* stray\n\\p a\n\\esbe",
    );
}

/// A well-formed sidebar: the `\cat` becomes the category, the blocks up
/// to `\esbe` are the sidebar's, and the verse open before it is still open
/// after it (the paragraph after `\esbe` continues verse 1, whose end lands
/// there), with no end inside the sidebar.
#[test]
fn sidebar_with_category_inside_a_verse() {
    let source = "\\id GEN\n\\c 1\n\\p \\v 1 before\n\\esb \\cat People\\cat*\n\\ms Title\n\\p inside\n\\esbe\n\\p after\n\\p \\v 2 next";
    assert_eq!(common::codes(source), Vec::<Code>::new());
    snapshot("sidebar_with_category_inside_a_verse", source);
}

/// `\periph Title|id="x"` opens a division that runs to the next `\periph`;
/// the title is the text before `|`, and `id` is the default attribute.
#[test]
fn periph_divisions() {
    let source = "\\id FRT\n\\periph Title Page|id=\"title\"\n\\p one\n\\periph Preface|preface\n\\p two";
    assert_eq!(common::codes(source), Vec::<Code>::new());
    snapshot("periph_divisions", source);
}

/// A verse starting inside a paragraph that is not verse text (`\lit`) ends
/// the previous verse before that paragraph, in the last verse-text one.
#[test]
fn verse_ends_before_a_non_verse_text_paragraph() {
    let source = "\\id GEN\n\\c 1\n\\p \\v 9 nine\n\\lit Glory:\n\\v 15 fifteen\n\\p \\v 16 sixteen";
    assert_eq!(common::codes(source), Vec::<Code>::new());
    snapshot("verse_ends_before_a_non_verse_text_paragraph", source);
}

/// A `\cat` directly after the caller is the note's category, not content.
#[test]
fn note_category() {
    let source = "\\id GEN\n\\c 1\n\\p \\v 1 a\\f + \\cat ex st\\cat* \\fr 1.1 \\ft note\\f* b";
    assert_eq!(common::codes(source), Vec::<Code>::new());
    snapshot("note_category", source);
}

/// A verse still open at `\c` ends in the last verse-text paragraph of its
/// own chapter, not in the first one of the next (which may itself be verse
/// text, like `\d`).
#[test]
fn verse_ends_before_the_next_chapter() {
    let source = "\\id GEN\n\\c 1\n\\p \\v 5 last\n\\q1 line\n\\c 2\n\\s1 Heading\n\\d A Psalm\n\\p \\v 1 first";
    assert_eq!(common::codes(source), Vec::<Code>::new());
    snapshot("verse_ends_before_the_next_chapter", source);
}

#[test]
fn newline_in_attributes() {
    check(
        Code::NewlineInAttributes,
        "\\id GEN\n\\c 1\n\\p \\v 1 \\w a|lemma=\"x\"\nstrong=\"H1\"\\w* b",
    );
}

/// The markers Paratext's `usfm.sty` predates and the one entry it gets wrong,
/// supplied by `usfm_parser/usfm-extra.sty`: `\ipc`, `\ta` and `\wl` are in
/// `usx.rnc`'s paragraph and character enums, and `\xta` occurs under `\ex` as
/// well as `\x` (`CrossReferenceChar` under `CrossReference.style.enum`, which
/// usfm.sty already reflects on `\xo` and `\xt`). All of it parses silently.
///
/// One consequence the supplement does not undo: no character style's
/// `\OccursUnder` in usfm.sty mentions `\ipc`, since the sheet predates it, so
/// `\bd` inside an `\ipc` is `marker-not-listed-here` (Info, and those lists
/// are advisory anyway).
#[test]
fn markers_supplied_by_the_stylesheet_supplement() {
    let source = "\\id GEN\n\\ipc (50.24)\n\\c 1\n\\p \\v 1 \\ta color|a-uk=\"colour\"\\ta* and \\wl tăiat|lang=\"ro\"\\wl*\\ex + \\xo 1:1 \\xo* \\xt Matt 1:1\\xt* \\xta and\\xta*\\ex*";
    assert_eq!(common::codes(source), Vec::<Code>::new());
    snapshot("markers_supplied_by_the_stylesheet_supplement", source);
}

// The twelve inputs in `tasks/conformance/fixtures/usfm-grammar/autofix/`, one test each,
// named after the file. Upstream repairs each of them before parsing; here
// each one is a recovery shape, and the test says which `Code` we report, or
// snapshots the tree for the shapes that are ordinary USFM and report nothing.
// `wrong_book_code.usfm` is `\id` on a line of its own, which is exactly the
// input of `missing_book_code` above, so it has no test of its own.

/// `c_without_p.usfm`: a book with no `\p` at all. Every verse opens an
/// implicit paragraph, including the one after the `\c 3 \ca 4 \ca*` /
/// `\cp Three` pair, which is the shape the other two variants miss.
#[test]
fn content_outside_paragraph_c_without_p() {
    check_variant(
        Code::ContentOutsideParagraph,
        "c_without_p",
        "\\id GEN genesis Some desc\n\\c 1\n\\v 1 test verse\n\\s5\nsome more text\n\\v 2 more verse\n\\c 2\n\\v 1 next chapter\t\n\\c 3 \\ca 4 \\ca*\n\\cp Three\n\\v 1 text",
    );
}

/// `space_in_chapter_number.usfm`: `\c 2 3` is chapter 2 followed by a stray
/// `3`, which opens an implicit paragraph. `\v 3 4 text of 34` is verse 3 with
/// the rest as its text, since a verse number stops at the first space.
#[test]
fn content_outside_paragraph_space_in_chapter_number() {
    check_variant(
        Code::ContentOutsideParagraph,
        "space_in_chapter_number",
        "\\id GEN genesis Some desc\n\\c 1\n\\p\n\\v 1 test verse\n\\s3 test\n\\p\n\\v 2 more verse \n\\c 2 3\n\\p\n\\v 1 text\n\\v 2 more text\n\\v 3 4 text of 34\n\\p\n\\v 35 rest",
    );
}

/// `no_space_before_chapternumber.usfm`: `\v1` and `\c2` are markers in their
/// own right, and unknown ones, rather than `\v`/`\c` with a number.
#[test]
fn unknown_marker_no_space_before_chapternumber() {
    check_variant(
        Code::UnknownMarker,
        "no_space_before_chapternumber",
        "\\id GEN genesis Some desc\n\\c 1\n\\p\n\\v1 test verse\n\\s3 test\n\\v 2 more verse\n\\c2\n\\p\n\\v 1 next chapter\n\\v2 error text",
    );
}

/// `no_space_before_versenumber.usfm`: the same file with a well-formed
/// `\c 2`, so only the two `\v1`/`\v2` markers are unknown.
#[test]
fn unknown_marker_no_space_before_versenumber() {
    check_variant(
        Code::UnknownMarker,
        "no_space_before_versenumber",
        "\\id GEN genesis Some desc\n\\c 1\n\\p\n\\v1 test verse\n\\s3 test\n\\v 2 more verse\n\\c 2\n\\p\n\\v 1 next chapter\n\\v2 error text",
    );
}

/// `slash_in_text.usfm`: a lone `\` in running text, and `\slash` written
/// without a space after the backslash, which is an unknown marker.
#[test]
fn stray_backslash_slash_in_text() {
    check_variant(
        Code::StrayBackslash,
        "slash_in_text",
        "\\id GEN genesis Some desc\n\\c 1\n\\p\n\\v 1 test verse\n\\s3 test\n\\p\n\\v 2 more verse and \\ a slash\n\\c 2\n\\p\n\\v 1 text and \\slash without space\n\\v 2 more text",
    );
}

/// `wrong_book_code2.usfm`: `\id genesis Some desc`, a book name where the
/// three-letter code belongs.
#[test]
fn unknown_book_code_wrong_book_code2() {
    check_variant(
        Code::UnknownBookCode,
        "wrong_book_code2",
        "\\id genesis Some desc\n\\c 1\n\\p\n\\v 1 test verse",
    );
}

/// `b_without_p.usfm`: text on the line after `\b`, with no `\p` to open a
/// paragraph for it. Upstream inserts one; we keep the text in the `\b`
/// paragraph and report nothing, because `b` is a member of
/// `VersePara.para.style.enum` in tcdocs' `usx.rnc` and `VersePara` admits
/// text, so `<para style="b">text</para>` is a shape USX allows.
#[test]
fn autofix_b_without_p() {
    let source = "\\id GEN genesis Some desc\n\\c 1\n\\p\n\\v 1 test verse\n\\s some poem\n\\q1\nfirst line\n\\b\n\\q1\n\\v 2 more lines\n\\b\nremaining verses\n\\v 3 text follows\n\\c 2\n\\p\n\\v 1 test verse\n\\s some poem\n\\q1\nfirst line\n\\b\n\\q1\n\\v 2 more lines\n\\b\n\\v 3 verse follows";
    assert_eq!(common::codes(source), Vec::<Code>::new());
    snapshot("autofix_b_without_p", source);
}

/// `s5_without_p.usfm`: text on the line after `\s5`. A marker's content runs
/// to the next marker, so the text is the heading's, which is ordinary USFM
/// however the author meant it.
#[test]
fn autofix_s5_without_p() {
    let source = "\\id GEN genesis Some desc\n\\c 1\n\\p\n\\v 1 test verse\n\\s5\nsome more text\n\\v 2 more verse\n\\c 2\n\\p\n\\v 1 next chapter\n";
    assert_eq!(common::codes(source), Vec::<Code>::new());
    snapshot("autofix_s5_without_p", source);
}

/// `s5_1.usfm`: `\s5` with nothing on its line and a `\p` after it. An empty
/// paragraph is ordinary USFM.
#[test]
fn autofix_s5_1() {
    let source =
        "\\id GEN genesis Some desc\n\\c 1\n\\p\n\\v 1 test verse\n\\s5\n\\p\nsome more text";
    assert_eq!(common::codes(source), Vec::<Code>::new());
    snapshot("autofix_s5_1", source);
}

/// `empty_marker.usfm`: `\sp` with no content between two paragraphs. Also an
/// empty paragraph, and also nothing to report.
#[test]
fn autofix_empty_marker() {
    let source = "\\id GEN genesis Some desc\n\\c 1\n\\p\n\\v 1 test verse\n\\s3 test\n\\sp\n\\p\n\\v 2 more verse \n\\c 2\n\\p\n\\v 1 text and \n\\v 2 more text";
    assert_eq!(common::codes(source), Vec::<Code>::new());
    snapshot("autofix_empty_marker", source);
}

// The Paratext-shaped test project in `tasks/conformance/fixtures/machine-py/`, from
// sillsdev/machine.py. Unlike the tcdocs and usfm-grammar corpora it ships no
// expected output of any kind, so the expectation here is the parser's own
// tree and diagnostics: the books are snapshotted whole, and each shape in
// them that no existing test covers gets a `check_variant` of its own below.

const MACHINE_PY_41MAT: &str = include_str!("../../../tasks/conformance/fixtures/machine-py/Tes/41MATTes.SFM");
const MACHINE_PY_03LEV: &str = include_str!("../../../tasks/conformance/fixtures/machine-py/Tes/03LEVTes.SFM");
const MACHINE_PY_42MRK: &str = include_str!("../../../tasks/conformance/fixtures/machine-py/Tes/42MRKTes.SFM");
const MACHINE_PY_44JHN: &str = include_str!("../../../tasks/conformance/fixtures/machine-py/Tes/44JHNTes.SFM");
const MACHINE_PY_CUSTOM_STY: &str = include_str!("../../../tasks/conformance/fixtures/machine-py/Tes/custom.sty");

/// `Tes/41MATTes.SFM` whole, the one book in the set that exercises anything:
/// an `\fe` endnote, `\rq`, `\fm`, `\pn` with a nested `\+pro`, `\fig`, an
/// `\esb` sidebar, two tables, `\ts-s`/`\ts-e`, `\va`/`\vp`, `//`, verse
/// segments (`\v 3-4a`, `\v 4b`), and a handful of deliberate mistakes. Every
/// one of the standard markers parses without a diagnostic; the four below are
/// the mistakes the *parser* reports, listed here so a change to any of them
/// has to be accepted explicitly. The other four are `usfm_semantic`'s since
/// ticket 21 — three `verse-text-before-chapter` and a `verse-in-heading` —
/// and `usfm_semantic/tests/checks.rs` asserts the union of the eight.
///
/// Two of the file's oddities are deliberately not reported *here*: `\v 6`
/// occurs twice in chapter 2 and `\v 5` comes after it. Both are well-formed
/// verse markers, and whether a book's verses are unique and in order is a
/// question about the document, not about its syntax, so the parser keeps them
/// and says nothing. `usfm_semantic` reports them since ticket 23 — with a
/// third, `\v 2-3` and `\v 3-4a` both claiming verse 3 — and
/// `usfm_semantic/tests/checks.rs`'s `machine_py_41mat_order` is this test's
/// twin, pinning the three to their verses. This snapshot is the parser alone,
/// so it is unchanged by them.
/// The thin space (U+2009) ending the `\v 4` line survives into the text:
/// whitespace rule 1 normalises ASCII whitespace only.
#[test]
fn machine_py_41mat() {
    assert_eq!(
        common::codes(MACHINE_PY_41MAT),
        vec![
            // `\weirdtaglookingthing`.
            Code::UnknownMarker,
            // `\w*` with no `\w`.
            Code::UnmatchedClosingMarker,
            // `\v 3-4a` on the line after `\esbe`, and `\v 1` after `\c 4`.
            Code::ContentOutsideParagraph,
            Code::ContentOutsideParagraph,
        ],
    );
    snapshot("machine_py_41mat", MACHINE_PY_41MAT);
}

/// `\weirdtaglookingthing that is not an actual tag`: a marker that is a whole
/// word and has no closing marker anywhere, unlike the `\foo custom \foo*` of
/// `unknown_marker`. The marker is dropped and the text on the rest of the
/// line is kept, so the paragraph reads `and a that is not an actual tag.`
#[test]
fn unknown_marker_word_looking() {
    check_variant(
        Code::UnknownMarker,
        "word_looking",
        "\\id GEN\n\\c 1\n\\p \\v 1 and a \\weirdtaglookingthing that is not an actual tag.",
    );
}

/// `\w*` with no `\w`: the closer of a style that carries attributes, rather
/// than the `\em*`/`\x*` of `unmatched_closing_marker`. It is dropped without
/// taking the `,` after it with it, and no attribute parsing starts.
#[test]
fn unmatched_closing_marker_attributed_style() {
    check_variant(
        Code::UnmatchedClosingMarker,
        "attributed_style",
        "\\id GEN\n\\c 1\n\\p \\v 3 Chapter one \\w*,\n\\li2 verse three.",
    );
}

/// Content on the line after `\esbe`, the mirror of `content_after_sidebar_marker`.
/// `\esbe` ends the sidebar and takes no content, so the verse that follows it
/// goes into an implicit `\p` after the sidebar, and a character style in that
/// content is checked for placement against `\p` — not against `\esbe`, which
/// lists no children at all and would report `marker-not-listed-here` for
/// every style in the file.
#[test]
fn content_after_sidebar_end_marker() {
    check_variant(
        Code::ContentOutsideParagraph,
        "after_esbe",
        "\\id GEN\n\\c 1\n\\esb\n\\ms Title\n\\p inside\n\\esbe\n\\v 1 a \\w three|lemma\\w*.",
    );
}

/// `Tes/03LEVTes.SFM`: `\v 55b`, a verse segment continuing `\v 55` in the same
/// paragraph, and `\id Leviticus` — a book name where the code belongs, which
/// drops the `\id` line and so also leaves the document without one. The
/// `missing-id` that follows is `usfm_semantic`'s (ticket 21); the test there
/// asserts a caller still sees both.
#[test]
fn machine_py_03lev() {
    assert_eq!(common::codes(MACHINE_PY_03LEV), vec![Code::UnknownBookCode]);
    snapshot("machine_py_03lev", MACHINE_PY_03LEV);
}

/// `Tes/42MRKTes.SFM`: a scripture book that stops after its introduction,
/// with no `\c` and no `\v` at all. Nothing is reported: `\ip` is not verse
/// text, so `verse-text-before-chapter` does not apply, and a book file that
/// only has its front matter written yet is not malformed USFM.
#[test]
fn machine_py_42mrk() {
    assert_eq!(common::codes(MACHINE_PY_42MRK), Vec::<Code>::new());
    snapshot("machine_py_42mrk", MACHINE_PY_42MRK);
}

/// `Tes/44JHNTes.SFM` is zero bytes, which a Paratext project uses for a book
/// nobody has started. It parses to a document with no blocks and reports
/// nothing: the `missing-id` rule (`usfm_semantic`'s since ticket 21) asks what
/// the first block is, and there is no first block to be wrong about. The point
/// of the test is that the parser neither panics nor loops on empty input;
/// `an_empty_document_reports_nothing` in `checks.rs` is the other half.
#[test]
fn machine_py_44jhn_empty() {
    assert!(MACHINE_PY_44JHN.is_empty());
    assert_eq!(common::codes(MACHINE_PY_44JHN), Vec::<Code>::new());
    snapshot("machine_py_44jhn_empty", MACHINE_PY_44JHN);
}

/// `Tes/custom.sty`, the project stylesheet beside those books, defines
/// `\test` as a character style. There is no one-call API for extending a
/// sheet from a `.sty` string: a caller reads the file with
/// `StyleSheet::from_str` and adds its rules to a clone of the default sheet,
/// which is what a Paratext project's stylesheet override amounts to. With
/// that sheet `\test` is an ordinary character style instead of an unknown
/// marker, and 41MAT — which uses none of the project's own markers — parses
/// exactly as it does with the default sheet.
#[test]
fn machine_py_custom_stylesheet() {
    use std::str::FromStr;
    use std::sync::Arc;

    use usfm_parser::DEFAULT_STYLESHEET;
    use usfm_parser::parser::Parser;
    use usfm_style::StyleSheet;

    let custom = StyleSheet::from_str(MACHINE_PY_CUSTOM_STY).expect("custom.sty parses");
    assert_eq!(
        custom.rules.iter().map(|r| r.marker.as_str()).collect::<Vec<_>>(),
        vec!["test"],
    );
    let mut extended = (**DEFAULT_STYLESHEET).clone();
    for rule in custom.rules {
        extended.add_rule(rule);
    }
    let extended = Arc::new(extended);

    let source = "\\id GEN\n\\c 1\n\\p \\v 1 a \\test custom\\test* b";
    assert_eq!(
        common::codes(source),
        vec![Code::UnknownMarker, Code::UnknownMarker],
        "`\\test` and `\\test*` are unknown to the default stylesheet"
    );

    let result = Parser::new(source).parse(&extended);
    assert!(
        result.diagnostics.is_empty(),
        "`\\test` should resolve against the project stylesheet, got {:?}",
        result.diagnostics
    );
    let mut rendered = String::new();
    common::Printer::new(result.document.style_sheet()).document(&mut rendered, &result.document);
    assert!(
        rendered.contains("Char test"),
        "`\\test` should be a character style, got:\n{rendered}"
    );

    assert_eq!(
        Parser::new(MACHINE_PY_41MAT).parse(&extended).diagnostics,
        Parser::new(MACHINE_PY_41MAT)
            .parse(&DEFAULT_STYLESHEET)
            .diagnostics,
        "the project stylesheet adds a marker 41MAT does not use"
    );
}

/// The snapshot for `code`, if this suite has one.
fn recovery_snapshot(code: &Code) -> Option<std::path::PathBuf> {
    let path = std::path::PathBuf::from(format!(
        "{}/tests/snapshots/recovery__{}.snap",
        env!("CARGO_MANIFEST_DIR"),
        code.as_str().replace('-', "_")
    ));
    path.exists().then_some(path)
}

/// Every code in the recovery table has a snapshot produced by a test in
/// this file. Codes that cannot be triggered from the default stylesheet
/// are listed explicitly, and the codes the semantic pass owns
/// ([`Code::is_semantic`]) are covered by `usfm_semantic/tests/checks.rs`
/// instead — its `semantic_checks_are_covered` requires a
/// `checks__<code>.snap` for each of them.
#[test]
fn recovery_table_is_covered() {
    let exempt = [
        // Needs a stylesheet without `p`.
        Code::ContentDropped,
        // Not reachable from well-formed parser state by construction.
        Code::Internal,
    ];
    let missing: Vec<&str> = Code::ALL
        .iter()
        .filter(|code| !exempt.contains(code) && !code.is_semantic())
        .filter(|code| recovery_snapshot(code).is_none())
        .map(|code| code.as_str())
        .collect();
    assert!(
        missing.is_empty(),
        "codes without a recovery test: {missing:?}"
    );
}

/// The other half of the rule, which a moved check is easy to leave half done:
/// a code the semantic pass owns must have no snapshot *here*. A `recovery__`
/// file left behind after a move would still be read by `cargo insta` as an
/// unreferenced snapshot, and would say the parser reports something it does
/// not (ticket 21).
#[test]
fn semantic_codes_have_no_recovery_snapshot() {
    let stale: Vec<String> = Code::ALL
        .iter()
        .filter(|code| code.is_semantic())
        .filter_map(|code| {
            recovery_snapshot(code).map(|path| {
                format!(
                    "{}: {}",
                    code.as_str(),
                    path.file_name().unwrap().to_string_lossy()
                )
            })
        })
        .collect();
    assert!(
        stale.is_empty(),
        "semantic codes with a recovery snapshot left behind: {stale:?}"
    );
}
