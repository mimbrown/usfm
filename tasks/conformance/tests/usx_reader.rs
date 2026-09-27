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
//!    `eq_ignoring_spans`, after [`Normalise`] — the list of what a USX file
//!    cannot say, measured over the corpus rather than guessed. Four entries,
//!    each naming a case that shows it. Two the spec expected are not on it,
//!    because the tree never carried them or no compared case has them:
//!    whether a `\+` was written is not in the tree to begin with, and no
//!    compared case writes an empty `|` list (`\ts-s |\*`, which the reader
//!    reads as no list, `crates/usfm_usx/tests/reader.rs`).
//! 4. [`every_read_node_has_a_span_in_its_source`][]: `span_check`'s
//!    invariants, as they read for XML, over every reference.
//!
//! The gate's `cargo test --workspace` excludes this package (its generated
//! tests are the harness's own, which `cargo run` already runs), so
//! `scripts/gate.sh` runs this file by name.

use std::borrow::Cow;
use std::panic::{self, AssertUnwindSafe};
use std::sync::Arc;

use usfm::ast::visit_mut::VisitMut;
use usfm::ast::{
    Attributes, Block, Char, Document, Inline, Milestone, Note, Para, Periph, SPAN, StyleId,
    TableCell, Text, default_attribute_name, eq_ignoring_spans, is_valid_attribute_name,
};
use usfm::diagnostics::Severity;
use usfm::parser::span_check::{NodeSpan, Prefix, nodes};
use usfm::style::StyleSheet;
use usfm::usx::{DEFAULT_USX_VERSION, read_usx};
use usfm::{DEFAULT_STYLESHEET, parse_with_options};
use usfm_tests::{TestCase, ValidationStatus, discover_tests, usx_properties};

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
        let mut parsed = parse_with_options(
            &usfm,
            &DEFAULT_STYLESHEET,
            case.expected_has_end_milestones(),
        )
        .document;
        let text = case.expected_usx_text().unwrap();
        let mut read = read_usx(&text).document;
        Normalise::apply(&mut parsed);
        Normalise::apply(&mut read);
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
/// they read for a tree read from XML: every span in bounds, not inverted and
/// on a character boundary; every node but a `Text` starting at the element
/// it was read from (`<`) or at the attribute that stands for its marker
/// (`version=` for the `\usfm` paragraph, `category=`, `alt=`), or, for an
/// implicit `\p`, at or before its content; every attribute's at its name. The one invariant a read tree
/// cannot hold is the marker the span starts with: USX has elements, not
/// `\p`, and an attribute list has no `|` (its `pipe` is `SPAN`).
#[test]
fn every_read_node_has_a_span_in_its_source() {
    let mut failures = Vec::new();
    for case in &references() {
        let text = case.expected_usx_text().unwrap();
        let result = read_usx(&text);
        for NodeSpan {
            label,
            span,
            prefix,
        } in nodes(&result.document)
        {
            let fail = |why: String| (case.name.clone(), format!("{label} {span:?}: {why}"));
            if span.start > span.end || span.end as usize > text.len() {
                failures.push(fail("out of bounds or inverted".into()));
                continue;
            }
            if span == SPAN {
                continue;
            }
            let (start, end) = (span.start as usize, span.end as usize);
            if !text.is_char_boundary(start) || !text.is_char_boundary(end) {
                failures.push(fail("splits a character".into()));
                continue;
            }
            let slice = &text[start..end];
            // What a node was read from: an element, or the attribute that
            // stands for a marker (`version=`, `category=`, `alt=`).
            let element_or_attribute = slice.starts_with('<')
                || slice
                    .split_once('=')
                    .is_some_and(|(name, _)| is_valid_attribute_name(name));
            let holds = match prefix {
                Prefix::Anything => true,
                Prefix::Exact(_) => false,
                // A marker (`\\p`, `//`): the element or attribute it became.
                Prefix::Marker(marker) if !is_valid_attribute_name(&marker) => element_or_attribute,
                // An attribute of a list: its name.
                Prefix::Marker(_) => is_valid_attribute_name(slice),
                Prefix::MarkerOrImplicit { content_start, .. } => {
                    element_or_attribute
                        || content_start.is_some_and(|content| span.start <= content)
                }
            };
            if !holds {
                failures.push(fail(format!("starts {:?}", &slice[..slice.len().min(20)])));
            }
        }
    }
    assert_none("every read node has a span in its source", failures);
}

/// What USX cannot say, applied to both trees before they are compared:
/// each rewrite puts a construct into the one form USX has for it, which is
/// the form the reader builds. Measured over the corpus (ticket 45); each
/// entry names the case that showed it.
struct Normalise {
    style_sheet: Arc<StyleSheet>,
}

impl Normalise {
    fn apply(document: &mut Document<'_>) {
        let mut normalise = Normalise {
            style_sheet: Arc::clone(document.style_sheet()),
        };
        normalise.visit_document(document);
        declare_version(document);
    }

    fn marker(&self, style: StyleId) -> &str {
        &self.style_sheet.get_rule(style.index()).marker
    }

    /// 1. **The default attribute has its name.** `\w a|b\w*` and
    ///    `\w a|lemma="b"\w*` are one `<char lemma="b">`; the reader cannot
    ///    tell them apart and names every attribute
    ///    (`specExamples/cross-ref`: `\ref 1|GEN 2:1\ref*` is `loc`;
    ///    `usfm-grammar/bugfixes/attrib-for-tl`: `\tl …|es\tl*` is `lang`).
    fn name_default_attribute(&self, attributes: Option<&mut Attributes<'_>>, marker: &str) {
        let Some(attributes) = attributes else {
            return;
        };
        for pair in &mut attributes.pairs {
            if pair.name.is_empty()
                && let Some(name) = default_attribute_name(marker)
            {
                pair.name = Cow::Borrowed(name);
            }
        }
    }
}

impl VisitMut for Normalise {
    /// 3. **A note's trailing whitespace is not compared**, for the reason
    ///    the harness gives in `normalize_tree`: Paratext writes `\ft text \f*`
    ///    as `text </char></note>` in one reference and `text </char> </note>`
    ///    in another (`usfmjsTests/isa_inline_quotes`), and the reader reads
    ///    each as written. A cell's end is the same rule, which the reader
    ///    already applies (rule 5), so only a note needs it here.
    fn visit_note(&mut self, note: &mut Note<'_>) {
        for child in &mut note.children {
            self.visit_inline(child);
        }
        trim_trailing_text(&mut note.children);
    }

    fn visit_char(&mut self, char: &mut Char<'_>) {
        let marker = self.marker(char.style).to_string();
        self.name_default_attribute(char.attributes.as_mut(), &marker);
        for child in &mut char.children {
            self.visit_inline(child);
        }
    }

    fn visit_milestone(&mut self, milestone: &mut Milestone<'_>) {
        let marker = self.marker(milestone.style).to_string();
        self.name_default_attribute(milestone.attributes.as_mut(), &marker);
    }

    fn visit_periph(&mut self, periph: &mut Periph<'_>) {
        self.name_default_attribute(periph.attributes.as_mut(), "periph");
        for block in &mut periph.blocks {
            self.visit_block(block);
        }
    }
}

/// Trim ASCII whitespace from the end of the last text in `children`,
/// descending into a trailing character style and dropping what becomes
/// empty: the harness's `trim_trailing_text`, over the tree.
fn trim_trailing_text(children: &mut Vec<Inline<'_>>) {
    while let Some(last) = children.last_mut() {
        match last {
            Inline::Text(text) => {
                let trimmed = text.trim_end_matches(|c: char| c.is_ascii_whitespace());
                if trimmed.is_empty() {
                    children.pop();
                    continue;
                }
                text.content = Cow::Owned(trimmed.to_string());
            }
            Inline::Char(char) => trim_trailing_text(&mut char.children),
            _ => {}
        }
        return;
    }
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

/// 2. **A version is declared.** `<usx version>` is always there, and the
///    reader reads it as the `\usfm` paragraph; a USFM file without one is
///    written with [`DEFAULT_USX_VERSION`], so it reads as if it had declared
///    that, right after its `\id` (`advanced/complex`, and most of tcdocs).
fn declare_version(document: &mut Document<'_>) {
    if document.usfm_version().is_some() {
        return;
    }
    let Some(&usfm) = document.style_sheet().get_marker_index("usfm") else {
        return;
    };
    let para = Block::Para(Para {
        style: StyleId::new(usfm as u32),
        children: vec![Inline::Text(Text::synthesized(DEFAULT_USX_VERSION))],
        span: SPAN,
    });
    let at = usize::from(matches!(document.blocks.first(), Some(Block::Book(_))));
    document.blocks.insert(at, para);
}
