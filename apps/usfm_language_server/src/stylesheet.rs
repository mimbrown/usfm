//! Which stylesheet a document is parsed with.
//!
//! A Paratext project ships a `custom.sty` beside its books, holding the
//! markers that project invented. Parsing with the default sheet alone would
//! report every one of them as `unknown-marker`, so the server looks for a
//! project sheet the way the editor's user would expect:
//!
//! 1. `initializationOptions.stylesheet`, a path the client sends at
//!    `initialize` — absolute, or relative to the workspace root (or, with no
//!    workspace, to the document's own directory);
//! 2. otherwise the project's own files next to the document:
//!    [`PROJECT_STYLESHEET`] for every book, and for a peripheral book
//!    (`\id FRT`, `INT`, `GLO`, `XXA`, …) [`PERIPHERAL_STYLESHEET`] over it,
//!    so that `frtbak.sty` wins there and is not read for Scripture at all —
//!    `usfm_paratext`'s rule for a project;
//! 3. otherwise the default sheet.
//!
//! A sheet found either way **extends** the default rather than replacing it,
//! exactly as `crates/usfm_parser/tests/recovery.rs`'s
//! `machine_py_custom_stylesheet` does: a project's `.sty` names the markers
//! it adds or overrides, not the eight hundred it inherits. (This is where the
//! server differs from `usfm parse --stylesheet`, which takes the named file
//! as the whole sheet.)
//!
//! A sheet that cannot be read or parsed is reported once, as a
//! `window/showMessage` warning, and the document is parsed with the default
//! sheet: a project with a broken `.sty` still gets diagnostics.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use usfm::StyleSheet;
use usfm::parser::DEFAULT_STYLESHEET;

/// The name Paratext gives a project's stylesheet override.
pub const PROJECT_STYLESHEET: &str = "custom.sty";
/// The name of a project's sheet for its peripheral books.
pub const PERIPHERAL_STYLESHEET: &str = "frtbak.sty";

/// Whether `text` is a peripheral book, by its `\id` line: front and back
/// matter, an introduction, a glossary and the like. A document with no
/// readable `\id` is taken as Scripture.
pub fn is_peripheral(text: &str) -> bool {
    text.lines()
        .find_map(|line| line.trim_start().strip_prefix("\\id "))
        .and_then(|rest| rest.trim_start().get(..3))
        .and_then(|code| code.parse::<usfm::ast::BookCode>().ok())
        .is_some_and(|code| code.is_non_scripture())
}

/// The stylesheet each document is parsed with, and the sheets read so far.
///
/// Parsing a `.sty` file means reading it from disk and rebuilding the whole
/// default sheet around it, so every path is read once and the result kept —
/// including the failure, which is what keeps a broken sheet from warning on
/// every keystroke.
///
/// No `Debug`: a `StyleSheet` is eight hundred rules and has none.
#[derive(Default)]
pub struct Stylesheets {
    /// `initializationOptions.stylesheet`, resolved against the workspace root
    /// if it was relative and a root was given.
    configured: Option<PathBuf>,
    /// One entry per list of files tried, in the order they are read;
    /// `None` means it failed and has been reported.
    cache: HashMap<Vec<PathBuf>, Option<Arc<StyleSheet>>>,
}

impl Stylesheets {
    /// Record what `initialize` was told: the `stylesheet` option (if the
    /// client sent one) and the workspace root (if it has one).
    pub fn configure(&mut self, stylesheet: Option<&str>, root: Option<PathBuf>) {
        self.configured =
            stylesheet
                .map(PathBuf::from)
                .map(|path| match (path.is_absolute(), root.as_ref()) {
                    (false, Some(root)) => root.join(path),
                    _ => path,
                });
    }

    /// The sheet to parse the document at `path` with, and a warning to show
    /// if a sheet was named and could not be used.
    ///
    /// The warning comes back rather than being sent from here so that this
    /// type stays a plain cache with no `Client` and no `async`, which is also
    /// what makes it testable without a server.
    ///
    /// `peripheral` is [`is_peripheral`] of the document's text, which
    /// decides the order a project's two files are read in.
    pub fn for_document(
        &mut self,
        path: Option<&Path>,
        peripheral: bool,
    ) -> (Arc<StyleSheet>, Option<String>) {
        let sheet_path = self.paths_for(path, peripheral);
        if sheet_path.is_empty() {
            return (Arc::clone(&DEFAULT_STYLESHEET), None);
        }
        if let Some(cached) = self.cache.get(&sheet_path) {
            // Already read: a hit hands back the sheet, a miss the default
            // sheet and no second warning about the same file.
            return (
                cached
                    .clone()
                    .unwrap_or_else(|| Arc::clone(&DEFAULT_STYLESHEET)),
                None,
            );
        }
        match load(&sheet_path) {
            Ok(sheet) => {
                self.cache.insert(sheet_path, Some(Arc::clone(&sheet)));
                (sheet, None)
            }
            Err(message) => {
                self.cache.insert(sheet_path, None);
                (Arc::clone(&DEFAULT_STYLESHEET), Some(message))
            }
        }
    }

    /// Which `.sty` files apply to the document at `path`, in the order
    /// they are read: the later wins.
    fn paths_for(&self, path: Option<&Path>, peripheral: bool) -> Vec<PathBuf> {
        if self.configured.is_some() {
            return self.path_for(path).into_iter().collect();
        }
        let Some(directory) = path.and_then(Path::parent) else {
            return Vec::new();
        };
        let order: &[&str] = if peripheral {
            &[PROJECT_STYLESHEET, PERIPHERAL_STYLESHEET]
        } else {
            &[PROJECT_STYLESHEET]
        };
        order
            .iter()
            .map(|name| directory.join(name))
            .filter(|path| path.is_file())
            .collect()
    }

    /// The configured `.sty` file, if any, for the document at `path`.
    fn path_for(&self, path: Option<&Path>) -> Option<PathBuf> {
        if let Some(configured) = &self.configured {
            // A relative option with no workspace root is resolved against the
            // document; with neither, it is taken as given (and relative to
            // wherever the server was started, which is the client's business).
            if configured.is_absolute() {
                return Some(configured.clone());
            }
            return Some(match path.and_then(Path::parent) {
                Some(directory) => directory.join(configured),
                None => configured.clone(),
            });
        }
        None
    }
}

/// Read `.sty` files, in order, over a copy of the default sheet: an entry
/// for a marker the sheet has amends it, a new marker adds a rule.
fn load(paths: &[PathBuf]) -> Result<Arc<StyleSheet>, String> {
    let mut extended = (**DEFAULT_STYLESHEET).clone();
    for path in paths {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("usfm: cannot read the stylesheet {}: {e}", path.display()))?;
        extended.extend_from_str(&text).map_err(|e| {
            format!(
                "usfm: cannot parse the stylesheet {}: {e:?}",
                path.display()
            )
        })?;
    }
    Ok(Arc::new(extended))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A directory of this test run's own, named after the test in it.
    fn scratch(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!("usfm-ls-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("creating the test directory");
        directory
    }

    fn book(directory: &Path) -> PathBuf {
        let path = directory.join("41MAT.SFM");
        std::fs::write(&path, "\\id MAT\n").expect("writing the test book");
        path
    }

    #[test]
    fn with_no_project_sheet_a_document_gets_the_default() {
        let directory = scratch("no-sheet");
        let mut sheets = Stylesheets::default();
        let (sheet, warning) = sheets.for_document(Some(&book(&directory)), false);
        assert!(Arc::ptr_eq(&sheet, &DEFAULT_STYLESHEET));
        assert_eq!(warning, None);
        // And so does a document with no path at all (an untitled buffer).
        let (sheet, warning) = sheets.for_document(None, false);
        assert!(Arc::ptr_eq(&sheet, &DEFAULT_STYLESHEET));
        assert_eq!(warning, None);
    }

    #[test]
    fn a_custom_sty_beside_the_file_extends_the_default() {
        let directory = scratch("beside");
        std::fs::write(
            directory.join(PROJECT_STYLESHEET),
            "\\Marker test\n\\StyleType Character\n",
        )
        .expect("writing custom.sty");

        let mut sheets = Stylesheets::default();
        let (sheet, warning) = sheets.for_document(Some(&book(&directory)), false);
        assert_eq!(warning, None);
        // The project's own marker, and one of the default sheet's: the
        // project sheet extends, it does not replace.
        assert!(sheet.get_rule_by_marker("test").is_some());
        assert!(sheet.get_rule_by_marker("p").is_some());
        // Read once: the second call is the cache.
        let (again, _) = sheets.for_document(Some(&book(&directory)), false);
        assert!(Arc::ptr_eq(&sheet, &again));
    }

    /// A `custom.sty` saved with a byte-order mark, as Paratext's own
    /// project sheets can be: its first marker used to vanish, so `\zgrk`
    /// was reported as an unknown custom marker and dropped.
    #[test]
    fn a_custom_sty_with_a_byte_order_mark_keeps_its_first_marker() {
        let directory = scratch("bom");
        std::fs::write(
            directory.join(PROJECT_STYLESHEET),
            "\u{feff}\\Marker zgrk\r\n\\Name grk - Change to Greek font\r\n\\Endmarker zgrk*\r\n\
             \\StyleType character\r\n\\OccursUnder p q1 f\r\n\
             \\TextProperties nonpublishable nonvernacular\r\n",
        )
        .expect("writing custom.sty");

        let mut sheets = Stylesheets::default();
        let (sheet, warning) = sheets.for_document(Some(&book(&directory)), false);
        assert_eq!(warning, None);
        assert!(sheet.get_rule_by_marker("zgrk").is_some());
        let parse = usfm::parse_with(
            "\\id MAT\n\\c 1\n\\p \\v 1 a \\zgrk logos\\zgrk* b\n",
            &sheet,
        );
        assert!(parse.diagnostics.is_empty(), "{:?}", parse.diagnostics);
    }

    /// Most of a real project's `custom.sty` is overrides of Paratext's own
    /// markers with no `\\StyleType` — a font, a colour, an indent. Read on its
    /// own, such a sheet was rejected whole (`StyleTypeRequired`), so the
    /// project's new markers were lost with it. This is the shape of one
    /// (upgraded by Paratext 8), abridged.
    #[test]
    fn overrides_without_a_style_type_amend_the_default() {
        let directory = scratch("overrides");
        std::fs::write(
            directory.join(PROJECT_STYLESHEET),
            "# Custom style file created by the upgrade to Paratext 8.0.\n\
             \n\\Marker toc1\n\\Regular\n\\Color 16711680\n\
             \n\\Marker mt4\n\\OccursUnder id ip pb\n\\Rank 6\n\\TextProperties nonpublishable\n\
             \n\\Marker vp\n\\Endmarker vp*\n\\Color 16711680\n\
             \n\\Marker zgrk\n\\Name grk - Change to Greek font\n\\Endmarker zgrk*\n\
             \\StyleType character\n\\OccursUnder c p q1 f fe\n\
             \\TextProperties nonpublishable nonvernacular\n\
             \n\\Marker em\n\\Italic -\n\\Bold\n\
             \n\\Marker (\n\\Name open parenthesis\n\\StyleType Character\n\
             \n\\Marker z-timeline-start \n\\TextProperties nonpublishable\n\\StyleType Character\n",
        )
        .expect("writing custom.sty");

        let mut sheets = Stylesheets::default();
        let (sheet, warning) = sheets.for_document(Some(&book(&directory)), false);
        assert_eq!(warning, None);
        // The new markers are there, and an override kept what it did not
        // mention: `\\vp` is still a character style, `\\mt4` a paragraph
        // with the project's `OccursUnder`.
        assert!(sheet.get_rule_by_marker("zgrk").unwrap().is_character());
        assert!(sheet.get_rule_by_marker("z-timeline-start").is_some());
        assert!(sheet.get_rule_by_marker("vp").unwrap().is_character());
        let mt4 = sheet.get_rule_by_marker("mt4").unwrap();
        assert!(mt4.is_paragraph());
        assert_eq!(mt4.occurs_under, ["id", "ip", "pb"]);
        let parse = usfm::parse_with(
            "\\id MAT\n\\c 1\n\\p \\v 1 a \\zgrk logos\\zgrk* b\n",
            &sheet,
        );
        assert!(parse.diagnostics.is_empty(), "{:?}", parse.diagnostics);
    }

    #[test]
    fn a_configured_path_wins_over_the_file_beside_the_document() {
        let directory = scratch("configured");
        std::fs::write(
            directory.join(PROJECT_STYLESHEET),
            "\\Marker beside\n\\StyleType Character\n",
        )
        .expect("writing custom.sty");
        std::fs::write(
            directory.join("project.sty"),
            "\\Marker configured\n\\StyleType Character\n",
        )
        .expect("writing project.sty");

        let mut sheets = Stylesheets::default();
        sheets.configure(
            Some(directory.join("project.sty").to_str().unwrap()),
            Some(directory.clone()),
        );
        let (sheet, warning) = sheets.for_document(Some(&book(&directory)), false);
        assert_eq!(warning, None);
        assert!(sheet.get_rule_by_marker("configured").is_some());
        assert!(sheet.get_rule_by_marker("beside").is_none());
    }

    #[test]
    fn a_relative_configured_path_resolves_against_the_workspace_root() {
        let directory = scratch("relative");
        std::fs::write(
            directory.join("project.sty"),
            "\\Marker configured\n\\StyleType Character\n",
        )
        .expect("writing project.sty");

        let mut sheets = Stylesheets::default();
        sheets.configure(Some("project.sty"), Some(directory.clone()));
        let (sheet, warning) = sheets.for_document(Some(&book(&directory)), false);
        assert_eq!(warning, None);
        assert!(sheet.get_rule_by_marker("configured").is_some());
    }

    /// `frtbak.sty` is read over `custom.sty` for a peripheral book and not
    /// at all for a book of Scripture.
    #[test]
    fn frtbak_sty_is_read_for_peripheral_books_only() {
        let directory = scratch("frtbak");
        let entry = |marker: &str, name: &str| {
            format!(
                "\\Marker {marker}\n\\Endmarker {marker}*\n\\Name {name}\n\\StyleType Character\n\n"
            )
        };
        std::fs::write(
            directory.join(PROJECT_STYLESHEET),
            entry("zboth", "from custom"),
        )
        .unwrap();
        std::fs::write(
            directory.join(PERIPHERAL_STYLESHEET),
            entry("zboth", "from frtbak") + &entry("zfrtbak", "f"),
        )
        .unwrap();
        let (main, _) = Stylesheets::default().for_document(Some(&book(&directory)), false);
        assert!(main.get_rule_by_marker("zfrtbak").is_none());
        let (peripheral, _) = Stylesheets::default().for_document(Some(&book(&directory)), true);
        assert!(peripheral.get_rule_by_marker("zfrtbak").is_some());
        let mut sheets = Stylesheets::default();
        let mut name = |text: &str| {
            let (sheet, warning) =
                sheets.for_document(Some(&book(&directory)), is_peripheral(text));
            assert_eq!(warning, None);
            sheet.get_rule_by_marker("zboth").unwrap().name.clone()
        };
        assert_eq!(name("\\id MAT\n\\c 1").as_deref(), Some("from custom"));
        assert_eq!(name("\\id INT intro\n").as_deref(), Some("from frtbak"));
        assert_eq!(name("no id at all").as_deref(), Some("from custom"));
    }

    #[test]
    fn a_missing_sheet_warns_once_and_falls_back_to_the_default() {
        let directory = scratch("missing");
        let mut sheets = Stylesheets::default();
        sheets.configure(
            Some(directory.join("nowhere.sty").to_str().unwrap()),
            Some(directory.clone()),
        );

        let (sheet, warning) = sheets.for_document(Some(&book(&directory)), false);
        assert!(Arc::ptr_eq(&sheet, &DEFAULT_STYLESHEET));
        let warning = warning.expect("a missing stylesheet is reported");
        assert!(warning.contains("nowhere.sty"), "{warning}");

        // The same document again says nothing more: the failure is cached, so
        // a typo in the setting does not warn on every keystroke.
        let (sheet, warning) = sheets.for_document(Some(&book(&directory)), false);
        assert!(Arc::ptr_eq(&sheet, &DEFAULT_STYLESHEET));
        assert_eq!(warning, None);
    }
}
