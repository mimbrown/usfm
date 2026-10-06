//! Every fix, applied and parsed again: the code must be gone and no new
//! code may appear. Through the facade, since the diagnostics a fix answers
//! are the parser's and the semantic pass's together.

use usfm::Code;
use usfm::diagnostics::Diagnostic;
use usfm::span::Span;
use usfm_fix::{FIXABLE, Fix, fix, fixes, is_fixable};

/// `source` with one `fix` applied.
fn apply(source: &str, fix: &Fix) -> String {
    let (text, taken) = usfm_fix::apply(source, std::slice::from_ref(fix));
    assert_eq!(taken, [0]);
    text
}

fn codes(source: &str) -> Vec<Code> {
    usfm::parse(source)
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}

/// Fix the first `code` in `source` and hand back what the file becomes.
///
/// The assertion every fix has to pass: the re-parse reports that code
/// one time less — none at all where the input had one, which is every
/// case here but the document with two unknown markers, where fixing one
/// leaves the other for its own action — and reports nothing the first
/// parse did not.
fn fixed(code: Code, source: &str) -> String {
    let result = usfm::parse(source);
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == code)
        .unwrap_or_else(|| panic!("{code} in {source:?}, got {:?}", codes(source)));
    let fix = fix(&result.document, source, diagnostic)
        .unwrap_or_else(|| panic!("a fix for {code} in {source:?}"));

    let fixed = apply(source, &fix);
    let before = codes(source);
    let after = codes(&fixed);
    let count = |codes: &[Code]| codes.iter().filter(|each| **each == code).count();
    assert_eq!(
        count(&after),
        count(&before) - 1,
        "{code} survived the fix: {source:?} -> {fixed:?} ({after:?})",
    );
    let gained: Vec<Code> = after
        .iter()
        .copied()
        .filter(|code| !before.contains(code))
        .collect();
    assert!(
        gained.is_empty(),
        "the fix for {code} gained {gained:?}: {source:?} -> {fixed:?}",
    );
    fixed
}

#[test]
fn an_unknown_marker_is_deleted_with_the_space_after_it() {
    assert_eq!(
        fixed(
            Code::UnknownMarker,
            "\\id GEN\n\\c 1\n\\p \\v 1 a \\foo b\n"
        ),
        "\\id GEN\n\\c 1\n\\p \\v 1 a b\n",
    );
    // A closing marker of the same unknown style is one too.
    assert_eq!(
        fixed(
            Code::UnknownMarker,
            "\\id GEN\n\\c 1\n\\p \\v 1 a \\foo custom\\foo* b\n",
        ),
        "\\id GEN\n\\c 1\n\\p \\v 1 a custom\\foo* b\n",
    );
    // On a line of its own, with nothing after it.
    assert_eq!(
        fixed(Code::UnknownMarker, "\\id GEN\n\\c 1\n\\qqq\n\\p \\v 1 a\n"),
        "\\id GEN\n\\c 1\n\n\\p \\v 1 a\n",
    );
}

#[test]
fn an_open_character_style_gets_its_closer() {
    // Closed by the next paragraph: the closer goes after the text, not
    // after the line break the text run's span swept up.
    assert_eq!(
        fixed(
            Code::CharacterStyleNotClosed,
            "\\id GEN\n\\c 1\n\\p \\v 1 a \\em b\n\\p \\v 2 c\n",
        ),
        "\\id GEN\n\\c 1\n\\p \\v 1 a \\em b\\em*\n\\p \\v 2 c\n",
    );
    // Closed by the end of the file.
    assert_eq!(
        fixed(
            Code::CharacterStyleNotClosed,
            "\\id GEN\n\\c 1\n\\p \\v 1 a \\em b",
        ),
        "\\id GEN\n\\c 1\n\\p \\v 1 a \\em b\\em*",
    );
    // Closed by the next cell.
    assert_eq!(
        fixed(
            Code::CharacterStyleNotClosed,
            "\\id GEN\n\\c 1\n\\tr \\tc1 \\em a \\tc2 b\n",
        ),
        "\\id GEN\n\\c 1\n\\tr \\tc1 \\em a\\em* \\tc2 b\n",
    );
    // Closed by an outer closing marker: the nested style's own closer
    // goes *before* it, and it is spelled `\+nd*` as the source spelled
    // the opener.
    assert_eq!(
        fixed(
            Code::CharacterStyleNotClosed,
            "\\id GEN\n\\c 1\n\\p \\v 1 \\em a \\+nd b\\em* c\n",
        ),
        "\\id GEN\n\\c 1\n\\p \\v 1 \\em a \\+nd b\\+nd*\\em* c\n",
    );
}

#[test]
fn a_note_without_a_caller_gets_the_plus() {
    assert_eq!(
        fixed(
            Code::MissingNoteCaller,
            "\\id GEN\n\\c 1\n\\p \\v 1 a\\f \\ft note\\f* b\n",
        ),
        "\\id GEN\n\\c 1\n\\p \\v 1 a\\f + \\ft note\\f* b\n",
    );
}

#[test]
fn an_unquoted_attribute_value_is_quoted() {
    assert_eq!(
        fixed(
            Code::AttributeValueNotQuoted,
            "\\id GEN\n\\c 1\n\\p \\v 1 \\w word|lemma=grace\\w*\n",
        ),
        "\\id GEN\n\\c 1\n\\p \\v 1 \\w word|lemma=\"grace\"\\w*\n",
    );
}

#[test]
fn an_empty_milestone_attribute_list_is_deleted_with_its_space() {
    assert_eq!(
        fixed(
            Code::EmptyMilestoneAttributeList,
            "\\id GEN\n\\c 1\n\\p \\v 1 a \\ts-s |\\* b\n",
        ),
        "\\id GEN\n\\c 1\n\\p \\v 1 a \\ts-s\\* b\n",
    );
    // On a milestone the sheet does not define, whose own diagnostic
    // stays: the fix takes the list and leaves that alone.
    assert_eq!(
        fixed(
            Code::EmptyMilestoneAttributeList,
            "\\id GEN\n\\c 1\n\\p \\v 1 a \\zaln-s |\\* b\n",
        ),
        "\\id GEN\n\\c 1\n\\p \\v 1 a \\zaln-s\\* b\n",
    );
}

/// The title is what the lightbulb menu shows, so it names the marker it
/// is about.
#[test]
fn a_fix_is_titled_after_what_it_edits() {
    let source = "\\id GEN\n\\c 1\n\\p \\v 1 a \\foo b\n";
    let result = usfm::parse(source);
    let diagnostic = &result.diagnostics[0];
    assert_eq!(
        fix(&result.document, source, diagnostic)
            .expect("a fix")
            .title,
        "Delete `\\foo`",
    );
}

/// A code with no obvious edit has no action rather than a guessed one.
#[test]
fn a_code_without_an_obvious_edit_has_no_fix() {
    let source = "\\c 1\n\\p \\v 1 a\n";
    let result = usfm::parse(source);
    let missing_id = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == Code::MissingId)
        .expect("`missing-id`");
    assert_eq!(fix(&result.document, source, missing_id), None);
}

/// A diagnostic from another text — an editor that changed the file
/// between the parse and the request — edits nothing.
#[test]
fn a_span_past_the_end_of_the_source_has_no_fix() {
    let source = "\\id GEN\n\\c 1\n\\p \\v 1 a \\foo b\n";
    let result = usfm::parse(source);
    let mut diagnostic = Diagnostic::new(
        Code::UnknownMarker,
        Span::new(900, 904),
        "unknown marker `\\foo`",
    );
    assert_eq!(fix(&result.document, source, &diagnostic), None);
    // And one that points at text which is not a marker at all: `GEN`.
    diagnostic.span = Span::new(4, 7);
    assert_eq!(fix(&result.document, source, &diagnostic), None);
}

/// Every fixable code has a fix in this file's cases, and nothing else does:
/// the list and the `match` in `fix` cannot drift apart.
#[test]
fn the_fixable_list_is_the_codes_that_have_a_fix() {
    let sources = [
        "\\id GEN\n\\c 1\n\\p \\v 1 a \\foo b\n",
        "\\id GEN\n\\c 1\n\\p \\v 1 a \\em b\n",
        "\\id GEN\n\\c 1\n\\p \\v 1 a\\f \\ft note\\f* b\n",
        "\\id GEN\n\\c 1\n\\p \\v 1 \\w word|lemma=grace\\w*\n",
        "\\id GEN\n\\c 1\n\\p \\v 1 a \\ts-s |\\* b\n",
        // And codes with none.
        "\\c 1\n\\v 2 a\n\\v 1 b \\xq c\n",
    ];
    let mut fixed: Vec<Code> = Vec::new();
    for source in sources {
        let result = usfm::parse(source);
        for found in fixes(&result.document, source, &result.diagnostics, None) {
            assert!(is_fixable(found.code), "{} is not listed", found.code);
            fixed.push(found.code);
        }
    }
    for code in FIXABLE {
        assert!(fixed.contains(code), "no case here fixes {code}");
    }
}

/// `only` narrows the fixes to the codes named.
#[test]
fn only_the_codes_asked_for_are_fixed() {
    let source = "\\id GEN\n\\c 1\n\\p \\v 1 a \\foo b \\w word|lemma=grace\\w*\n";
    let result = usfm::parse(source);
    let all = fixes(&result.document, source, &result.diagnostics, None);
    assert_eq!(all.len(), 2);
    let only = fixes(
        &result.document,
        source,
        &result.diagnostics,
        Some(&[Code::AttributeValueNotQuoted]),
    );
    assert_eq!(only.len(), 1);
    assert_eq!(only[0].code, Code::AttributeValueNotQuoted);
    assert_eq!(
        usfm_fix::apply(source, &only).0,
        "\\id GEN\n\\c 1\n\\p \\v 1 a \\foo b \\w word|lemma=\"grace\"\\w*\n",
    );
}

/// Several fixes go in together, and the result reports none of them.
#[test]
fn several_fixes_are_applied_in_one_pass() {
    let source = "\\id GEN\n\\c 1\n\\p \\v 1 a \\foo b\\f \\ft note\\f* \\w word|lemma=grace\\w* \\ts-s |\\*\n\\p \\v 2 \\em c\n";
    let result = usfm::parse(source);
    let all = fixes(&result.document, source, &result.diagnostics, None);
    assert_eq!(all.len(), 5);
    let (fixed, taken) = usfm_fix::apply(source, &all);
    assert_eq!(taken, [0, 1, 2, 3, 4]);
    assert_eq!(
        fixed,
        "\\id GEN\n\\c 1\n\\p \\v 1 a b\\f + \\ft note\\f* \\w word|lemma=\"grace\"\\w* \\ts-s\\*\n\\p \\v 2 \\em c\\em*\n",
    );
    assert!(
        codes(&fixed).iter().all(|code| !is_fixable(*code)),
        "{:?}",
        codes(&fixed)
    );
}

/// Two fixes that touch the same place are not both applied: the first is,
/// and the other is left for the next parse to report again.
#[test]
fn overlapping_fixes_are_not_applied_together() {
    // Two unknown markers back to back: deleting the first takes the space
    // the second starts after, so their edits touch.
    let source = "\\id GEN\n\\c 1\n\\p \\v 1 a \\foo \\bar b\n";
    let result = usfm::parse(source);
    let all = fixes(&result.document, source, &result.diagnostics, None);
    assert_eq!(all.len(), 2);
    let (once, taken) = usfm_fix::apply(source, &all);
    assert_eq!(taken, [0]);
    assert_eq!(once, "\\id GEN\n\\c 1\n\\p \\v 1 a \\bar b\n");

    let result = usfm::parse(&once);
    let rest = fixes(&result.document, &once, &result.diagnostics, None);
    let (twice, _) = usfm_fix::apply(&once, &rest);
    assert_eq!(twice, "\\id GEN\n\\c 1\n\\p \\v 1 a b\n");
    assert!(codes(&twice).is_empty());
}
