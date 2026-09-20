//! The round-trip property — parse, write USFM, parse again — in one place.
//!
//! Three assertions, the ones ticket 25 proved on the `pass` corpus and
//! ticket 27 turned into an invariant over *every* input:
//!
//! 1. the two trees are equal ignoring spans ([`usfm::ast::eq_ignoring_spans`],
//!    which also compares styles by marker rather than by index);
//! 2. the second parse reports no diagnostic code the first did not
//!    ([`codes_are_not_gained`]);
//! 3. writing the second tree gives the same text back, byte for byte — the
//!    writer's output is a fixed point.
//!
//! On 2: the second parse's codes are a *subset*, not the same multiset. That
//! is the writer doing its job. The AST records what a construct *is*, not
//! which of several spellings the source used, so the writer emits the one
//! canonical spelling and a diagnostic about the other spelling has nothing
//! left to fire on: `\v 01` is `\v 1` once the number is a `usize`, an
//! implicitly closed `\add` comes back with its `\add*`, a `\nd` nested
//! without `+` comes back with one, an unquoted attribute value comes back
//! quoted, a stray `\` comes back as `\\`.
//!
//! It is deliberately *not* "the second parse reports no error": an Error the
//! tree still holds is written back and reported again, which is the writer
//! being faithful, not a bug. `\xq` in a paragraph is `marker-not-allowed-here`
//! both times because the node is still there and still in the wrong place —
//! the writer's job is to reproduce the document, not to repair it. What would
//! be a bug is an error the *first* parse did not report, and that is what
//! assertion 2 catches, at every severity rather than only at Error.
//!
//! That is a measurement, not a preference: ticket 27 asked whether "no Error
//! on the second parse" holds over the whole conformance corpus, and it does
//! not. 21 of the 275 cases keep one, and each is a document that is still
//! wrong after being written out faithfully — `missing-id` (the book has no
//! `\id`), `verse-text-before-chapter` and `verse-outside-chapter` (a `\v`
//! with no `\c` above it), `empty-book`, `unexpected-table-column`,
//! `id-not-first`, `unmatched-sidebar-end`, `marker-not-allowed-here`,
//! `empty-attribute-list`, `no-default-attribute`,
//! `default-attribute-with-others`, `verse-in-heading`. None of them is a
//! codegen or parser bug, and none of them would be fixed by a writer: a
//! writer that made them go away would be editing the document.
//!
//! Assertion 3 is what keeps assertion 2 from hiding a bug: if the writer
//! produced something the parser reads *differently*, the second tree would
//! write out differently too. Together the three say the writer canonicalises
//! and then stands still.
//!
//! This module is the single definition of the property. `crates/usfm_codegen`'s
//! `tests/roundtrip.rs` uses it over the `pass` corpus and the benchmark
//! books, the runner's `--roundtrip` gates it over every conformance case of
//! every root, and `tasks/fuzz`'s `roundtrip` target asserts the same three
//! things over arbitrary bytes.

use std::collections::BTreeMap;

use usfm::codegen::to_usfm_string;

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

/// Parse, write, parse, and check the three properties. The `Err` is the
/// report a failing case prints.
pub fn check(source: &str) -> Result<(), String> {
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

/// One case that did not round-trip: its name and the report.
pub struct RoundTripFailure {
    pub name: String,
    pub reason: String,
}

/// Run [`check`] over every case of every root that has an input, in the order
/// [`discover_tests`](crate::discover_tests) returns them.
///
/// Every case counts — `pass`, `fail`, and the ones with no `origin.xml`. The
/// property does not depend on the input being valid USFM: an invalid input is
/// parsed into whatever the recovery rules make of it, and writing *that* tree
/// out and reading it back must still give the same tree.
///
/// Returns the number of cases run and the failures among them.
pub fn run_roundtrip(tests: &[crate::TestCase]) -> (usize, Vec<RoundTripFailure>) {
    let mut count = 0;
    let mut failures = Vec::new();
    for test in tests {
        if !test.has_usfm() {
            continue;
        }
        let Ok(source) = test.read_usfm() else {
            failures.push(RoundTripFailure {
                name: test.name.clone(),
                reason: format!("cannot read {}", test.usfm_path().display()),
            });
            continue;
        };
        count += 1;
        if let Err(reason) = check(&source) {
            failures.push(RoundTripFailure {
                name: test.name.clone(),
                reason,
            });
        }
    }
    (count, failures)
}
