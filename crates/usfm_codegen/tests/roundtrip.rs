//! M5's exit criterion: parse -> codegen -> parse yields the same tree.
//!
//! Four corpora, every one of them real USFM rather than a fixture written
//! for this test:
//!
//! * every tcdocs case marked `pass` (`tcdocs/tests`),
//! * every usfm-grammar `bugfixes` case marked `pass`
//!   (`tasks/conformance/fixtures/usfm-grammar`),
//! * the benchmark corpus's `web/` — 86 files, the whole World English Bible,
//! * the machine.py books (`tasks/conformance/fixtures/machine-py`), which are
//!   not a harness root and so are reached by no `discover_tests` walk. They
//!   are here because one of them, `41MATTes.SFM`, was the one input in the
//!   repo that did not round-trip (ticket 28, fixed by ticket 27).
//!
//! and the three assertions of [`usfm_tests::roundtrip`], which is where the
//! property itself lives (ticket 27 gave it a fuzz target and a gate step, and
//! all three call the one definition): the trees are equal ignoring spans, the
//! second parse gains no diagnostic code, and the output is a fixed point.
//!
//! What this test adds is the corpora. The gate's `--roundtrip` step runs the
//! same property over every conformance case, `pass` and `fail` alike; here it
//! is the `pass` cases — where the tree is a real document rather than a
//! repair — and the 86 books of the benchmark corpus, which no other check
//! reaches. Last comes a fifth, smaller corpus:
//! [`the_fuzz_findings_round_trip`], the minimised inputs ticket 27's fuzz
//! target found, so a regression in any of them is a failing `cargo test`.

use usfm_benchmark::FileClass;
use usfm_tests::roundtrip::check;
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

/// The machine.py fixtures: seven Paratext-shaped books, among them the
/// `41MATTes.SFM` of ticket 28 and a zero-byte one. They ship no reference
/// USX and are not a harness root, so this test names their directory rather
/// than discovering them.
#[test]
fn the_machine_py_fixtures_round_trip() {
    let root = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tasks/conformance/fixtures/machine-py"
    );
    let mut cases: Vec<(String, String)> = vec![];
    let mut stack = vec![std::path::PathBuf::from(root)];
    while let Some(directory) = stack.pop() {
        let entries = std::fs::read_dir(&directory)
            .unwrap_or_else(|err| panic!("reading {}: {err}", directory.display()));
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|extension| extension == "SFM") {
                let name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let source = std::fs::read_to_string(&path)
                    .unwrap_or_else(|err| panic!("reading {}: {err}", path.display()));
                cases.push((name, source));
            }
        }
    }
    cases.sort();
    let count = cases.len();
    let failures = run(cases);
    report("machine.py fixtures", count, failures);
}

/// The inputs the `roundtrip` fuzz target found (ticket 27, and the last three
/// of them fixed by ticket 35), minimised with `cargo fuzz tmin`: twenty-five
/// inputs, eighteen bugs — one of the bugs turned up three times and another
/// twice, under different spellings — and one more, ticket 28's verse end,
/// which is a whole file and is covered by
/// [`the_machine_py_fixtures_round_trip`] instead. Three were in `usfm_ast`'s
/// `add_child`, one in its `NumberRange` display, thirteen in the parser and
/// one here. Each is fixed, and each is listed so a regression is a failing
/// `cargo test` rather than the next ten-minute fuzz run; the rule each one
/// broke has its own test in the crate that was wrong, and this checks the
/// property they were found by.
#[test]
fn the_fuzz_findings_round_trip() {
    let cases: Vec<(String, String)> = [
        // A stray `\` and a `\*` that ends no milestone: two text runs merged
        // with the whitespace of both, giving a `Text` holding two spaces.
        ("stray_backslash_then_dropped_milestone_end", "\\ \\* i"),
        // The same drop at the start of a paragraph: nothing to merge with, so
        // the whitespace stayed as the first text's leading space.
        ("dropped_milestone_end_opens_a_paragraph", "\\p\\* n"),
        // The same drop just after `\v N`, whose space the verse marker had
        // already eaten: the writer always puts one back, so the text's own
        // leading space made two.
        ("dropped_milestone_end_after_a_verse", "\\v 3\\* x"),
        // A `\v` on a `\periph` line: the start was thrown away with the rest
        // of the line, the end was emitted into the paragraph before it.
        ("verse_on_a_periph_line", " i\\periph\\v 2"),
        // A `\va`/`\vp` that reached the paragraph as a character style,
        // because the `\v` between it and the verse was dropped: written
        // back, they are the verse's alternate and published numbers.
        ("va_and_vp_after_a_dropped_verse", "\\v 1\\v\\va 3\\va*\\vp 1b\\vp* text"),
        ("empty_vp_after_a_dropped_verse", "\\v 1\\v\\vp"),
        // A `\vp` after `\v N` that is never closed: kept as a `Char` beside
        // the verse, which is a tree no USFM spells — closing it, as the
        // writer must, makes the parser read it as the published number.
        ("unclosed_vp_after_a_verse", "\\v 1\\vp x"),
        // A `\periph` title built by joining the line's text runs across a
        // character style, which contributes none: the whitespace on both
        // sides of it ended up side by side in the title.
        ("periph_title_across_a_style", "iT\n\\periph\u{0}*\n\\w 1\"\\w*\np"),
        // A note's `\cat` category, joined the same way across a character
        // style and doubling the whitespace at the seam the same way.
        (
            "note_category_across_a_style",
            "\\e-\\ef -\\cat\u{0} \\rb \u{5}b\\rb*\no\"r*\n",
        ),
        // A verse after text on the `\esbe` line: that line becomes an
        // implicit `\p`, so the previous verse's end goes inline after the
        // text, not before the sidebar as it does when the line is bare.
        ("verse_after_text_on_the_esbe_line", "\\v 1p\\esb\\esbe\\.\\v 7"),
        // A block after a `\periph` that only a dropped `\id` ended: written
        // out, the periph swallows it.
        // A `\periph` inside a sidebar swallowed the `\esbe` that closes it.
        ("periph_inside_a_sidebar", "\\esb\\periph"),
        ("block_after_a_periph_that_nothing_ended", "\\periph\\id\\"),
        ("verse_after_a_periph_that_nothing_ended", "\u{fffd}\\periph\\id\\v 3"),
        // Two tables that only a dropped marker separates: written out, the
        // rows run together into one table.
        ("two_tables_with_a_dropped_marker", "\\tr \\tc1 x\\c\n\\tr \\tc1 y"),
        // A `\cp` paragraph after a chapter start, which only a dropped
        // marker can put there: written out, the chapter absorbs it.
        ("cp_paragraph_after_a_chapter", "\\c 3\\c\n\\cp"),
        ("cp_paragraph_after_a_chapter_with_words", "\\c 3\\c\n\\cp A B"),
        // A block-level milestone after a paragraph that only a dropped
        // marker closed: written out, the paragraph runs on and swallows it.
        ("block_milestone_after_a_dropped_id", "r\\id\\-\\*"),
        // A verse range whose ends are the same number but whose end carries
        // a modifier: written as the bare number, the modifier was lost.
        ("collapsed_verse_range_with_an_end_modifier", "\\v 4-4t"),
        // A default attribute value ending in whitespace at the end of a
        // `\periph` list: the writer's own line break read back without it.
        ("periph_default_attribute_ending_in_space", "\\periph|: "),
        ("periph_default_attribute_ended_by_a_marker", "\\periph|s \\"),
        // A default attribute whose verbatim value does not end in whitespace,
        // followed by a named pair: the separator the writer added was read
        // back as part of the value.
        ("default_attribute_then_named_pair", "\\z|\"a=\\*"),
        // The same shape on a character style rather than a milestone.
        ("default_attribute_then_named_pair_on_a_char", "\\p \\w x|\"a=\\w*"),
        // A block-level milestone standing where the construct before it is
        // written as a line that runs on — after a `Sidebar`, after a `Table`,
        // at the head of a `\periph` division — so the written form reads it
        // back inside that line (ticket 35). The last of the three was worse:
        // the `\periph` title keeps only text, so the milestone was dropped
        // without a diagnostic.
        ("block_milestone_after_a_sidebar", "\\esb\\c\\sh\\*"),
        ("block_milestone_after_a_table", "\\p x\n\\tr \\tc1 y\n\\c\n\\zaln-s\\*"),
        ("block_milestone_at_the_head_of_a_periph", "\\periph\\id\\e\\*"),
        // A published verse number holding a backslash, written raw: the `\`
        // and the `\` of the `\vp*` after it made one escape, and the number
        // came back as `\vp*`.
        ("published_verse_number_that_is_a_backslash", "\\v 1\\vp\\"),
    ]
    .into_iter()
    .map(|(name, source)| (name.to_string(), source.to_string()))
    .collect();
    let count = cases.len();
    let failures = run(cases);
    report("fuzz findings", count, failures);
}
