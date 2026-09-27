//! The two USX properties of M7 (ticket 46), in one place:
//!
//! 1. **Read** ([`check_read`]): read a case's reference USX with
//!    `usfm_usx::read_usx`, write the tree back with `usfm_usx`, and the
//!    result is the reference under the harness's own comparison.
//! 2. **Round trip** ([`check_roundtrip`]): read the reference, write it as
//!    USFM with `usfm_codegen`, parse that with `usfm::parse`, write USX, and
//!    the result is the reference the same way.
//!
//! Both compare on the USX side, with [`TestCase::normalize_for_comparison`]
//! and [`compare_xml`] — the reference patched as the harness patches it, its
//! whitespace compared as the harness compares it — so nothing the tree does
//! not carry has to be excused here: the canonical spelling `usfm_codegen`
//! writes (a named default attribute, an explicit closer, a `+` on a nested
//! style) is invisible once it is USX again. The tree-level comparison with
//! `usfm::parse` of `origin.usfm`, which does need such a list, is the
//! `usx_reader` test's.
//!
//! **The cases** are the ones the harness compares today
//! ([`compared_cases`]): a case with an input and a reference whose parse
//! matched that reference. Those are the references the harness vouches for;
//! a `fail` case whose parse reported an Error was never compared with its
//! reference, which may hold an `<unmatched>` or a reference implementation's
//! bug, and so says nothing about the reader.
//!
//! The runner's `--usx-read` and `--usx-roundtrip` gate these against their
//! known lists the way `--roundtrip` gates [`crate::roundtrip`].

use usfm::ast::visit_mut::{VisitMut, walk_document, walk_periph, walk_sidebar};
use usfm::ast::{Block, Document, Inline, Para, Periph, Sidebar, TableCell};
use usfm::codegen::to_usfm_string;
use usfm::usx::{UsxOptions, XmlNode, read_usx, to_usx_node_with_options};
use usfm::{DEFAULT_STYLESHEET, parse_with_options};

use crate::{TestCase, TestResult, compare_xml};

/// One case that did not hold a property: its name and the report.
pub struct UsxFailure {
    pub name: String,
    pub reason: String,
}

/// The cases the harness compares today and passes: an input, a reference,
/// and the parser's output matched it ([`TestCase::run`] is `Passed`).
pub fn compared_cases(tests: &[TestCase]) -> Vec<&TestCase> {
    tests
        .iter()
        .filter(|case| case.has_usfm() && case.has_expected_usx())
        .filter(|case| matches!(case.run(), TestResult::Passed))
        .collect()
}

/// Write `document` as USX and compare it with `case`'s reference the way the
/// harness does. `include_vid` follows the reference, as the harness's own
/// run does: a USX 3.0 file without `vid` is not wrong, just older.
fn compare_with_reference(case: &TestCase, document: &usfm::Document<'_>) -> Result<(), String> {
    let options = UsxOptions {
        include_vid: case.expected_has_vid(),
    };
    let mut actual: XmlNode = to_usx_node_with_options(document, options);
    let mut expected = case
        .read_expected_usx()
        .map_err(|err| format!("cannot read the reference: {err}"))?;
    case.normalize_for_comparison(&mut actual);
    case.normalize_for_comparison(&mut expected);
    compare_xml(&actual, &expected).map_err(|mismatch| mismatch.to_string())
}

/// Property 1: read the reference and write it back.
///
/// The reader builds the verse and chapter ends a reference without `eid`s
/// leaves out (ticket 47), as a parse with ends would; the harness parses
/// such a case without them, and so the ends are dropped here before the
/// tree is written, the way `include_vid` follows the reference.
pub fn check_read(case: &TestCase) -> Result<(), String> {
    let text = case
        .expected_usx_text()
        .map_err(|err| format!("cannot read the reference: {err}"))?;
    let mut read = read_usx(&text);
    if !case.expected_has_end_milestones() {
        DropEnds.visit_document(&mut read.document);
    }
    compare_with_reference(case, &read.document)
        .map_err(|mismatch| format!("write(read(reference)) differs: {mismatch}"))
}

/// Property 2: read the reference, write USFM, parse it, write USX. The parse
/// builds verse and chapter ends when the reference has them, as the
/// harness's run of the case does.
pub fn check_roundtrip(case: &TestCase) -> Result<(), String> {
    let text = case
        .expected_usx_text()
        .map_err(|err| format!("cannot read the reference: {err}"))?;
    let read = read_usx(&text);
    let usfm = to_usfm_string(&read.document);
    let parsed = parse_with_options(
        &usfm,
        &DEFAULT_STYLESHEET,
        case.expected_has_end_milestones(),
    );
    compare_with_reference(case, &parsed.document).map_err(|mismatch| {
        format!("write(parse(codegen(read(reference)))) differs: {mismatch}\n--- USFM ---\n{usfm}")
    })
}

/// Every `VerseEnd` and `ChapterEnd` in a tree, removed: what a parse
/// without end milestones would have built.
struct DropEnds;

impl DropEnds {
    fn blocks(blocks: &mut Vec<Block<'_>>) {
        blocks.retain(|block| !matches!(block, Block::ChapterEnd(_)));
    }

    /// A built end stands between the text before it and the space it took
    /// from that text (`text<eid/> <v/>`); without it, the two are one run
    /// again, as a parse without ends reads them.
    fn inlines(children: &mut Vec<Inline<'_>>) {
        let mut kept: Vec<Inline<'_>> = Vec::with_capacity(children.len());
        for inline in children.drain(..) {
            match (kept.last_mut(), inline) {
                (_, Inline::VerseEnd(_)) => {}
                (Some(Inline::Text(before)), Inline::Text(text)) => {
                    before.content.to_mut().push_str(&text.content);
                }
                (_, inline) => kept.push(inline),
            }
        }
        *children = kept;
    }
}

impl VisitMut for DropEnds {
    fn visit_document(&mut self, document: &mut Document<'_>) {
        Self::blocks(&mut document.blocks);
        walk_document(self, document);
    }

    fn visit_sidebar(&mut self, sidebar: &mut Sidebar<'_>) {
        Self::blocks(&mut sidebar.blocks);
        walk_sidebar(self, sidebar);
    }

    fn visit_periph(&mut self, periph: &mut Periph<'_>) {
        Self::blocks(&mut periph.blocks);
        walk_periph(self, periph);
    }

    // An end is only ever a paragraph's or a cell's child.
    fn visit_para(&mut self, para: &mut Para<'_>) {
        Self::inlines(&mut para.children);
    }

    fn visit_table_cell(&mut self, cell: &mut TableCell<'_>) {
        Self::inlines(&mut cell.children);
    }
}

/// Run `property` over [`compared_cases`], in discovery order. Returns the
/// number of cases run and the failures among them.
pub fn run(
    tests: &[TestCase],
    property: fn(&TestCase) -> Result<(), String>,
) -> (usize, Vec<UsxFailure>) {
    let cases = compared_cases(tests);
    let failures = cases
        .iter()
        .filter_map(|case| {
            property(case).err().map(|reason| UsxFailure {
                name: case.name.clone(),
                reason,
            })
        })
        .collect();
    (cases.len(), failures)
}
