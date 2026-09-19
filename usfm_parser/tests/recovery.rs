//! One test per recovery rule.
//!
//! Each test feeds the parser a minimal input that triggers one
//! [`Code`], asserts that code was emitted, and snapshots the exact
//! recovered tree plus diagnostics. The snapshot is the executable form of
//! the recovery table documented on `Code`.
//!
//! Snapshots live in `tests/snapshots/recovery__<code>.snap`.

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

#[test]
fn unknown_book_code() {
    check(
        Code::UnknownBookCode,
        "\\id ZZZ Some book\n\\c 1\n\\p \\v 1 a",
    );
}

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
fn empty_attribute_list() {
    check(
        Code::EmptyAttributeList,
        "\\id GEN\n\\c 1\n\\p \\v 1 \\w word| \\w*",
    );
}

#[test]
fn no_default_attribute() {
    check(
        Code::NoDefaultAttribute,
        "\\id GEN\n\\c 1\n\\p \\v 1 \\em caption|no default\\em*",
    );
}

#[test]
fn default_attribute_with_others() {
    check(
        Code::DefaultAttributeWithOthers,
        "\\id GEN\n\\c 1\n\\p \\v 1 \\w word|grace strong=\"H1234\"\\w*",
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

/// `\xq` may only occur under `\x`; in a paragraph it is reported and kept.
#[test]
fn marker_not_allowed_here() {
    check(
        Code::MarkerNotAllowedHere,
        "\\id GEN\n\\c 1\n\\p \\v 1 a \\xq quote\\xq* b",
    );
}

/// `\f` under `\cl` is not in the stylesheet's list, but Paratext accepts
/// it, so it only informs.
#[test]
fn marker_not_listed_here() {
    check(
        Code::MarkerNotListedHere,
        "\\id GEN\n\\c 1\n\\cl Chapter One\\f + \\ft note\\f*\n\\p \\v 1 a",
    );
}

/// A note's parent is its paragraph, whatever character style is open
/// around it, and note text markers occur under the note: all silent.
#[test]
fn placement_looks_through_character_styles_for_notes() {
    let source = "\\id GEN\n\\c 1\n\\p \\v 1 \\wj a \\x + \\xo 1.1 \\xt Gen 1\\x* b\\wj*";
    assert_eq!(common::codes(source), Vec::<Code>::new());
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
fn empty_word() {
    check(
        Code::EmptyWord,
        "\\id GEN\n\\c 1\n\\p \\v 1 a \\w |lemma=\"x\"\\w* b",
    );
}

/// `\jmp` and other attributed styles may be empty; only `\w` is checked.
#[test]
fn empty_jmp_is_valid() {
    let codes = common::codes("\\id GEN\n\\c 1\n\\p \\v 1 a \\jmp |link-href=\"#x\"\\jmp* b");
    assert_eq!(codes, vec![]);
}

#[test]
fn verse_in_character_style() {
    check(
        Code::VerseInCharacterStyle,
        "\\id GEN\n\\c 1\n\\p \\v 1 \\wj a \\v 2 b\\wj* c",
    );
}

#[test]
fn verse_in_heading() {
    check(
        Code::VerseInHeading,
        "\\id GEN\n\\c 1\n\\s1 \\v 1 heading\n\\p a",
    );
}

#[test]
fn verse_outside_chapter() {
    check(Code::VerseOutsideChapter, "\\id GEN\n\\p \\v 1 a");
}

#[test]
fn verse_text_before_chapter() {
    check(
        Code::VerseTextBeforeChapter,
        "\\id GEN\n\\ip intro\n\\p body\n\\c 1\n\\p \\v 1 a",
    );
}

#[test]
fn missing_id() {
    check(Code::MissingId, "\\c 1\n\\p \\v 1 a");
}

#[test]
fn id_not_first() {
    check(
        Code::IdNotFirst,
        "\\id GEN\n\\c 1\n\\p \\v 1 a\n\\id EXO\n\\c 1\n\\p \\v 1 b",
    );
}

#[test]
fn empty_book() {
    check(Code::EmptyBook, "\\id GEN Genesis\n");
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

/// `\th3` right after `\th1`: the cell keeps column 3 and the gap is reported.
#[test]
fn unexpected_table_column() {
    check(
        Code::UnexpectedTableColumn,
        "\\id GEN\n\\c 1\n\\tr \\th1 a \\th3 c\n\\tr \\tcr2 b \\tcr3 c",
    );
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

/// Every code in the recovery table has a snapshot produced by a test in
/// this file. Codes that cannot be triggered from the default stylesheet
/// are listed explicitly.
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
        .filter(|code| !exempt.contains(code))
        .map(|code| code.as_str())
        .filter(|name| {
            let path = format!(
                "{}/tests/snapshots/recovery__{}.snap",
                env!("CARGO_MANIFEST_DIR"),
                name.replace('-', "_")
            );
            !std::path::Path::new(&path).exists()
        })
        .collect();
    assert!(
        missing.is_empty(),
        "codes without a recovery test: {missing:?}"
    );
}
