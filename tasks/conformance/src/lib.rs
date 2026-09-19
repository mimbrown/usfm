//! USFM Parser Test Suite
//!
//! This crate provides a test harness for validating the USFM parser against
//! the official tcdocs test suite from usfm-bible/tcdocs, and against the
//! usfm-grammar regression fixtures vendored under `tasks/conformance/fixtures/`. Both are
//! discovered as [`ROOTS`] and gated by the one baseline file.
//!
//! # Test Structure
//!
//! Each test case consists of:
//! - `origin.usfm` - The input USFM file
//! - `origin.xml` - The expected USX output
//! - `metadata.xml` - Test metadata (description, pass/fail expectation, tags)
//!
//! # Semantics
//!
//! What a case asserts depends on its `<validated>` verdict and on whether it
//! ships a reference USX ([`TestCase::run`] implements this):
//!
//! - `pass` (or unmarked) with an `origin.xml`: the output must match it and
//!   no error diagnostic may be reported.
//! - `fail` with an `origin.xml`: reporting an error is an expected failure,
//!   and so is matching the reference with no error; reporting nothing *and*
//!   not matching is an unexpected pass, which fails the run.
//! - `pass` with no `origin.xml` (three of the usfm-grammar `bugfixes` cases):
//!   the input must parse with no error diagnostics. There is nothing to
//!   compare, so that is the whole assertion.
//! - `fail` with no `origin.xml`: undecidable once no error was reported, and
//!   the only shape the harness skips.
//!
//! # Running Tests
//!
//! ```bash
//! # Run all tests
//! cargo test --package usfm_tests
//!
//! # Run specific test
//! cargo test --package usfm_tests test_basic_minimal
//!
//! # Run all tests in a category
//! cargo test --package usfm_tests test_basic_
//!
//! # Run with output
//! cargo test --package usfm_tests -- --nocapture
//! ```

use std::fmt::Display;
use std::fs::{File, read_to_string};
use std::io::BufReader;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use usfm::diagnostics::Diagnostic;
use usfm::{DEFAULT_STYLESHEET, parse_with_options};
use usfm::usx::{UsxOptions, XmlDocument, XmlElement, XmlNode, to_usx_node_with_options};
use xml::reader::{EventReader, XmlEvent};

/// Root path to the tcdocs test suite
pub const TCDOCS_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tcdocs/tests");

/// Root path to the vendored usfm-grammar fixtures. Its cases live one
/// directory deeper (`bugfixes/<case>`) so that the directory under this root
/// names the category, as the top-level directories do under `TCDOCS_ROOT`.
pub const USFM_GRAMMAR_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/usfm-grammar");

/// The roots the harness discovers cases under, each with the prefix its test
/// names carry. tcdocs cases keep their bare `<category>/<case>` names, so a
/// baseline entry, a `--show` argument and a patch path all keep meaning what
/// they meant when tcdocs was the only root.
pub const ROOTS: &[(&str, &str)] = &[(TCDOCS_ROOT, ""), (USFM_GRAMMAR_ROOT, "usfm-grammar")];

/// Patches to the reference USX, one unified diff per test at
/// `<test name>.patch`. See the README in that directory for the rules.
pub const PATCH_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tcdocs-patches");

/// Test metadata parsed from metadata.xml
#[derive(Debug, Default, Clone)]
pub struct TestMetadata {
    pub description: String,
    pub validated: ValidationStatus,
    pub tags: Vec<String>,
}

/// Whether a test is expected to pass or fail
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ValidationStatus {
    Pass,
    Fail,
    #[default]
    Unknown,
}

impl From<&str> for ValidationStatus {
    fn from(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "pass" => ValidationStatus::Pass,
            "fail" => ValidationStatus::Fail,
            _ => ValidationStatus::Unknown,
        }
    }
}

/// A single test case from one of the [`ROOTS`]
#[derive(Debug, Clone)]
pub struct TestCase {
    /// Unique identifier derived from path (e.g., "basic/minimal" or
    /// "usfm-grammar/bugfixes/q4")
    pub name: String,
    /// The category this case is reported under: the directory below its
    /// root, with the root's prefix (e.g. "basic", "usfm-grammar/bugfixes")
    pub category: String,
    /// Full path to the test directory
    pub path: PathBuf,
    /// Test metadata
    pub metadata: TestMetadata,
}

/// The name a case at `path` carries, and the category it belongs to, taken
/// from whichever root contains it. A path under no root keeps its own text as
/// its name and its first component as its category.
fn name_and_category(path: &Path) -> (String, String) {
    let (relative, prefix) = ROOTS
        .iter()
        .find_map(|(root, prefix)| Some((path.strip_prefix(root).ok()?, *prefix)))
        .unwrap_or((path, ""));
    let relative = relative
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/");
    let head = relative.split('/').next().unwrap_or(&relative);
    let join = |tail: &str| {
        if prefix.is_empty() {
            tail.to_string()
        } else {
            format!("{prefix}/{tail}")
        }
    };
    (join(&relative), join(head))
}

impl TestCase {
    /// Load a test case from a directory
    pub fn load(path: &Path) -> Result<Self, TestError> {
        let metadata_path = path.join("metadata.xml");
        let metadata = if metadata_path.exists() {
            read_metadata(&metadata_path)?
        } else {
            TestMetadata::default()
        };

        let (name, category) = name_and_category(path);

        Ok(TestCase {
            name,
            category,
            path: path.to_path_buf(),
            metadata,
        })
    }

    /// Check if this test has input USFM
    pub fn has_usfm(&self) -> bool {
        self.usfm_path().exists()
    }

    /// Check if this test has expected USX output
    pub fn has_expected_usx(&self) -> bool {
        self.usx_path().exists()
    }

    /// Path to the input USFM file
    pub fn usfm_path(&self) -> PathBuf {
        self.path.join("origin.usfm")
    }

    /// Path to the expected USX output
    pub fn usx_path(&self) -> PathBuf {
        self.path.join("origin.xml")
    }

    /// Read the input USFM
    pub fn read_usfm(&self) -> Result<String, TestError> {
        read_to_string(self.usfm_path()).map_err(TestError::Io)
    }

    /// Path of the patch applied to this test's reference USX, whether or
    /// not one exists: `tasks/conformance/tcdocs-patches/<name>.patch`.
    pub fn patch_path(&self) -> PathBuf {
        Path::new(PATCH_ROOT).join(format!("{}.patch", self.name))
    }

    /// Whether the reference USX is read through a patch.
    pub fn has_patch(&self) -> bool {
        self.patch_path().exists()
    }

    /// The reference USX as text, with the byte-order mark and CR line
    /// endings some files carry removed, so patches are written against
    /// the same text on every platform.
    fn reference_text(&self) -> Result<String, TestError> {
        let content = read_to_string(self.usx_path()).map_err(TestError::Io)?;
        Ok(content.trim_start_matches('\u{feff}').replace("\r\n", "\n"))
    }

    /// usfm-grammar writes `<usx version>` truncated to `major.minor`, so a
    /// book whose `\usfm` line says `3.1.2` gets `version="3.1"` in its
    /// reference file. We write the declared version through, which is what
    /// the tcdocs files show for `\usfm 3.1` and what `usx.rnc` allows
    /// (`\d+\.\d+(\.\d+)?`). Put the dropped component back, and only that:
    /// a reference version that is not a prefix of the declared one is left
    /// alone and still has to match.
    fn restore_usx_version(&self, text: String) -> String {
        if !self.path.starts_with(USFM_GRAMMAR_ROOT) {
            return text;
        }
        let Some(declared) = self
            .read_usfm()
            .ok()
            .and_then(|usfm| usfm_version(&usfm).map(String::from))
        else {
            return text;
        };
        let Some((major_minor, _)) = declared.rsplit_once('.') else {
            return text;
        };
        text.replace(
            &format!("<usx version=\"{major_minor}\""),
            &format!("<usx version=\"{declared}\""),
        )
    }

    /// The reference USX with this test's patch applied, if there is one.
    fn patched_reference_text(&self) -> Result<String, TestError> {
        let text = self.reference_text()?;
        if !self.has_patch() {
            return Ok(text);
        }
        let path = self.patch_path();
        let patch_text = read_to_string(&path).map_err(TestError::Io)?;
        let patch = diffy::Patch::from_str(&patch_text)
            .map_err(|e| TestError::Patch(format!("{} does not parse: {e}", path.display())))?;
        diffy::apply(&text, &patch).map_err(|e| {
            TestError::Patch(format!(
                "{} does not apply to {}: {e}",
                path.display(),
                self.usx_path().display()
            ))
        })
    }

    fn parse_usx_text(text: &str) -> Result<XmlNode, TestError> {
        let normalized = normalize_usx(text);
        let doc =
            XmlDocument::from(BufReader::new(normalized.as_bytes())).map_err(TestError::Xml)?;
        Ok(XmlNode::Element(doc.root))
    }

    /// Read and parse the expected USX: the reference file, patched if a
    /// patch exists for this test. The version is restored after the patch,
    /// so a patch is written against the file's own text.
    pub fn read_expected_usx(&self) -> Result<XmlNode, TestError> {
        let text = self.restore_usx_version(self.patched_reference_text()?);
        Self::parse_usx_text(&text)
    }

    /// Read and parse the reference USX as the file has it, ignoring any
    /// patch.
    pub fn read_unpatched_usx(&self) -> Result<XmlNode, TestError> {
        let text = self.restore_usx_version(self.reference_text()?);
        Self::parse_usx_text(&text)
    }

    /// Check if expected USX contains end milestones (eid= attributes)
    pub fn expected_has_end_milestones(&self) -> bool {
        if let Ok(content) = read_to_string(self.usx_path()) {
            content.contains("eid=")
        } else {
            true // Default to inserting milestones if we can't read expected
        }
    }

    /// Check if expected USX contains vid attributes
    pub fn expected_has_vid(&self) -> bool {
        if let Ok(content) = read_to_string(self.usx_path()) {
            content.contains("vid=")
        } else {
            true // Default to including vid if we can't read expected
        }
    }

    /// Run the parser and return the USX output and diagnostics
    pub fn parse_to_usx(&self) -> Result<(XmlNode, Vec<Diagnostic>), TestError> {
        self.parse_to_usx_with_options(true, true)
    }

    /// Run the parser with configurable options
    pub fn parse_to_usx_with_options(
        &self,
        insert_end_milestones: bool,
        include_vid: bool,
    ) -> Result<(XmlNode, Vec<Diagnostic>), TestError> {
        let usfm = self.read_usfm()?;
        // Through the facade, so the harness sees what a caller sees: the
        // parser's diagnostics *and* `usfm_semantic`'s (ticket 19). A `pass`
        // case is judged on error diagnostics, so a semantic Warning changes
        // no verdict; what it does mean is that every tcdocs input runs the
        // semantic checks on every run.
        let result = parse_with_options(&usfm, &DEFAULT_STYLESHEET, insert_end_milestones);
        // The serializer resolves styles against the document's own
        // stylesheet, which is the base sheet plus anything the parser derived
        // (hardening plan D3); `include_vid` is the one thing a caller varies.
        let usx = to_usx_node_with_options(&result.document, UsxOptions { include_vid });
        Ok((usx, result.diagnostics))
    }

    /// Put a tree into the form the comparison works on: [`normalize_tree`]
    /// for every case, plus [`collapse_whitespace_tree`] for the usfm-grammar
    /// root, whose reference files do not normalise whitespace at all.
    ///
    /// usfm-grammar copies the source text into USX verbatim, so a line break
    /// inside a paragraph stays a line break and the newline before the next
    /// marker stays in the text (`q4/origin.xml` keeps it after verse 33 and
    /// loses it after verse 34 only because the file ends there). Our rules 1-6
    /// on `Text` turn that run into one space and drop it at a paragraph-level
    /// boundary; they are pinned against the tcdocs files, which agree with
    /// them, and tested in `crates/usfm_parser/tests/whitespace.rs`. So for this root
    /// whitespace is not what is under test: the markers, attributes and
    /// structure are.
    pub fn normalize_for_comparison(&self, node: &mut XmlNode) {
        normalize_tree(node);
        if self.path.starts_with(USFM_GRAMMAR_ROOT) {
            collapse_whitespace_tree(node);
        }
    }

    /// Run the test and compare output
    pub fn run(&self) -> TestResult {
        // Skip tests without USFM input
        if !self.has_usfm() {
            return TestResult::Skipped {
                reason: "No origin.usfm file".to_string(),
            };
        }

        // Check if expected output has end milestones and vid - if not, skip them
        let insert_end_milestones = self.expected_has_end_milestones();
        let include_vid = self.expected_has_vid();

        // Parse the USFM, catching panics
        let parse_result = panic::catch_unwind(AssertUnwindSafe(|| {
            self.parse_to_usx_with_options(insert_end_milestones, include_vid)
        }));

        let (mut actual, diagnostics) = match parse_result {
            Ok(Ok(parsed)) => parsed,
            Ok(Err(e)) => {
                return TestResult::Failed {
                    reason: format!("Test harness error: {}", e),
                    diff: None,
                };
            }
            Err(panic_info) => {
                let panic_msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic_info.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "Unknown panic".to_string()
                };

                if self.metadata.validated == ValidationStatus::Fail {
                    return TestResult::ExpectedFailure {
                        reason: format!("Parser panic (expected): {}", panic_msg),
                    };
                }
                return TestResult::Panicked { reason: panic_msg };
            }
        };

        let errors: Vec<String> = diagnostics
            .iter()
            .filter(|d| d.is_error())
            .map(|d| d.to_string())
            .collect();
        let error_summary = || {
            let shown: Vec<&str> = errors.iter().map(String::as_str).take(3).collect();
            let more = if errors.len() > 3 {
                format!(" (+{} more)", errors.len() - 3)
            } else {
                String::new()
            };
            format!("{}{}", shown.join("; "), more)
        };

        // tcdocs marks a test `fail` either because the input is invalid
        // USFM or because the reference implementation is known to get it
        // wrong. So for `fail` inputs: reporting an error is a pass, and
        // matching the expected USX with no error is also a pass. Reporting
        // no error *and* not matching means we neither detect the problem
        // nor produce the expected recovery.
        let marked_fail = self.metadata.validated == ValidationStatus::Fail;
        if marked_fail && !errors.is_empty() {
            return TestResult::ExpectedFailure {
                reason: format!("Errors reported: {}", error_summary()),
            };
        }

        // Inputs marked `pass` (or unmarked) are valid USFM: no error
        // diagnostics allowed.
        if !marked_fail && !errors.is_empty() {
            return TestResult::Failed {
                reason: format!("Error diagnostics on valid input: {}", error_summary()),
                diff: None,
            };
        }

        // A case with no reference USX is still a real expectation when the
        // input is marked `pass`: "must parse with no error diagnostics", which
        // the check above has just established. Only a `fail` input that
        // reported nothing and has nothing to compare against is undecidable,
        // and that is the one shape still skipped.
        if !self.has_expected_usx() {
            if marked_fail {
                return TestResult::Skipped {
                    reason: "No origin.xml file for comparison".to_string(),
                };
            }
            return TestResult::Passed;
        }

        // Compare with expected
        let mut expected = match self.read_expected_usx() {
            Ok(usx) => usx,
            Err(e) => {
                return TestResult::Failed {
                    reason: format!("Failed to read expected USX: {}", e),
                    diff: None,
                };
            }
        };
        self.normalize_for_comparison(&mut actual);
        self.normalize_for_comparison(&mut expected);

        // A patch exists to paper over one specific difference. When the
        // output matches the reference file as it is, that difference is
        // gone (the parser changed, or tcdocs did) and the patch must be
        // deleted, so the patch directory keeps describing real deviations.
        if self.has_patch()
            && let Ok(mut unpatched) = self.read_unpatched_usx()
        {
            self.normalize_for_comparison(&mut unpatched);
            if compare_xml(&actual, &unpatched).is_ok() {
                return TestResult::Failed {
                    reason: format!(
                        "{} is redundant: the output matches the unpatched reference; delete the patch",
                        self.patch_path().display()
                    ),
                    diff: None,
                };
            }
        }

        match compare_xml(&actual, &expected) {
            Ok(()) => TestResult::Passed,
            Err(mismatch) if marked_fail => TestResult::UnexpectedPass {
                reason: format!(
                    "No error diagnostics on input marked 'fail', and USX differs: {}",
                    mismatch
                ),
            },
            Err(mismatch) => TestResult::Failed {
                reason: format!("{}", mismatch),
                diff: Some(Diff {
                    actual: format!("{}", actual),
                    expected: format!("{}", expected),
                }),
            },
        }
    }
}

/// Result of running a test
#[derive(Debug)]
pub enum TestResult {
    Passed,
    Failed { reason: String, diff: Option<Diff> },
    Skipped { reason: String },
    ExpectedFailure { reason: String },
    UnexpectedPass { reason: String },
    Panicked { reason: String },
}

impl TestResult {
    pub fn is_success(&self) -> bool {
        matches!(
            self,
            TestResult::Passed | TestResult::ExpectedFailure { .. } | TestResult::Skipped { .. }
        )
    }

    pub fn is_failure(&self) -> bool {
        matches!(
            self,
            TestResult::Failed { .. }
                | TestResult::UnexpectedPass { .. }
                | TestResult::Panicked { .. }
        )
    }
}

/// Diff between actual and expected output
#[derive(Debug)]
pub struct Diff {
    pub actual: String,
    pub expected: String,
}

/// Errors that can occur during testing
#[derive(Debug)]
pub enum TestError {
    Io(std::io::Error),
    Xml(xml::reader::Error),
    /// A reference patch does not parse or no longer applies.
    Patch(String),
}

impl Display for TestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TestError::Io(e) => write!(f, "IO error: {}", e),
            TestError::Xml(e) => write!(f, "XML error: {}", e),
            TestError::Patch(e) => write!(f, "patch error: {}", e),
        }
    }
}

impl std::error::Error for TestError {}

/// Mismatch between actual and expected XML
#[derive(Debug)]
pub struct XmlMismatch {
    pub description: String,
    pub actual: String,
    pub expected: String,
}

impl Display for XmlMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}\n  actual: {}\n  expected: {}",
            self.description, self.actual, self.expected
        )
    }
}

/// The version on the `\usfm` line of a USFM document, if it has one.
fn usfm_version(usfm: &str) -> Option<&str> {
    usfm.lines().find_map(|line| {
        let rest = line.trim().strip_prefix("\\usfm ")?;
        Some(rest.trim())
    })
}

/// Read test metadata from metadata.xml
fn read_metadata(path: &Path) -> Result<TestMetadata, TestError> {
    let file = File::open(path).map_err(TestError::Io)?;
    let file = BufReader::new(file);

    let parser = EventReader::new(file);
    let mut metadata = TestMetadata::default();
    let mut current_field: Option<MetadataField> = None;

    for e in parser {
        match e.map_err(TestError::Xml)? {
            XmlEvent::StartElement { name, .. } => {
                current_field = match name.local_name.as_str() {
                    "description" => Some(MetadataField::Description),
                    "validated" => Some(MetadataField::Validated),
                    "tags" => Some(MetadataField::Tags),
                    _ => None,
                }
            }
            XmlEvent::EndElement { .. } => {
                current_field = None;
            }
            XmlEvent::Characters(s) => match current_field {
                Some(MetadataField::Description) => metadata.description = s,
                Some(MetadataField::Validated) => {
                    metadata.validated = ValidationStatus::from(s.as_str())
                }
                Some(MetadataField::Tags) => {
                    metadata.tags.extend(s.split_whitespace().map(String::from));
                }
                None => {}
            },
            _ => {}
        }
    }

    Ok(metadata)
}

enum MetadataField {
    Description,
    Validated,
    Tags,
}

// Textual normalisation of the *expected* USX only, applied after any
// per-test patch. Each of these papers over a quirk that runs through many
// reference files, not of the parser (a quirk confined to one file gets a
// patch in `tasks/conformance/tcdocs-patches` instead):
//
// - A verse-end milestone is placed after the whitespace that precedes the
//   next verse (`text <verse eid/><verse sid/>` becomes
//   `text<verse eid/> <verse sid/>`), which is where the parser puts it.
// - `closed` is bookkeeping the parser does not emit: it records whether a
//   marker was closed explicitly in the source, which the AST keeps in the
//   node itself. Paratext writes `closed="false"` in eight tcdocs files and
//   usfm-grammar writes `closed="true"` on every explicitly closed `<char>`.
// - A verse start directly after text gets a space before it, since the
//   reference implementation always separates them.
static MATCH_BAD_VERSE_END: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r#" <verse eid="([^"]+)" /><verse"#).unwrap());
const REPLACE_BAD_VERSE_END: &str = r#"<verse eid="$1" /> <verse"#;

static MATCH_CLOSED: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r#" closed="(?:true|false)""#).unwrap());

static MATCH_NO_SPACE_BEFORE_VERSE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r#"([^\s>])(<verse [^>]*sid)"#).unwrap());

/// Normalize USX content for comparison
fn normalize_usx(content: &str) -> String {
    let content = MATCH_BAD_VERSE_END.replace_all(content, REPLACE_BAD_VERSE_END);
    let content = MATCH_CLOSED.replace_all(&content, "");
    let content = MATCH_NO_SPACE_BEFORE_VERSE.replace_all(&content, "$1 $2");
    content.into_owned()
}

/// Structural normalisation applied to *both* trees before comparison.
///
/// Trailing whitespace at the end of a `<note>` or `<cell>` is not compared.
/// The reference files are inconsistent there: for `\ft text \f*` Paratext
/// writes `text </char></note>`, `text </char> </note>` or
/// `text </char>\n</note>` depending on the file, so no single parser rule
/// matches all of them. The parser keeps the whitespace (rule 5 on `Text`);
/// what it does with it is covered by `crates/usfm_parser/tests/whitespace.rs`, not
/// by this suite.
pub fn normalize_tree(node: &mut XmlNode) {
    let XmlNode::Element(element) = node else {
        return;
    };
    for child in &mut element.children {
        normalize_tree(child);
    }
    if element.name.local_name == "note" || element.name.local_name == "cell" {
        trim_trailing_text(element);
    }
}

/// Collapse every run of ASCII whitespace inside a text node to one space,
/// trim each text node at both ends, and drop what becomes empty. Applied to
/// both trees, this compares two documents for everything except how much
/// whitespace sits between their pieces. See
/// [`TestCase::normalize_for_comparison`] for why one root needs it.
pub fn collapse_whitespace_tree(node: &mut XmlNode) {
    let XmlNode::Element(element) = node else {
        return;
    };
    for child in &mut element.children {
        collapse_whitespace_tree(child);
    }
    element.children.retain_mut(|child| {
        let XmlNode::Text(text) = child else {
            return true;
        };
        *text = text
            .split_ascii_whitespace()
            .collect::<Vec<&str>>()
            .join(" ");
        !text.is_empty()
    });
}

/// Trim ASCII whitespace from the end of the last text in `element`,
/// descending through trailing child elements, and drop what becomes empty.
/// A trailing verse-end milestone is skipped, since it sits after the text.
fn trim_trailing_text(element: &mut XmlElement) {
    let mut index = element.children.len();
    while index > 0 {
        index -= 1;
        match &mut element.children[index] {
            XmlNode::Element(child) if child.name.local_name == "verse" => continue,
            XmlNode::Element(child) => {
                trim_trailing_text(child);
                return;
            }
            XmlNode::Text(text) => {
                let trimmed = text.trim_end_matches(|c: char| c.is_ascii_whitespace());
                if trimmed.is_empty() {
                    // Whitespace-only: drop it and keep looking, since the
                    // text that matters may sit in the element before it.
                    element.children.remove(index);
                    continue;
                }
                *text = trimmed.to_string();
                return;
            }
        }
    }
}

/// Compare attributes without considering order
fn attributes_equal(
    actual: &[xml::attribute::OwnedAttribute],
    expected: &[xml::attribute::OwnedAttribute],
) -> bool {
    if actual.len() != expected.len() {
        return false;
    }
    // Check that every expected attribute exists in actual with same value
    expected.iter().all(|exp| {
        actual
            .iter()
            .any(|act| act.name == exp.name && act.value == exp.value)
    })
}

/// Compare two XML nodes recursively
pub fn compare_xml(actual: &XmlNode, expected: &XmlNode) -> Result<(), XmlMismatch> {
    match (actual, expected) {
        (XmlNode::Element(actual), XmlNode::Element(expected)) => {
            if actual.name != expected.name {
                return Err(XmlMismatch {
                    description: "Element name mismatch".to_string(),
                    actual: format!("<{}>", actual.name),
                    expected: format!("<{}>", expected.name),
                });
            }
            if !attributes_equal(&actual.attributes, &expected.attributes) {
                return Err(XmlMismatch {
                    description: format!("Attributes mismatch on <{}>", actual.name),
                    actual: format!("{:?}", actual.attributes),
                    expected: format!("{:?}", expected.attributes),
                });
            }

            let mut actual_children = actual.children.iter();
            let mut expected_children = expected.children.iter();

            loop {
                match (actual_children.next(), expected_children.next()) {
                    (Some(a), Some(e)) => compare_xml(a, e)?,
                    (Some(node), None) => {
                        // TODO: this shouldn't be necessary
                        if let XmlNode::Text(text) = node
                            && text.is_empty()
                        {
                            continue;
                        }
                        return Err(XmlMismatch {
                            description: format!("Extra child in <{}>", actual.name),
                            actual: format!("```{}```", node),
                            expected: "(none)".to_string(),
                        });
                    }
                    (None, Some(node)) => {
                        if let XmlNode::Text(text) = node
                            && text.is_empty()
                        {
                            continue;
                        }
                        return Err(XmlMismatch {
                            description: format!("Missing child in <{}>", actual.name),
                            actual: "(none)".to_string(),
                            expected: format!("```{}```", node),
                        });
                    }
                    (None, None) => break,
                }
            }
            Ok(())
        }
        (XmlNode::Text(actual), XmlNode::Text(expected)) => {
            if actual != expected {
                return Err(XmlMismatch {
                    description: "Text mismatch".to_string(),
                    actual: format!("\"{}\"", actual),
                    expected: format!("\"{}\"", expected),
                });
            }
            Ok(())
        }
        (XmlNode::Text(actual), XmlNode::Element(expected)) => Err(XmlMismatch {
            description: "Expected element, got text".to_string(),
            actual: format!("\"{}\"", actual),
            expected: format!("<{}>", expected.name),
        }),
        (XmlNode::Element(actual), XmlNode::Text(expected)) => Err(XmlMismatch {
            description: "Expected text, got element".to_string(),
            actual: format!("<{}>", actual.name),
            expected: format!("\"{}\"", expected),
        }),
    }
}

/// Discover all test cases, under every root in [`ROOTS`]
pub fn discover_tests() -> Vec<TestCase> {
    let mut tests: Vec<TestCase> = ROOTS
        .iter()
        .flat_map(|(root, _)| discover_tests_in(Path::new(root)))
        .collect();
    tests.sort_by(|a, b| a.name.cmp(&b.name));
    tests
}

/// The directory of the case named `name`, looked up in each root in turn.
/// The inverse of [`TestCase::name`], for `--show` and the like.
pub fn path_for_name(name: &str) -> Option<PathBuf> {
    let name = name.trim_end_matches('/');
    ROOTS.iter().find_map(|(root, prefix)| {
        let tail = match *prefix {
            "" => name,
            prefix => name.strip_prefix(prefix)?.strip_prefix('/')?,
        };
        let path = Path::new(root).join(tail);
        path.is_dir().then_some(path)
    })
}

/// Discover test cases in a specific directory
pub fn discover_tests_in(root: &Path) -> Vec<TestCase> {
    let mut tests = Vec::new();
    discover_tests_recursive(root, &mut tests);
    tests.sort_by(|a, b| a.name.cmp(&b.name));
    tests
}

fn discover_tests_recursive(dir: &Path, tests: &mut Vec<TestCase>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    // Check if this directory is a test case (has metadata.xml or origin.usfm)
    let is_test_dir = dir.join("metadata.xml").exists() || dir.join("origin.usfm").exists();

    if is_test_dir && let Ok(test) = TestCase::load(dir) {
        tests.push(test);
    }

    // Recurse into subdirectories
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            discover_tests_recursive(&path, tests);
        }
    }
}

/// Run all tests and return a summary
pub fn run_all_tests() -> TestSummary {
    let tests = discover_tests();
    run_tests(&tests)
}

/// Run a set of tests and return a summary
pub fn run_tests(tests: &[TestCase]) -> TestSummary {
    let mut summary = TestSummary::default();

    for test in tests {
        let result = test.run();
        match &result {
            TestResult::Passed => summary.passed += 1,
            TestResult::Failed { .. } => {
                summary.failed += 1;
                summary.failures.push((test.clone(), result));
            }
            TestResult::Skipped { .. } => summary.skipped += 1,
            TestResult::ExpectedFailure { .. } => summary.expected_failures += 1,
            TestResult::UnexpectedPass { .. } => {
                summary.unexpected_passes += 1;
                summary.failures.push((test.clone(), result));
            }
            TestResult::Panicked { .. } => {
                summary.panicked += 1;
                summary.failures.push((test.clone(), result));
            }
        }
    }

    summary
}

/// Filter tests by category, or by any prefix of a test name
pub fn filter_by_category<'a>(tests: &'a [TestCase], category: &str) -> Vec<&'a TestCase> {
    tests
        .iter()
        .filter(|t| t.name.starts_with(category))
        .collect()
}

/// Get all unique categories from test cases
pub fn get_categories(tests: &[TestCase]) -> Vec<String> {
    let mut categories: Vec<String> = tests.iter().map(|t| t.category.clone()).collect();
    categories.sort();
    categories.dedup();
    categories
}

/// Summary of test results
#[derive(Debug, Default)]
pub struct TestSummary {
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub expected_failures: usize,
    pub unexpected_passes: usize,
    pub panicked: usize,
    pub failures: Vec<(TestCase, TestResult)>,
}

impl TestSummary {
    pub fn total(&self) -> usize {
        self.passed
            + self.failed
            + self.skipped
            + self.expected_failures
            + self.unexpected_passes
            + self.panicked
    }

    pub fn is_success(&self) -> bool {
        self.failed == 0 && self.unexpected_passes == 0 && self.panicked == 0
    }

    pub fn pass_rate(&self) -> f64 {
        let testable = self.total() - self.skipped;
        if testable == 0 {
            return 100.0;
        }
        ((self.passed + self.expected_failures) as f64 / testable as f64) * 100.0
    }
}

impl Display for TestSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Test Results:")?;
        writeln!(f, "  Passed: {}", self.passed)?;
        writeln!(f, "  Failed: {}", self.failed)?;
        writeln!(f, "  Panicked: {}", self.panicked)?;
        writeln!(f, "  Skipped: {}", self.skipped)?;
        writeln!(f, "  Expected failures: {}", self.expected_failures)?;
        writeln!(f, "  Unexpected passes: {}", self.unexpected_passes)?;
        writeln!(f, "  Total: {}", self.total())?;
        writeln!(f, "  Pass rate: {:.1}%", self.pass_rate())?;

        if !self.failures.is_empty() {
            writeln!(f, "\nFailures:")?;
            for (test, result) in &self.failures {
                writeln!(f, "\n  {} - {}", test.name, test.metadata.description)?;
                match result {
                    TestResult::Failed { reason, .. } => writeln!(f, "    {}", reason)?,
                    TestResult::UnexpectedPass { reason } => writeln!(f, "    {}", reason)?,
                    TestResult::Panicked { reason } => writeln!(f, "    PANIC: {}", reason)?,
                    _ => {}
                }
            }
        }

        Ok(())
    }
}

/// Run a single test case and assert it passes.
/// Used by generated test functions.
pub fn run_single_test(path: &Path) {
    let test = TestCase::load(path).expect("Failed to load test case");
    let result = test.run();

    match &result {
        TestResult::Passed => {}
        TestResult::Skipped { reason } => {
            println!("SKIPPED: {}", reason);
        }
        TestResult::ExpectedFailure { reason } => {
            println!("EXPECTED FAILURE: {}", reason);
        }
        TestResult::Failed { reason, diff } => {
            if let Some(diff) = diff {
                println!("ACTUAL:\n{}\n", diff.actual);
                println!("EXPECTED:\n{}\n", diff.expected);
            }
            panic!("Test failed: {}", reason);
        }
        TestResult::UnexpectedPass { reason } => {
            panic!("Unexpected pass: {}", reason);
        }
        TestResult::Panicked { reason } => {
            panic!("Parser panicked: {}", reason);
        }
    }
}

// Include the auto-generated test functions
#[cfg(test)]
mod generated_tests {
    include!(concat!(env!("OUT_DIR"), "/generated_tests.rs"));
}
