//! A project folder read end to end: `tests/fixtures/SSVx`, written for these
//! tests in the shape of a real Paratext project (the naming, the BOMs, a
//! `custom.sty` with a font switch and an amendment).

use std::path::{Path, PathBuf};

use usfm_ast::BookCode;
use usfm_paratext::{BookNameForm, Error, Project};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/SSVx")
}

fn code(s: &str) -> BookCode {
    s.parse().unwrap()
}

#[test]
fn the_settings_and_names_are_read() {
    let project = Project::open(fixture()).unwrap();
    assert_eq!(project.settings().name(), Some("SSVx"));
    assert_eq!(project.settings().get("Versification"), Some("4"));
    assert_eq!(project.settings().naming().form, BookNameForm::NumberCode);
    let books: Vec<String> = project.books().iter().map(ToString::to_string).collect();
    assert_eq!(books, ["MAT", "INT"]);
    let int = project.book_names().get(code("INT")).unwrap();
    assert_eq!(int.short.as_deref(), Some("دیباچہ"));
    assert_eq!(int.abbreviation, None);
}

#[test]
fn every_present_book_is_found_and_read() {
    let project = Project::open(fixture()).unwrap();
    for book in project.books() {
        let path = project.book_path(book).unwrap();
        assert!(path.is_file(), "{}", path.display());
        let text = project.read_book(book).unwrap();
        assert!(text.starts_with(&format!("\\id {book}")), "{text:?}");
    }
    assert_eq!(
        project.book_path(code("INT")).unwrap(),
        fixture().join("A7INTSSVx.SFM")
    );
}

#[test]
fn a_missing_book_is_an_io_error_naming_its_file() {
    let project = Project::open(fixture()).unwrap();
    match project.read_book(code("GEN")) {
        Err(Error::Io { path, .. }) => assert!(path.ends_with("01GENSSVx.SFM")),
        other => panic!("expected an Io error, got {other:?}"),
    }
}

#[test]
fn the_style_sheet_is_the_default_with_custom_sty_over_it() {
    let project = Project::open(fixture()).unwrap();
    let sheet = project.style_sheet().unwrap();
    assert!(sheet.get_rule_by_marker("zgrk").is_some());
    // An amendment keeps `p` a paragraph, and the rest of the default sheet
    // is still there.
    assert!(sheet.get_rule_by_marker("p").is_some());
    assert!(sheet.get_rule_by_marker("xt").is_some());
}

#[test]
fn a_folder_with_no_settings_is_an_error() {
    let missing = fixture().join("no-such-project");
    match Project::open(&missing) {
        Err(Error::Io { path, .. }) => assert!(path.ends_with("Settings.xml")),
        other => panic!("expected an Io error, got {other:?}"),
    }
}
