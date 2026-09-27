//! The USX reader over both conformance roots (ticket 45).
//!
//! `usfm_usx::read_usx` is tested case by case in
//! `crates/usfm_usx/tests/reader.rs`; this file is where it meets the
//! reference files, which only this crate knows how to find, patch and
//! compare. Four properties, each over every reference it applies to; the
//! second is asserted by the runner, and the other three here:
//!
//! 1. [`every_reference_reads`][]: every `origin.xml` of both roots reads
//!    without a panic, as the file has it and as the harness reads it; the
//!    second has no Error on a `pass` case.
//! 2. That writing a read reference gives the reference, for every case the
//!    harness compares, is the runner's `--usx-read` step (ticket 46), which
//!    the gate runs against `tasks/conformance/usx-read-known.txt`; its
//!    definition is `usfm_tests::usx_properties::check_read`. It was a test
//!    here until then, and is not asserted twice.
//! 3. [`a_read_reference_is_the_parse_of_its_usfm`][]: the cases of 2, the
//!    read tree against `usfm::parse` of `origin.usfm` by
//!    `eq_ignoring_spans`, after `usfm_usx::testing::normalise` — the list
//!    of what a USX file cannot say, measured over the corpus rather than
//!    guessed, with each entry naming a case that shows it — and, on the
//!    usfm-grammar root only, [`CollapseWhitespace`], the fourth entry, which
//!    is the harness's rule for that root rather than something USX cannot
//!    say. The list lives in `usfm_usx` behind `testing` since ticket 49, so
//!    `tasks/fuzz`'s `usx_roundtrip` target compares through the same one;
//!    that target added two entries (unwritable attributes, the empty `|`),
//!    which no compared case needs.
//! 4. [`every_read_node_has_a_span_in_its_source`][]: `span_check`'s
//!    invariants, as they read for XML (`span_check::read_violations`), over
//!    every reference.
//! 5. [`a_reference_reads_the_same_without_its_eids`][]: USX 2 (ticket 47).
//!    Every reference with each `<verse eid>` and `<chapter eid>` stripped
//!    reads to the tree the reference itself reads to, by
//!    `eq_ignoring_spans`: the ends the reader builds are the ones the file
//!    had, so a file reads to one tree whichever version it is. It holds for
//!    every compared case; of the rest, one reference is on
//!    [`STRIPPED_EIDS_KNOWN`], where the reference's `eid` placement, not the
//!    reader, is what differs. [`a_usx_2_file_is_the_parse_of_its_usfm`] is
//!    the same rule against the parser directly, for the shapes the corpus
//!    has few of.
//! 6. [`the_web_books_round_trip`][]: machine.py's WEB books, real USX no
//!    tool of ours wrote (ticket 47), read with no Error and go USX -> USFM
//!    -> USX: the parse of the written USFM is the read tree, and its USX is
//!    the file under the harness's comparison. Their trees and diagnostics,
//!    and the malformed Tes book's, are snapshots in
//!    `crates/usfm_usx/tests/reader.rs`.
//!
//! The gate's `cargo test --workspace` excludes this package (its generated
//! tests are the harness's own, which `cargo run` already runs), so
//! `scripts/gate.sh` runs this file by name.

use std::borrow::Cow;
use std::panic::{self, AssertUnwindSafe};
use std::sync::LazyLock;

use regex::Regex;

use usfm::ast::visit_mut::VisitMut;
use usfm::ast::{Char, Inline, Note, Para, TableCell, Text, eq_ignoring_spans};
use usfm::diagnostics::Severity;
use usfm::parser::span_check::read_violations;
use usfm::usx::testing::normalise;
use usfm::usx::{UsxOptions, read_usx, to_usx_node_with_options, to_usx_string};
use usfm::{DEFAULT_STYLESHEET, parse_with_options};
use usfm_tests::{
    TestCase, ValidationStatus, collapse_whitespace_tree, compare_xml, discover_tests,
    normalize_tree, usx_properties,
};

/// The cases the harness compares today and passes: a reference exists and
/// the parser's output matched it. Property 3 is about exactly these, and so
/// is the runner's `--usx-read`, whose definition this is.
fn compared_cases() -> Vec<TestCase> {
    let tests = discover_tests();
    usx_properties::compared_cases(&tests)
        .into_iter()
        .cloned()
        .collect()
}

/// Every reference of both roots, `pass` and `fail` alike.
fn references() -> Vec<TestCase> {
    let cases: Vec<TestCase> = discover_tests()
        .into_iter()
        .filter(TestCase::has_expected_usx)
        .collect();
    // 258 tcdocs references and 13 usfm-grammar ones (spec, M7): a missing
    // submodule must not pass as an empty corpus.
    assert!(cases.len() >= 271, "only {} references found", cases.len());
    cases
}

/// Fail with every case that broke `property`, not just the first. There is
/// no known-failure list: the reader holds the three properties here over
/// every case they cover, and a case that stops holding one is a reader bug.
fn assert_none(property: &str, failures: Vec<(String, String)>) {
    assert!(
        failures.is_empty(),
        "{property}: {} case(s)\n{}",
        failures.len(),
        failures
            .iter()
            .map(|(name, why)| format!("{name}: {why}"))
            .collect::<Vec<_>>()
            .join("\n\n"),
    );
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown panic".to_string())
}

#[test]
fn every_reference_reads() {
    let mut failures = Vec::new();
    for case in &references() {
        // The file as it is, byte-order mark and all: it must not panic,
        // whatever it reports.
        let raw = std::fs::read_to_string(case.usx_path()).unwrap();
        if let Err(payload) = panic::catch_unwind(AssertUnwindSafe(|| read_usx(&raw))) {
            failures.push((
                case.name.clone(),
                format!("panicked: {}", panic_message(payload)),
            ));
            continue;
        }
        // As the harness reads it: a `pass` case reads with no Error.
        let text = case.expected_usx_text().unwrap();
        let result = read_usx(&text);
        let errors: Vec<String> = result
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Error)
            .map(ToString::to_string)
            .collect();
        if case.metadata.validated != ValidationStatus::Fail && !errors.is_empty() {
            failures.push((case.name.clone(), errors.join("; ")));
        }
    }
    assert_none("every reference reads", failures);
}

#[test]
fn a_read_reference_is_the_parse_of_its_usfm() {
    let cases = compared_cases();
    let mut failures = Vec::new();
    for case in &cases {
        let usfm = case.read_usfm().unwrap();
        // With verse and chapter ends whether the reference has them or not:
        // the reader builds the ones a USX 2 file leaves out (ticket 47), so
        // `advanced/complex`, the one compared reference with none, reads to
        // the parse that has them.
        let mut parsed = parse_with_options(&usfm, &DEFAULT_STYLESHEET, true).document;
        let text = case.expected_usx_text().unwrap();
        let mut read = read_usx(&text).document;
        normalise(&mut parsed);
        normalise(&mut read);
        if case.name.starts_with("usfm-grammar/") {
            CollapseWhitespace.visit_document(&mut parsed);
            CollapseWhitespace.visit_document(&mut read);
        }
        if !eq_ignoring_spans(&parsed, &read) {
            failures.push((
                case.name.clone(),
                format!(
                    "the trees differ\n--- parsed\n{}\n--- read\n{}",
                    usfm::codegen::to_usfm_string(&parsed),
                    usfm::codegen::to_usfm_string(&read)
                ),
            ));
        }
    }
    assert_none("read(reference) is parse(usfm)", failures);
}

/// The span invariants `usfm_parser::span_check` states for a parse, as
/// they read for a tree read from XML: `span_check::read_violations`, which
/// `tasks/fuzz`'s `read_usx` target asserts over arbitrary bytes too.
#[test]
fn every_read_node_has_a_span_in_its_source() {
    let mut failures = Vec::new();
    for case in &references() {
        let text = case.expected_usx_text().unwrap();
        let result = read_usx(&text);
        for violation in read_violations(&text, &result.document) {
            failures.push((case.name.clone(), violation));
        }
    }
    assert_none("every read node has a span in its source", failures);
}

/// An attribute name USFM does not allow but XML does (`x�-morph`, U+FFFD
/// being an XML name character) is kept, as the parser keeps it, for the
/// semantic pass to report as `malformed-attribute-name`, and its span is its
/// name all the same. `span_check` had taken "an attribute's span" to mean
/// "a valid name" (ticket 49, `tasks/fuzz`'s `read_usx`).
#[test]
fn a_malformed_attribute_name_keeps_its_span() {
    let usx = "<usx version=\"3.0\"><book code=\"GEN\" style=\"id\"/><para style=\"p\">\
               <char style=\"w\" x\u{fffd}-morph=\"a\">word</char></para></usx>";
    let result = usfm::parse_usx(usx);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == usfm::Code::MalformedAttributeName),
        "{:?}",
        result.diagnostics
    );
    assert_eq!(read_violations(usx, &result.document), Vec::<String>::new());
}

/// Every `<verse eid>` and `<chapter eid>` in `usx`, removed: the USX 2 form
/// of a USX 3 file, which the writer does not write (spec, M7: out of scope).
/// (`<ms eid>` is a milestone's own attribute, not an end, and stays.)
fn strip_eids(usx: &str) -> String {
    END_MILESTONE.replace_all(usx, "").into_owned()
}

/// A self-closing `<verse>` or `<chapter>` with an `eid`.
static END_MILESTONE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"<(?:verse|chapter)\s[^>]*\beid="[^"]*"[^>]*/>"#).unwrap());

/// Any `<verse>` or `<chapter>` start tag with an `eid`, self-closing or not:
/// what [`strip_eids`] must leave none of.
static ANY_END: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<(?:verse|chapter)\s[^>]*\beid=").unwrap());

/// The references property 5 does not hold for, each with the reason. A
/// listed case that holds is a failure too, as in the gate's known lists.
///
/// All `fail` cases the harness never compares: what differs is where the
/// reference put an `eid`, not where the reader builds one.
const STRIPPED_EIDS_KNOWN: &[(&str, &str)] = &[(
    "usfmjsTests/invalid",
    "the reference ends `\\v 11` before `<verse number=\"No\" status=\"invalid\">`, a verse \
     the reader drops as `malformed-verse-number` as the parser drops `\\v No`; without the \
     `eid`, verse 11 runs over the dropped verse's text, which is where the parser ends it",
)];

#[test]
fn a_reference_reads_the_same_without_its_eids() {
    let mut failures = Vec::new();
    let mut stripped_any = 0;
    for case in &references() {
        let known = STRIPPED_EIDS_KNOWN
            .iter()
            .any(|(name, _)| *name == case.name);
        let text = case.expected_usx_text().unwrap();
        let stripped = strip_eids(&text);
        assert!(
            !ANY_END.is_match(&stripped),
            "{}: an eid survived",
            case.name
        );
        if stripped.len() != text.len() {
            stripped_any += 1;
        }
        let with = read_usx(&text).document;
        let without = read_usx(&stripped).document;
        let holds = eq_ignoring_spans(&with, &without);
        if known {
            if holds {
                failures.push((case.name.clone(), "listed as known, but holds".into()));
            }
            continue;
        }
        if !holds {
            failures.push((
                case.name.clone(),
                format!(
                    "the trees differ\n--- with eids\n{}\n--- without\n{}",
                    to_usx_string(&with),
                    to_usx_string(&without)
                ),
            ));
        }
    }
    // Every reference but the few with no verse at all, and
    // `advanced/complex`, which is USX 2 already.
    assert!(
        stripped_any >= 260,
        "only {stripped_any} references had an eid"
    );
    assert_none("read(strip_eids(reference)) is read(reference)", failures);
}

/// A USX 2 file and its USFM: the ends the reader builds are the parser's,
/// for the placements `crates/usfm_parser/tests/verse_ends.rs` pins — before
/// the next verse in the paragraph, past a heading, before a character style
/// a verse starts in, into the previous cell or before a table, around a
/// sidebar and not inside it, and at a chapter.
#[test]
fn a_usx_2_file_is_the_parse_of_its_usfm() {
    let usx = r#"<usx version="2.6"><book code="GEN" style="id"/><chapter number="1" style="c"/><para style="p"><verse number="1" style="v"/>one <verse number="2" style="v"/>two</para><para style="s">Heading</para><para style="p"><char style="add"><verse number="3" style="v"/>three</char></para><sidebar style="esb"><para style="p"><verse number="9" style="v"/>aside</para></sidebar><para style="q1">still three</para><table><row style="tr"><cell style="tc1"><verse number="4" style="v"/>four</cell><cell style="tc2"><verse number="5" style="v"/>five</cell></row></table><chapter number="2" style="c"/><para style="p"><verse number="1" style="v"/>last</para></usx>"#;
    let usfm = "\\id GEN\n\\usfm 2.6\n\\c 1\n\\p \\v 1 one \\v 2 two\n\\s Heading\n\\p \\add \\v 3 three\\add*\n\\esb\n\\p \\v 9 aside\n\\esbe\n\\q1 still three\n\\tr \\tc1 \\v 4 four\\tc2 \\v 5 five\n\\c 2\n\\p \\v 1 last\n";
    assert_usx_2_reads_as(usx, usfm);
    // A verse that starts inside a character style of a paragraph that is not
    // verse text ends in that paragraph, which starts it (ticket 49: the
    // parser and the reader both looked only at the paragraph's own children,
    // and ended it in the paragraph before, ahead of its own start).
    assert_usx_2_reads_as(
        r#"<usx version="2.6"><book code="GEN" style="id"/><chapter number="1" style="c"/><para style="p"><verse number="1" style="v"/>a</para><para style="ip"><char style="w">p <verse number="2" style="v"/>b</char></para></usx>"#,
        "\\id GEN\n\\usfm 2.6\n\\c 1\n\\p \\v 1 a\n\\ip \\w p \\v 2 b\\w*\n",
    );
    // Two ends waiting at once — verse 8's for block level, verse 9's for the
    // head of the `\w` — and the second no longer replaces the first
    // (ticket 49, in the parser and the reader alike).
    assert_usx_2_reads_as(
        r#"<usx version="2.6"><book code="GEN" style="id"/><chapter number="1" style="c"/><para style="p"><verse number="8" style="v"/>a</para><para style="p"><verse number="9" style="v"/><char style="w"><verse number="10" style="v"/>b</char></para></usx>"#,
        "\\id GEN\n\\usfm 2.6\n\\c 1\n\\p \\v 8 a\n\\p \\v 9 \\w \\v 10 b\\w*\n",
    );
}

fn assert_usx_2_reads_as(usx: &str, usfm: &str) {
    let read = read_usx(usx);
    assert!(read.diagnostics.is_empty(), "{:?}", read.diagnostics);
    let parsed = usfm::parse(usfm).document;
    assert!(
        eq_ignoring_spans(&read.document, &parsed),
        "--- read\n{}\n--- parsed\n{}",
        to_usx_string(&read.document),
        to_usx_string(&parsed)
    );
}

/// machine.py's World English Bible books (`fixtures/machine-py/usx/`).
fn web_books() -> Vec<(String, String)> {
    ["1JN", "2JN", "3JN"]
        .iter()
        .map(|book| {
            let path = format!(
                "{}/fixtures/machine-py/usx/WEB-DBL/{book}.usx",
                env!("CARGO_MANIFEST_DIR")
            );
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|err| panic!("reading {path}: {err}"));
            (format!("WEB-DBL/{book}"), text)
        })
        .collect()
}

#[test]
fn the_web_books_round_trip() {
    let mut failures = Vec::new();
    for (name, text) in web_books() {
        let read = read_usx(&text);
        let errors: Vec<String> = read
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Error)
            .map(ToString::to_string)
            .collect();
        if !errors.is_empty() {
            failures.push((name.clone(), errors.join("; ")));
        }
        // USX -> USFM -> the tree again, whitespace and all.
        let usfm = usfm::codegen::to_usfm_string(&read.document);
        let parsed = usfm::parse(&usfm);
        if !eq_ignoring_spans(&read.document, &parsed.document) {
            failures.push((
                name.clone(),
                format!("parse(codegen(read)) differs\n{usfm}"),
            ));
        }
        // -> USX, against the file. DBL indents inside mixed content, as
        // usfm-grammar's generator does, so whitespace is compared the way
        // the harness compares that root; the tree comparison above is the
        // exact one.
        let options = UsxOptions {
            include_vid: text.contains("vid="),
        };
        let mut actual = to_usx_node_with_options(&parsed.document, options);
        let mut expected = TestCase::parse_usx_text(&text).unwrap();
        for node in [&mut actual, &mut expected] {
            normalize_tree(node);
            collapse_whitespace_tree(node);
        }
        if let Err(mismatch) = compare_xml(&actual, &expected) {
            failures.push((
                name.clone(),
                format!("USX -> USFM -> USX differs: {mismatch}"),
            ));
        }
        // And without its `eid`s, the same tree.
        if !eq_ignoring_spans(&read.document, &read_usx(&strip_eids(&text)).document) {
            failures.push((name, "reads differently without its eids".into()));
        }
    }
    assert_none("the WEB books", failures);
}

/// 4. **Whitespace is not compared on the usfm-grammar root**, as the harness
///    does not compare it there (`collapse_whitespace_tree`): that generator
///    copies its source's line breaks into the USX and indents its elements
///    inside mixed content, so `\xo 1:1 \xo* \xt Matt 1:1\xt*` comes out
///    as `<char style="xo">1:1 </char>\n      <char style="xt">` and the
///    space the source had between them is indistinguishable from the
///    indentation (`usfm-grammar/bugfixes/marker-ex`,
///    `usfm-grammar/bugfixes/custom-attrib-hyphens`). Every text run is
///    collapsed and trimmed at both ends, and a run left empty is dropped.
struct CollapseWhitespace;

impl CollapseWhitespace {
    fn children(&mut self, children: &mut Vec<Inline<'_>>) {
        for child in children.iter_mut() {
            self.visit_inline(child);
        }
        children.retain(|child| !matches!(child, Inline::Text(text) if text.is_empty()));
    }
}

impl VisitMut for CollapseWhitespace {
    fn visit_text(&mut self, text: &mut Text<'_>) {
        let collapsed = text.split_ascii_whitespace().collect::<Vec<_>>().join(" ");
        text.content = Cow::Owned(collapsed);
    }

    fn visit_para(&mut self, para: &mut Para<'_>) {
        self.children(&mut para.children);
    }

    fn visit_char(&mut self, char: &mut Char<'_>) {
        self.children(&mut char.children);
    }

    fn visit_note(&mut self, note: &mut Note<'_>) {
        self.children(&mut note.children);
    }

    fn visit_table_cell(&mut self, cell: &mut TableCell<'_>) {
        self.children(&mut cell.children);
    }
}
