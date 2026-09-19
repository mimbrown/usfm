//! AST snapshot tests.
//!
//! The tcdocs conformance suite compares USX text, which hides changes to the
//! parsed tree. These tests snapshot a compact rendering of the AST itself
//! (plus any diagnostics) for a fixed corpus, so lexer and parser refactors
//! can be checked for behaviour changes at the tree level.
//!
//! Corpus:
//! - A curated subset of tcdocs inputs (read from the submodule; skipped if absent).
//! - Local malformed inputs under `tests/fixtures/malformed/`.
//!
//! Review and accept changes with `cargo insta review` (or set
//! `INSTA_UPDATE=always` for a bulk accept).

mod common;

use std::path::{Path, PathBuf};

const TCDOCS_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../tcdocs/tests");
const MALFORMED_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/malformed");

/// Curated tcdocs cases covering each category and the constructs the parser
/// supports (or is expected to recover from).
const TCDOCS_FIXTURES: &[&str] = &[
    "basic/minimal",
    "basic/attributes",
    "basic/multiple-chapters",
    "basic/multiple-paragraphs",
    "basic/section",
    "mandatory/v",
    "mandatory/emptyV",
    "advanced/default-attributes",
    "advanced/implicit",
    "advanced/list",
    "advanced/header",
    "advanced/complex",
    "advanced/figureInNote",
    "advanced/custom-attributes",
    "advanced/periph",
    "specExamples/table",
    "specExamples/chapter-verse",
    "specExamples/attributes",
    "specExamples/poetry",
    "specExamples/footnote",
    "specExamples/cross-ref",
    "specExamples/milestone",
    "specExamples/titles",
    "specExamples/paragraph",
    "specExamples/list",
    "specExamples/introduction1",
    "specExamples/extended/sidebars",
    "usfmjsTests/nb",
    "usfmjsTests/qt",
    "usfmjsTests/links",
    "usfmjsTests/tw_words",
    "usfmjsTests/greek",
    "usfmjsTests/1ch_verse_span",
    "usfmjsTests/misc_footnotes",
    "usfmjsTests/esb",
    "usfmjsTests/out_of_sequence_verses",
    "usfmjsTests/inline_words",
    "usfmjsTests/heb-12-27.grc",
    "special-cases/nested-notes",
    "special-cases/notes2",
    "special-cases/empty-attributes",
    "special-cases/newline-attributes",
    "special-cases/nbsp",
    "special-cases/empty-para",
    "special-cases/nesting",
    "special-cases/id-in-text",
    "special-cases/punct-at-break",
    "paratextTests/CharStyleNotClosed",
    "paratextTests/FootnoteNotClosed",
    "paratextTests/NestingUnclosed",
    "paratextTests/MarkersMissingSpace",
    "paratextTests/InvalidMarker",
    "paratextTests/ValidMilestones",
    "paratextTests/InvalidMilestone_EndWithoutStart",
    "paratextTests/EmptyMarkers",
    "paratextTests/MissingIdMarker",
    "paratextTests/NoErrorsNesting",
    "paratextTests/ValidRubyMarkup",
    "paratextTests/CharStyleCrossesFootnote",
    "paratextTests/CrossReferencesQuoteOutsideNote",
    "biblica/CrossRefWithPipe",
    "biblica/PublishingVersesNotClosed",
];

fn snapshot_file(name: &str, path: &Path) {
    let source = match std::fs::read_to_string(path) {
        Ok(source) => source,
        Err(err) => panic!("cannot read fixture {}: {err}", path.display()),
    };
    let rendered = common::render(&source);
    insta::with_settings!({
        snapshot_path => "snapshots",
        prepend_module_to_snapshot => false,
        description => source.as_str(),
        omit_expression => true,
    }, {
        insta::assert_snapshot!(name, rendered);
    });
}

#[test]
fn tcdocs_corpus() {
    let root = Path::new(TCDOCS_ROOT);
    if !root.exists() {
        eprintln!("tcdocs submodule not present; skipping tcdocs snapshots");
        return;
    }
    for fixture in TCDOCS_FIXTURES {
        let path = root.join(fixture).join("origin.usfm");
        let name = format!("tcdocs__{}", fixture.replace(['/', '.'], "_"));
        snapshot_file(&name, &path);
    }
}

#[test]
fn malformed_corpus() {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(MALFORMED_ROOT)
        .expect("malformed fixtures directory")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "usfm"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no malformed fixtures found");
    for path in paths {
        let stem = path.file_stem().unwrap().to_string_lossy();
        let name = format!("malformed__{}", stem.replace(['-', '.'], "_"));
        snapshot_file(&name, &path);
    }
}
