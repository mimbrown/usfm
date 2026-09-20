//! M5's exit criterion: parse -> codegen -> parse yields the same tree.
//!
//! Three corpora, every one of them real USFM rather than a fixture written
//! for this test:
//!
//! * every tcdocs case marked `pass` (`tcdocs/tests`),
//! * every usfm-grammar `bugfixes` case marked `pass`
//!   (`tasks/conformance/fixtures/usfm-grammar`),
//! * the benchmark corpus's `web/` — 86 files, the whole World English Bible.
//!
//! and three assertions per case:
//!
//! 1. the two trees are equal ignoring spans (`usfm_ast::eq_ignoring_spans`,
//!    which also compares styles by marker rather than by index);
//! 2. the second parse reports no diagnostic code the first did not
//!    ([`codes_are_not_gained`]);
//! 3. writing the second tree gives the same text back, byte for byte — the
//!    writer's output is a fixed point.
//!
//! On 2: the second parse's codes are a *subset*, not the same multiset. That
//! is the writer doing its job, and the shape of the difference is the whole
//! reason this crate exists. The AST records what a construct *is*, not which
//! of several spellings the source used, so the writer emits the one canonical
//! spelling and a diagnostic about the other spelling has nothing left to fire
//! on: `\v 01` is `\v 1` once the number is a `usize`, an implicitly closed
//! `\add` comes back with its `\add*`, a `\nd` nested without `+` comes back
//! with one, an unquoted attribute value comes back quoted, a stray `\` comes
//! back as `\\`. Assertion 3 is what keeps that from hiding a bug: if the
//! writer produced something the parser reads *differently*, the second tree
//! would write out differently too. Together the three say the writer
//! canonicalises and then stands still.

use std::collections::BTreeMap;

use usfm_benchmark::FileClass;
use usfm_codegen::to_usfm_string;
use usfm_tests::{ValidationStatus, discover_tests};

/// Cases the writer cannot round-trip because the parser discards something
/// the tree has no room for. Each entry is `(case name, reason)`, and each one
/// is a ticket rather than a licence: nothing here is a spelling the writer
/// could choose differently.
const KNOWN: &[(&str, &str)] = &[];

/// The result of one case, so a run reports every failure rather than the
/// first.
struct Failure {
    name: String,
    reason: String,
}

/// The diagnostic codes of a parse, counted. Keyed by the code's name, which
/// `Code` has and `Ord` has not, so a report lists them in a fixed order.
fn codes(diagnostics: &[usfm::Diagnostic]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for diagnostic in diagnostics {
        *counts.entry(diagnostic.code.to_string()).or_insert(0) += 1;
    }
    counts
}

/// Whether the second parse gained a code, or more of one, that the first did
/// not report. Losing one is the writer canonicalising; gaining one is a bug.
fn codes_are_not_gained(
    first: &BTreeMap<String, usize>,
    second: &BTreeMap<String, usize>,
) -> Result<(), String> {
    let before = |code: &String| *first.get(code).unwrap_or(&0);
    let gained: Vec<String> = second
        .iter()
        .filter(|(code, count)| before(code) < **count)
        .map(|(code, count)| format!("{code} ({} -> {count})", before(code)))
        .collect();
    if gained.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "the second parse gained diagnostics: {}",
            gained.join(", ")
        ))
    }
}

/// Parse, write, parse, and check the three properties.
fn check(source: &str) -> Result<(), String> {
    let first = usfm::parse(source);
    let written = to_usfm_string(&first.document);
    let second = usfm::parse(&written);

    if !usfm::ast::eq_ignoring_spans(&first.document, &second.document) {
        return Err(format!(
            "the tree changed.\n--- written ---\n{}\n--- before ---\n{:#?}\n--- after ---\n{:#?}",
            written, first.document, second.document
        ));
    }
    codes_are_not_gained(&codes(&first.diagnostics), &codes(&second.diagnostics))
        .map_err(|reason| format!("{reason}\n--- written ---\n{written}"))?;
    let rewritten = to_usfm_string(&second.document);
    if rewritten != written {
        return Err(format!(
            "the output is not a fixed point.\n--- first ---\n{written}\n--- second ---\n{rewritten}"
        ));
    }
    Ok(())
}

/// Run `check` over every named source, honouring [`KNOWN`]: a listed case
/// must still fail, so an entry that has been fixed fails the run and is
/// deleted rather than left behind.
fn run(cases: Vec<(String, String)>) -> Vec<Failure> {
    let mut failures = vec![];
    for (name, source) in cases {
        let known = KNOWN.iter().find(|(case, _)| *case == name);
        match (check(&source), known) {
            (Ok(()), None) => {}
            (Ok(()), Some((_, reason))) => failures.push(Failure {
                name,
                reason: format!("is in KNOWN ({reason}) but round-trips; remove the entry"),
            }),
            (Err(_), Some(_)) => {}
            (Err(reason), None) => failures.push(Failure { name, reason }),
        }
    }
    failures
}

fn report(what: &str, count: usize, failures: Vec<Failure>) {
    assert!(count > 0, "{what}: no cases found");
    if failures.is_empty() {
        println!("{what}: {count} cases round-trip");
        return;
    }
    let listed: Vec<String> = failures
        .iter()
        .map(|failure| format!("\n=== {} ===\n{}", failure.name, failure.reason))
        .collect();
    panic!(
        "{what}: {} of {count} cases failed to round-trip:{}",
        failures.len(),
        listed.join("")
    );
}

/// Every `pass` case of both conformance roots. A `fail` case is deliberately
/// left out: its input is invalid USFM, the tree is whatever the recovery
/// rules made of it, and the writer has no obligation to reproduce a repair
/// that was never a document.
#[test]
fn every_conformance_pass_case_round_trips() {
    let cases: Vec<(String, String)> = discover_tests()
        .into_iter()
        .filter(|test| test.metadata.validated == ValidationStatus::Pass && test.has_usfm())
        .map(|test| {
            let source = test
                .read_usfm()
                .unwrap_or_else(|err| panic!("reading {}: {err}", test.name));
            (test.name, source)
        })
        .collect();
    let count = cases.len();
    let failures = run(cases);
    report("conformance", count, failures);
}

/// The benchmark corpus's `web/` class: 86 whole books, the largest body of
/// ordinary USFM in the repo.
#[test]
fn the_benchmark_corpus_round_trips() {
    let cases: Vec<(String, String)> = FileClass::Plain
        .load()
        .into_iter()
        .map(|file| {
            let name = file
                .path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            (name, file.text)
        })
        .collect();
    let count = cases.len();
    let failures = run(cases);
    report("benchmark corpus", count, failures);
}
