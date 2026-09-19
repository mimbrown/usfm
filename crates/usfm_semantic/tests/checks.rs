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

use usfm::diagnostics::Code;

/// Snapshot named `checks__<code>`.
fn check(code: Code, source: &str) {
    check_named(code, code.as_str().replace('-', "_"), source);
}

/// `recovery.rs`'s `check_variant` (a second case of one code, snapshotted as
/// `checks__<code>__<variant>`) has no caller yet and so is not written here;
/// ticket 20's checks are what bring it over.
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
