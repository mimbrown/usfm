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

/// `custom.sty` is read for every book; `frtbak.sty` is read over it for a
/// peripheral book, where it wins, and not at all for a book of Scripture.
#[test]
fn frtbak_sty_is_read_for_peripheral_books_only() {
    let dir = std::env::temp_dir().join(format!("usfm-paratext-sheets-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(fixture().join("Settings.xml"), dir.join("Settings.xml")).unwrap();
    let entry = |marker: &str, name: &str| {
        format!(
            "\\Marker {marker}\n\\Endmarker {marker}*\n\\Name {name}\n\\StyleType Character\n\n"
        )
    };
    std::fs::write(
        dir.join("custom.sty"),
        entry("zboth", "from custom") + &entry("zcustom", "c"),
    )
    .unwrap();
    let project = Project::open(&dir).unwrap();
    let name = |sheet: &usfm_style::StyleSheet| {
        sheet
            .get_rule_by_marker("zboth")
            .and_then(|rule| rule.name.clone())
    };

    // One file: one sheet for every book.
    let sheets = project.style_sheets().unwrap();
    assert!(std::sync::Arc::ptr_eq(&sheets.main, &sheets.peripheral));

    std::fs::write(
        dir.join("frtbak.sty"),
        entry("zboth", "from frtbak") + &entry("zfrtbak", "f"),
    )
    .unwrap();
    let sheets = project.style_sheets().unwrap();
    for sheet in [&sheets.main, &sheets.peripheral] {
        assert!(sheet.get_rule_by_marker("zcustom").is_some());
        assert!(sheet.get_rule_by_marker("xt").is_some());
    }
    assert!(sheets.peripheral.get_rule_by_marker("zfrtbak").is_some());
    assert!(sheets.main.get_rule_by_marker("zfrtbak").is_none());
    assert_eq!(
        name(sheets.for_book(code("MAT"))).as_deref(),
        Some("from custom")
    );
    assert_eq!(
        name(sheets.for_book(code("INT"))).as_deref(),
        Some("from frtbak")
    );
    assert_eq!(
        name(sheets.for_book(code("XXA"))).as_deref(),
        Some("from frtbak")
    );
    assert_eq!(
        name(&project.style_sheet().unwrap()).as_deref(),
        Some("from custom")
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_folder_with_no_settings_is_an_error() {
    let missing = fixture().join("no-such-project");
    match Project::open(&missing) {
        Err(Error::Io { path, .. }) => assert!(path.ends_with("Settings.xml")),
        other => panic!("expected an Io error, got {other:?}"),
    }
}

/// The project's punctuation and names read a reference as the project
/// writes it: Urdu digits, the Arabic comma, the Urdu book name.
#[test]
fn a_reference_in_the_projects_own_spelling() {
    use usfm_semantic::citation::{Piece, parse_citations};

    let project = Project::open(fixture()).unwrap();
    let format = project.settings().citation_format();
    let names = project.book_names().table();
    let found: Vec<(String, usize, Option<usize>)> =
        parse_citations("متی ۵:۳، ۷", &format, &names, None)
            .into_iter()
            .filter_map(|piece| match piece {
                Piece::Citation(c) => Some((c.book.to_string(), c.start.chapter, c.start.verse)),
                Piece::Text(_) => None,
            })
            .collect();
    assert_eq!(
        found,
        [
            ("MAT".to_string(), 5, Some(3)),
            ("MAT".to_string(), 5, Some(7))
        ]
    );
}
