//! One test per semantic check.
//!
//! The companion of `usfm_parser/tests/recovery.rs`: **every [`Code`] has a
//! test in one file or the other**, and [`Code::is_semantic`] says which. A
//! code this crate reports is tested here, with a snapshot in
//! `tests/snapshots/checks__<code>.snap`; a code the parser reports is tested
//! there, with a snapshot in `recovery__<code>.snap`. Each file's coverage
//! test ([`semantic_checks_are_covered`] here, `recovery_table_is_covered`
//! there) reads `is_semantic` so that moving a check between the passes is one
//! line in `usfm_diagnostics` plus a moved snapshot file.
//!
//! Each test feeds a minimal input through `usfm::parse` — the union of the
//! parser's diagnostics and `usfm_semantic::analyze`'s, which is what a
//! caller sees — asserts the code was emitted, and snapshots the tree plus
//! diagnostics with the renderer `recovery.rs` uses.

mod common;

use usfm::diagnostics::{Code, Severity};

/// Snapshot named `checks__<code>`.
fn check(code: Code, source: &str) {
    check_named(code, code.as_str().replace('-', "_"), source);
}

/// Snapshot named `checks__<code>__<variant>`, for additional cases of the
/// same code. The twin of `recovery.rs`'s function of the same name.
fn check_variant(code: Code, variant: &str, source: &str) {
    check_named(
        code,
        format!("{}__{variant}", code.as_str().replace('-', "_")),
        source,
    );
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
        insta::assert_snapshot!(format!("checks__{name}"), rendered);
    });
}

/// A code that *is* well formed but is not one of the books `BookCode` names.
/// USX accepts it, so nothing is dropped: the book is kept as
/// `BookCode::Other` and `\id ZZZ` reaches the output as written. Reported at
/// Warning because a typo in a real code has exactly this shape.
///
/// Moved here from `recovery.rs` by ticket 19: with nothing repaired there is
/// no recovery to describe, and the check reads the finished tree. The span
/// widened with the move — the parser reported the three bytes of the code,
/// this pass reports the `\id` line the AST's `Book` carries — which is what
/// the snapshot records.
#[test]
fn unlisted_book_code() {
    let source = "\\id ZZZ Some book\n\\c 1\n\\p \\v 1 a";
    assert_eq!(common::codes(source), vec![Code::UnlistedBookCode]);
    assert_eq!(
        common::parser_codes(source),
        Vec::<Code>::new(),
        "the parser repairs nothing here, so it says nothing"
    );
    assert!(
        common::render(source).contains("Book ZZZ"),
        "the book is kept: {}",
        common::render(source)
    );
    check(Code::UnlistedBookCode, source);
}

/// A listed code is silent, which is the other half of the rule above.
#[test]
fn a_listed_book_code_is_silent() {
    assert_eq!(
        common::codes("\\id GEN Genesis\n\\c 1\n\\p \\v 1 a"),
        Vec::<Code>::new()
    );
}

// ---------------------------------------------------------------------------
// Placement: `OccursUnder` from the stylesheet against the marker the style
// actually sits under. Moved here from `recovery.rs` by ticket 20 with their
// inputs unchanged; the spans in the snapshots widened from the opening
// marker to the whole node, which is the span the tree carries.
// ---------------------------------------------------------------------------

/// `\xq` may only occur under `\x`; in a paragraph it is reported and kept.
#[test]
fn marker_not_allowed_here() {
    let source = "\\id GEN\n\\c 1\n\\p \\v 1 a \\xq quote\\xq* b";
    assert_eq!(
        common::parser_codes(source),
        Vec::<Code>::new(),
        "the style is parsed where it is, so the parser says nothing"
    );
    check(Code::MarkerNotAllowedHere, source);
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

/// Content on the line after `\esbe` goes into an implicit `\p`, and that is
/// the parent a character style in it is checked against — not `\esbe`, which
/// lists no children at all and would report every style in the file. In the
/// tree the implicit paragraph is an ordinary `Para p`, so the rule needs no
/// special case here; the twin in `recovery.rs` snapshots the same input for
/// `content-outside-paragraph`.
#[test]
fn placement_after_a_sidebar_end_marker_is_against_the_implicit_paragraph() {
    let source = "\\id GEN\n\\c 1\n\\esb\n\\ms Title\n\\p inside\n\\esbe\n\\v 1 a \\w three|lemma\\w*.";
    assert_eq!(
        common::codes(source),
        vec![Code::ContentOutsideParagraph],
        "`\\w` occurs under `\\p`, so nothing is added to the parser's one diagnostic"
    );
}

/// Inside a table cell nothing is placement-checked: the cell markers are in
/// no `OccursUnder` list, so every style in every table would be reported.
#[test]
fn placement_is_not_checked_inside_a_table_cell() {
    let source = "\\id GEN\n\\c 1\n\\p \\v 1 a\n\\tr \\tc1 \\xq quoted\\xq*";
    let codes = common::codes(source);
    assert!(
        !codes.contains(&Code::MarkerNotAllowedHere) && !codes.contains(&Code::MarkerNotListedHere),
        "{codes:?}"
    );
}

// ---------------------------------------------------------------------------
// Attributes. The list is in the tree exactly as written whatever is reported,
// which is what puts all six codes here; `newline-in-attributes`,
// `attribute-value-not-quoted`, `missing-attribute-value` and
// `unterminated-attribute-value` stay with the parser, which decides what the
// list *is*.
// ---------------------------------------------------------------------------

#[test]
fn empty_attribute_list() {
    check(
        Code::EmptyAttributeList,
        "\\id GEN\n\\c 1\n\\p \\v 1 \\w word| \\w*",
    );
}

/// A second `|` in one character style replaces the first list rather than
/// extending it, so the tree holds one list and the check reports once —
/// where the parser, which ran the check at every `|`, reported once per pipe.
/// `tcdocs/tests/paratextTests/EmptyFigure` (`\fig |||||| \fig*`) is the only
/// input in the suites that has more than one, and it is unchanged by the
/// difference: what the pipes dropped was already nothing.
#[test]
fn empty_attribute_list_is_reported_once_per_list_not_per_pipe() {
    assert_eq!(
        common::codes("\\id GEN\n\\c 1\n\\p \\v 1 \\fig |||| \\fig*"),
        vec![Code::EmptyAttributeList],
    );
}

/// The same shape on a milestone. All 24 `\ts-s` markers in
/// `tasks/conformance/fixtures/usfm-grammar/autofix/fr-textTranslation-FR_TLX.txt`
/// are written `\ts-s |\*`, which is how unfoldingWord's aligned texts write
/// a translation section. A milestone carries nothing but its attributes, so
/// the empty list loses nothing and the file must not fail `--strict`.
#[test]
fn empty_milestone_attribute_list() {
    let source = "\\id GEN\n\\c 1\n\\p \\v 1 a \\ts-s |\\* b";
    check(Code::EmptyMilestoneAttributeList, source);
    let errors: Vec<&str> = common::codes(source)
        .iter()
        .filter(|code| code.severity() == Severity::Error)
        .map(|code| code.as_str())
        .collect();
    assert!(
        errors.is_empty(),
        "`\\ts-s |\\*` should report nothing at error severity, got {errors:?}"
    );
}

/// The same on a milestone the stylesheet does not define, which ticket 18
/// found reporting nothing: the parser returned an empty list both for
/// `\zaln-s\*` and for `\zaln-s |\*` and could not tell them apart.
/// `Milestone::attributes` is an `Option` for that reason (ticket 20), so the
/// two shapes are distinct in the tree and the rule is the one rule.
#[test]
fn empty_milestone_attribute_list_unknown_milestone() {
    check_variant(
        Code::EmptyMilestoneAttributeList,
        "unknown_milestone",
        "\\id GEN\n\\c 1\n\\p \\v 1 a \\zaln-s |\\* b",
    );
    assert_eq!(
        common::codes("\\id GEN\n\\c 1\n\\p \\v 1 a \\zaln-s\\* b"),
        vec![Code::UnknownCustomMilestone],
        "without the `|` there is no empty list to report"
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

/// An attribute name that is not an identifier. The fuzzer found this: the
/// name went straight into the USX output, which made it invalid XML. The
/// tree keeps the attribute as written; `usx.rs` drops it.
#[test]
fn malformed_attribute_name() {
    check(
        Code::MalformedAttributeName,
        "\\id GEN\n\\c 1\n\\p \\v 1 \\w word|b<c=\"1\"\\w*",
    );
}

/// The same attribute name twice. The fuzzer found the two-defaults form
/// (`\rb b|"h=c"`, where the quotes make two bare values): both became
/// `gloss` and the USX had the attribute twice, which is not XML.
#[test]
fn duplicate_attribute() {
    check(
        Code::DuplicateAttribute,
        "\\id GEN\n\\c 1\n\\p \\v 1 \\w word|lemma=\"a\" lemma=\"b\"\\w*",
    );
    check_variant(
        Code::DuplicateAttribute,
        "default_twice",
        "\\id GEN\n\\c 1\n\\p \\v 1 \\rb b|\"h=c\"\\rb*",
    );
}

// ---------------------------------------------------------------------------
// Structure, numbers and tables. Moved here from `recovery.rs` by ticket 21
// with their inputs unchanged: none of these repairs anything, and the tree
// holds what each rule reads — the block list, a `VerseStart` and what it sits
// in, a cell's column, a `\w` with no children. The spans in the snapshots
// widened from the marker to the node that starts at it, except `missing-id`
// (offset 0) and `empty-book` (end of source), which are where they were.
// ---------------------------------------------------------------------------

/// `\w` with attributes and no word. `\jmp` below is the other half of the
/// rule: only `\w` is checked.
#[test]
fn empty_word() {
    let source = "\\id GEN\n\\c 1\n\\p \\v 1 a \\w |lemma=\"x\"\\w* b";
    assert_eq!(
        common::parser_codes(source),
        Vec::<Code>::new(),
        "the empty node is kept, so the parser says nothing"
    );
    check(Code::EmptyWord, source);
}

/// `\jmp` and other attributed styles may be empty; only `\w` is checked.
#[test]
fn empty_jmp_is_valid() {
    let codes = common::codes("\\id GEN\n\\c 1\n\\p \\v 1 a \\jmp |link-href=\"#x\"\\jmp* b");
    assert_eq!(codes, vec![]);
}

/// A verse inside `\wj`. The verse is kept where it is, so the `VerseStart` is
/// in the tree inside a `Char` and the check reads it off that.
#[test]
fn verse_in_character_style() {
    let source = "\\id GEN\n\\c 1\n\\p \\v 1 \\wj a \\v 2 b\\wj* c";
    assert_eq!(
        common::parser_codes(source),
        Vec::<Code>::new(),
        "the verse is kept, so the parser says nothing"
    );
    check(Code::VerseInCharacterStyle, source);
}

/// A verse in a section heading. The paragraph's own style is what decides it,
/// which is why the check asks the nearest enclosing `Para` rather than the
/// last paragraph marker the parser had passed.
#[test]
fn verse_in_heading() {
    check(
        Code::VerseInHeading,
        "\\id GEN\n\\c 1\n\\s1 \\v 1 heading\n\\p a",
    );
}

/// `s3_without_p.usfm`: `\v 2` on the line after `\s3 test`, with no `\p`
/// between them, so the verse starts inside the heading.
#[test]
fn verse_in_heading_s3_without_p() {
    check_variant(
        Code::VerseInHeading,
        "s3_without_p",
        "\\id GEN genesis Some desc\n\\c 1\n\\p\n\\v 1 test verse\n\\s3 test\n\\v 2 more verse\n\\c 2\n\\p\n\\v 1 next chapter\n",
    );
}

/// `\s5`, unfoldingWord's chunk marker, is an empty heading written directly
/// before a verse: exempt, as it was in the parser.
#[test]
fn a_verse_after_the_s5_chunk_marker_is_silent() {
    assert_eq!(
        common::codes("\\id GEN\n\\c 1\n\\s5\n\\p \\v 1 a"),
        Vec::<Code>::new()
    );
}

/// A verse in a table cell is not in a heading, whatever paragraph came before
/// the table. The parser read the text type of the last paragraph marker it
/// had passed and reported this one; the tree says the verse is in a cell.
#[test]
fn a_verse_in_a_table_cell_after_a_heading_is_not_in_a_heading() {
    assert_eq!(
        common::codes("\\id GEN\n\\c 1\n\\s1 Head\n\\tr \\tc1 \\v 1 a"),
        Vec::<Code>::new()
    );
}

#[test]
fn verse_outside_chapter() {
    check(Code::VerseOutsideChapter, "\\id GEN\n\\p \\v 1 a");
}

/// A verse-text paragraph before the first `\c`, reported once per paragraph.
/// `\ip` is introduction, not verse text, so only the `\p` is reported.
#[test]
fn verse_text_before_chapter() {
    check(
        Code::VerseTextBeforeChapter,
        "\\id GEN\n\\ip intro\n\\p body\n\\c 1\n\\p \\v 1 a",
    );
}

/// The rule is about scripture: a peripheral book has no chapters to come
/// before, and a document with no `\id` at all has no book to judge.
#[test]
fn verse_text_before_chapter_needs_a_scripture_book() {
    assert_eq!(
        common::codes("\\id FRT\n\\p body\n"),
        Vec::<Code>::new(),
        "`\\id FRT` is not scripture"
    );
    assert_eq!(
        common::codes("\\p body\n"),
        vec![Code::MissingId],
        "with no `\\id` there is no book to be before a chapter"
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

/// An empty document reports neither: there is no first block to be wrong
/// about. `Tes/44JHNTes.SFM`, a Paratext project's placeholder for a book
/// nobody has started, is zero bytes.
#[test]
fn an_empty_document_reports_nothing() {
    assert_eq!(common::codes(""), Vec::<Code>::new());
}

/// `\th3` right after `\th1`: the cell keeps column 3 and the gap is reported,
/// from `TableCell::column` in row order.
#[test]
fn unexpected_table_column() {
    let source = "\\id GEN\n\\c 1\n\\tr \\th1 a \\th3 c\n\\tr \\tcr2 b \\tcr3 c";
    assert_eq!(
        common::parser_codes(source),
        Vec::<Code>::new(),
        "the cell keeps the column it names, so the parser says nothing"
    );
    check(Code::UnexpectedTableColumn, source);
}

/// A cell spanning two columns (`\tcr1-2`) moves the expectation on by its
/// colspan, so the cell after it is in column 3 and nothing is reported.
#[test]
fn a_column_span_is_counted() {
    assert_eq!(
        common::codes("\\id GEN\n\\c 1\n\\tr \\tcr1-2 a \\tcr3 b"),
        Vec::<Code>::new()
    );
}

/// The Paratext-shaped book from sillsdev/machine.py that ticket 09 vendored,
/// read by the two tests below. `recovery.rs` snapshots the same file for the
/// parser's half.
const MACHINE_PY_41MAT: &str =
    include_str!("../../../tasks/conformance/fixtures/machine-py/Tes/41MATTes.SFM");

// ---------------------------------------------------------------------------
// Verse and chapter order (ticket 23). The only checks here that are not about
// a node on its own: a verse is judged against the verses of its chapter and a
// chapter against the chapters of its book, which the walk passes in order and
// the `Analyzer` remembers. All four are Warnings — the document is well-formed
// USFM and nothing is repaired — so no tcdocs case can turn on them.
// Versification is out of scope: a *missing* verse is not reported.
// ---------------------------------------------------------------------------

/// The same number twice in one chapter, on the second one's `\v`.
#[test]
fn duplicate_verse_number() {
    let source = "\\id GEN\n\\c 1\n\\p \\v 1 a \\v 2 b \\v 2 c";
    assert_eq!(
        common::parser_codes(source),
        Vec::<Code>::new(),
        "both verses are well-formed, so the parser says nothing"
    );
    check(Code::DuplicateVerseNumber, source);
}

/// Segments are distinct verses: `\v 4a` and `\v 4b` are the two halves of
/// verse 4 and neither covers the other.
#[test]
fn verse_segments_are_distinct_verses() {
    assert_eq!(
        common::codes("\\id GEN\n\\c 1\n\\p \\v 4a a \\v 4b b \\v 5 c"),
        Vec::<Code>::new()
    );
}

/// The same segment twice, though, is the same verse twice.
#[test]
fn duplicate_verse_number_same_segment() {
    check_variant(
        Code::DuplicateVerseNumber,
        "same_segment",
        "\\id GEN\n\\c 1\n\\p \\v 4a a \\v 4a b",
    );
}

/// A bare number is the whole verse, segments and all, so `\v 4` then
/// `\v 4a` is a duplicate — and so is `\v 4a` then `\v 4`, which is the shape
/// `Tes/03LEVTes.SFM` has.
#[test]
fn duplicate_verse_number_whole_then_segment() {
    check_variant(
        Code::DuplicateVerseNumber,
        "whole_then_segment",
        "\\id GEN\n\\c 1\n\\p \\v 4 a \\v 4a b",
    );
    assert_eq!(
        common::codes("\\id GEN\n\\c 1\n\\p \\v 4a a \\v 4 b"),
        vec![Code::DuplicateVerseNumber],
        "the bare number covers the segment either way round"
    );
}

/// A range covers every number in it, so `\v 3-5` and a later `\v 4` are the
/// same verse written twice. Reported as a duplicate and not as
/// `verse-out-of-order`: a verse reports at most one of the two.
#[test]
fn duplicate_verse_number_inside_a_range() {
    let source = "\\id GEN\n\\c 1\n\\p \\v 3-5 a \\v 4 b";
    assert_eq!(common::codes(source), vec![Code::DuplicateVerseNumber]);
    check_variant(Code::DuplicateVerseNumber, "inside_a_range", source);
}

/// The other side of the range rule: the number after the range is new, and
/// a segment written on a range's end (`\v 3-4a`, then `\v 4b`) leaves the
/// other segment free — the shape `41MATTes.SFM` uses.
#[test]
fn a_verse_after_a_range_is_silent() {
    assert_eq!(
        common::codes("\\id GEN\n\\c 1\n\\p \\v 3-5 a \\v 6 b"),
        Vec::<Code>::new()
    );
    assert_eq!(
        common::codes("\\id GEN\n\\c 1\n\\p \\v 3-4a a \\v 4b b \\v 5 c"),
        Vec::<Code>::new()
    );
}

/// A range of four billion verses is a number like any other: coverage keeps
/// a range as a range, so nothing here iterates it. The fuzzer reaches inputs
/// like this, and a check that counted from 1 to `\v 4000000000` would hang
/// rather than crash, which is the harder failure to notice.
#[test]
fn an_absurdly_wide_range_is_not_iterated() {
    let source = "\\id GEN\n\\c 1\n\\p \\v 1-4000000000 a \\v 2000000000 b";
    assert_eq!(common::codes(source), vec![Code::DuplicateVerseNumber]);
}

/// A number lower than the previous verse's end.
#[test]
fn verse_out_of_order() {
    let source = "\\id GEN\n\\c 1\n\\p \\v 6 a \\v 7a b \\v 5 c";
    assert_eq!(
        common::parser_codes(source),
        Vec::<Code>::new(),
        "every verse here is well-formed, so the parser says nothing"
    );
    check(Code::VerseOutOfOrder, source);
}

/// A gap in a list is a number that is still free, so a verse that fills it
/// is out of order without being a duplicate: `\v 1,3-5` then `\v 2`.
#[test]
fn verse_out_of_order_into_a_gap() {
    let source = "\\id GEN\n\\c 1\n\\p \\v 1,3-5 a \\v 2 b";
    assert_eq!(common::codes(source), vec![Code::VerseOutOfOrder]);
    check_variant(Code::VerseOutOfOrder, "into_a_gap", source);
}

/// Segments compare by letter, so `4b` after `4a` is forward and `4a` after
/// `4b` is backwards.
#[test]
fn verse_out_of_order_by_segment() {
    check_variant(
        Code::VerseOutOfOrder,
        "by_segment",
        "\\id GEN\n\\c 1\n\\p \\v 4b a \\v 4a b",
    );
}

/// A verse before the first `\c` is in no chapter, so the order checks never
/// see it: `verse-outside-chapter` has already said what there is to say.
#[test]
fn a_verse_before_the_first_chapter_is_not_ordered() {
    assert_eq!(
        common::codes("\\id GEN\n\\p \\v 5 a\n\\c 1\n\\p \\v 1 b \\v 2 c"),
        vec![Code::VerseTextBeforeChapter, Code::VerseOutsideChapter],
        "no duplicate and no ordering report for the chapterless `\\v 5`"
    );
}

/// Each chapter counts its verses on its own, so the same number in two
/// chapters is not a duplicate.
#[test]
fn verse_numbers_start_again_in_each_chapter() {
    assert_eq!(
        common::codes("\\id GEN\n\\c 1\n\\p \\v 1 a\n\\c 2\n\\p \\v 1 b"),
        Vec::<Code>::new()
    );
}

/// The same chapter number twice, on the second `\c`.
#[test]
fn duplicate_chapter_number() {
    let source = "\\id GEN\n\\c 1\n\\p \\v 1 a\n\\c 1\n\\p \\v 1 b";
    assert_eq!(
        common::parser_codes(source),
        Vec::<Code>::new(),
        "both chapters are well-formed, so the parser says nothing"
    );
    check(Code::DuplicateChapterNumber, source);
}

/// A chapter numbered lower than the one before it.
#[test]
fn chapter_out_of_order() {
    check(
        Code::ChapterOutOfOrder,
        "\\id GEN\n\\c 2\n\\p \\v 1 a\n\\c 1\n\\p \\v 1 b",
    );
}

/// Chapters are counted per book, not per document. A document with two `\id`
/// lines is two books — the CLI makes one whenever it is given several files —
/// and the second book's chapters start again at 1, so neither its `\c 1` nor
/// its `\v 1` is a repeat of the first book's. `id-not-first` is still
/// reported, and is the only thing reported.
#[test]
fn chapters_and_verses_start_again_in_each_book() {
    let source = "\\id GEN\n\\c 1\n\\p \\v 1 a\n\\id EXO\n\\c 1\n\\p \\v 1 b";
    assert_eq!(
        common::codes(source),
        vec![Code::IdNotFirst],
        "the second book repeats nothing of the first"
    );
}

/// A gap is not a mistake here — versification is out of scope, so `\c 1`
/// followed by `\c 3` is a book with two chapters and nothing to report.
#[test]
fn a_chapter_gap_is_silent() {
    assert_eq!(
        common::codes("\\id GEN\n\\c 1\n\\p \\v 1 a\n\\c 3\n\\p \\v 1 b"),
        Vec::<Code>::new()
    );
}

/// The machine.py fixture ticket 09 vendored, whose eight mistakes
/// `recovery.rs` lists. Four of them are this crate's, and the union a caller
/// sees still has all eight — plus the three the order checks add, which
/// `machine_py_41mat_order` below pins to their verses.
#[test]
fn machine_py_41mat() {
    assert_eq!(
        common::codes(MACHINE_PY_41MAT),
        vec![
            // Three `\p` in the introduction, before `\c 1`.
            Code::VerseTextBeforeChapter,
            Code::VerseTextBeforeChapter,
            Code::VerseTextBeforeChapter,
            // `\weirdtaglookingthing`.
            Code::UnknownMarker,
            // `\v 1` on the line after `\s Chapter One`, with no `\p`.
            Code::VerseInHeading,
            // `\w*` with no `\w`.
            Code::UnmatchedClosingMarker,
            // `\v 3-4a` on the line after `\esbe`, and `\v 1` after `\c 4`.
            Code::ContentOutsideParagraph,
            // `\v 3-4a` again: `\v 2-3` before it already covered verse 3.
            Code::DuplicateVerseNumber,
            // `\v 6` twice in chapter 2, and `\v 5` after the second of them.
            Code::DuplicateVerseNumber,
            Code::VerseOutOfOrder,
            Code::ContentOutsideParagraph,
        ],
    );
}

/// The twin of `recovery.rs`'s `machine_py_41mat`, which says the file's two
/// verse oddities are the parser's business to keep and this crate's to
/// report: `\v 6` appears twice in chapter 2, and `\v 5` comes after it.
/// Both are reported here, on the later `\v` of each pair — and with them a
/// third the ticket did not name, because `\v 2-3` and `\v 3-4a` both claim
/// verse 3.
#[test]
fn machine_py_41mat_order() {
    let reported: Vec<(Code, u32)> = usfm::parse(MACHINE_PY_41MAT)
        .diagnostics
        .iter()
        .filter(|diagnostic| {
            matches!(
                diagnostic.code,
                Code::DuplicateVerseNumber | Code::VerseOutOfOrder
            )
        })
        .map(|diagnostic| (diagnostic.code, diagnostic.span.start))
        .collect();
    let offset = |marker: &str| MACHINE_PY_41MAT.find(marker).expect("in the fixture") as u32;
    assert_eq!(
        reported,
        vec![
            (Code::DuplicateVerseNumber, offset("\\v 3-4a")),
            (Code::DuplicateVerseNumber, offset("\\v 6 Bad verse.")),
            (Code::VerseOutOfOrder, offset("\\v 5 Chapter two")),
        ],
    );
}

/// `Tes/03LEVTes.SFM`: `\id Leviticus` is a book name where the code belongs,
/// so the parser drops the `\id` line and the document is left without one.
/// The two halves report one code each, and the order a caller sees is the
/// order of the offsets. Since ticket 23 there is a third: the file writes
/// `\v 55` and then `\v 55b`, and a bare number covers its own segments.
#[test]
fn machine_py_03lev() {
    const SOURCE: &str =
        include_str!("../../../tasks/conformance/fixtures/machine-py/Tes/03LEVTes.SFM");
    assert_eq!(
        common::codes(SOURCE),
        vec![
            Code::MissingId,
            Code::UnknownBookCode,
            Code::DuplicateVerseNumber
        ]
    );
    assert_eq!(common::parser_codes(SOURCE), vec![Code::UnknownBookCode]);
}

/// Every code [`Code::is_semantic`] names has a snapshot produced by a test in
/// this file. The mirror of `recovery.rs`'s `recovery_table_is_covered`, which
/// skips exactly these codes.
#[test]
fn semantic_checks_are_covered() {
    let missing: Vec<&str> = Code::ALL
        .iter()
        .filter(|code| code.is_semantic())
        .map(|code| code.as_str())
        .filter(|name| {
            let path = format!(
                "{}/tests/snapshots/checks__{}.snap",
                env!("CARGO_MANIFEST_DIR"),
                name.replace('-', "_")
            );
            !std::path::Path::new(&path).exists()
        })
        .collect();
    assert!(
        missing.is_empty(),
        "semantic codes without a check test: {missing:?}"
    );
}
