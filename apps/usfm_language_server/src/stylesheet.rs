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
//! 2. otherwise a [`PROJECT_STYLESHEET`] file next to the document;
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
use std::str::FromStr;
use std::sync::Arc;

use usfm::StyleSheet;
use usfm::parser::DEFAULT_STYLESHEET;

/// The name Paratext gives a project's stylesheet override.
pub const PROJECT_STYLESHEET: &str = "custom.sty";

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
    /// One entry per path tried; `None` means it failed and has been reported.
    cache: HashMap<PathBuf, Option<Arc<StyleSheet>>>,
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
    pub fn for_document(&mut self, path: Option<&Path>) -> (Arc<StyleSheet>, Option<String>) {
        let Some(sheet_path) = self.path_for(path) else {
            return (Arc::clone(&DEFAULT_STYLESHEET), None);
        };
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

    /// Which `.sty` file, if any, applies to the document at `path`.
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
        let beside = path?.parent()?.join(PROJECT_STYLESHEET);
        beside.is_file().then_some(beside)
    }
}

/// Read one `.sty` file and add its rules to a copy of the default sheet.
fn load(path: &Path) -> Result<Arc<StyleSheet>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("usfm: cannot read the stylesheet {}: {e}", path.display()))?;
    let custom = StyleSheet::from_str(&text).map_err(|e| {
        format!(
            "usfm: cannot parse the stylesheet {}: {e:?}",
            path.display()
        )
    })?;
    let mut extended = (**DEFAULT_STYLESHEET).clone();
    for rule in custom.rules {
        extended.add_rule(rule);
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
        let (sheet, warning) = sheets.for_document(Some(&book(&directory)));
        assert!(Arc::ptr_eq(&sheet, &DEFAULT_STYLESHEET));
        assert_eq!(warning, None);
        // And so does a document with no path at all (an untitled buffer).
        let (sheet, warning) = sheets.for_document(None);
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
        let (sheet, warning) = sheets.for_document(Some(&book(&directory)));
        assert_eq!(warning, None);
        // The project's own marker, and one of the default sheet's: the
        // project sheet extends, it does not replace.
        assert!(sheet.get_rule_by_marker("test").is_some());
        assert!(sheet.get_rule_by_marker("p").is_some());
        // Read once: the second call is the cache.
        let (again, _) = sheets.for_document(Some(&book(&directory)));
        assert!(Arc::ptr_eq(&sheet, &again));
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
        let (sheet, warning) = sheets.for_document(Some(&book(&directory)));
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
        let (sheet, warning) = sheets.for_document(Some(&book(&directory)));
        assert_eq!(warning, None);
        assert!(sheet.get_rule_by_marker("configured").is_some());
    }

    #[test]
    fn a_missing_sheet_warns_once_and_falls_back_to_the_default() {
        let directory = scratch("missing");
        let mut sheets = Stylesheets::default();
        sheets.configure(
            Some(directory.join("nowhere.sty").to_str().unwrap()),
            Some(directory.clone()),
        );

        let (sheet, warning) = sheets.for_document(Some(&book(&directory)));
        assert!(Arc::ptr_eq(&sheet, &DEFAULT_STYLESHEET));
        let warning = warning.expect("a missing stylesheet is reported");
        assert!(warning.contains("nowhere.sty"), "{warning}");

        // The same document again says nothing more: the failure is cached, so
        // a typo in the setting does not warn on every keystroke.
        let (sheet, warning) = sheets.for_document(Some(&book(&directory)));
        assert!(Arc::ptr_eq(&sheet, &DEFAULT_STYLESHEET));
        assert_eq!(warning, None);
    }
}
