//! `usfm.toml`, a project's configuration file.
//!
//! One file for every tool, a table per kind of setting. Today there is one
//! kind, the diagnostics a project wants reported:
//!
//! ```toml
//! [lint.rules]
//! character-style-not-closed = "off"
//! content-outside-paragraph = "error"
//! ```
//!
//! A rule is a diagnostic code by its kebab-case name, and its level is
//! `"off"`, `"info"`, `"warning"` or `"error"`. A code the file does not name
//! keeps the severity it has. Nothing here changes what the parser does: a
//! rule switched off is repaired exactly as before and not reported.
//!
//! The file is strict about itself. A table, a key, a code or a level it
//! does not know is an error naming it, so a misspelt rule cannot sit in a
//! file doing nothing.
//!
//! A tool finds the file with [`Config::discover`]: the nearest `usfm.toml`
//! in the directory it is given or one above it. [`Lint::apply`] is then the
//! whole effect — the list of diagnostics with the project's levels — and
//! everything a tool does with diagnostics (print them, refuse a run, offer
//! a fix) it does with that list.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use toml::{Table, Value};
use usfm_diagnostics::{Code, Diagnostic, Severity};

/// The file's name, in a project's folder or one above the file being read.
pub const FILE_NAME: &str = "usfm.toml";

/// A `usfm.toml`, read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Config {
    /// `[lint]`: what is reported.
    pub lint: Lint,
}

/// `[lint]`: the level of each rule the file names.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Lint {
    rules: HashMap<Code, Level>,
}

/// What a project wants done with one diagnostic code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// Not reported.
    Off,
    /// Reported at this severity.
    Report(Severity),
}

/// Why a `usfm.toml` could not be used.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigError(String);

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ConfigError {}

impl std::str::FromStr for Config {
    type Err = ConfigError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let table: Table = text
            .parse()
            .map_err(|e: toml::de::Error| ConfigError(e.message().to_string()))?;
        let mut config = Config::default();
        for (key, value) in &table {
            match key.as_str() {
                "lint" => config.lint = Lint::from_value(value)?,
                other => return Err(unknown("table", other, &["lint"])),
            }
        }
        Ok(config)
    }
}

impl Config {
    /// The file at `path`.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| ConfigError(format!("{}: {e}", path.display())))?;
        text.parse()
            .map_err(|ConfigError(message)| ConfigError(format!("{}: {message}", path.display())))
    }

    /// The path of the nearest `usfm.toml`: in `dir`, or in the closest
    /// directory above it that has one.
    pub fn find(dir: &Path) -> Option<PathBuf> {
        dir.ancestors()
            .map(|dir| dir.join(FILE_NAME))
            .find(|path| path.is_file())
    }

    /// The nearest `usfm.toml` to `dir`, read; the default — every rule as
    /// it is — when there is none.
    pub fn discover(dir: &Path) -> Result<Self, ConfigError> {
        match Self::find(dir) {
            Some(path) => Self::load(&path),
            None => Ok(Self::default()),
        }
    }
}

impl Lint {
    fn from_value(value: &Value) -> Result<Self, ConfigError> {
        let mut lint = Lint::default();
        for (key, value) in table(value, "lint")? {
            match key.as_str() {
                "rules" => {
                    for (name, level) in table(value, "lint.rules")? {
                        let code = Code::parse(name).ok_or_else(|| {
                            ConfigError(format!("lint.rules: `{name}` is not a diagnostic code"))
                        })?;
                        lint.rules.insert(code, Level::from_value(name, level)?);
                    }
                }
                other => return Err(unknown("key in [lint]", other, &["rules"])),
            }
        }
        Ok(lint)
    }

    /// The level the file gives `code`, `None` when it does not name it.
    pub fn level(&self, code: Code) -> Option<Level> {
        self.rules.get(&code).copied()
    }

    /// Set `code`'s level, as a line of the file would.
    pub fn set(&mut self, code: Code, level: Level) {
        self.rules.insert(code, level);
    }

    /// Whether the file names no rule, so that [`apply`](Self::apply)
    /// changes nothing.
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// `diagnostics` as the project wants them: a rule that is off is
    /// removed, and one with a level is reported at it. Order is kept.
    pub fn apply(&self, diagnostics: &mut Vec<Diagnostic>) {
        if self.is_empty() {
            return;
        }
        diagnostics.retain_mut(|diagnostic| match self.level(diagnostic.code) {
            Some(Level::Off) => false,
            Some(Level::Report(severity)) => {
                diagnostic.severity = severity;
                true
            }
            None => true,
        });
    }
}

impl Level {
    fn from_value(rule: &str, value: &Value) -> Result<Self, ConfigError> {
        const LEVELS: [&str; 4] = ["off", "info", "warning", "error"];
        match value.as_str() {
            Some("off") => Ok(Level::Off),
            Some("info") => Ok(Level::Report(Severity::Info)),
            Some("warning") => Ok(Level::Report(Severity::Warning)),
            Some("error") => Ok(Level::Report(Severity::Error)),
            _ => Err(ConfigError(format!(
                "lint.rules: the level of `{rule}` is not one of {}",
                quoted(&LEVELS)
            ))),
        }
    }
}

fn table<'v>(value: &'v Value, name: &str) -> Result<&'v Table, ConfigError> {
    value
        .as_table()
        .ok_or_else(|| ConfigError(format!("`{name}` is not a table")))
}

fn unknown(what: &str, name: &str, known: &[&str]) -> ConfigError {
    ConfigError(format!(
        "unknown {what} `{name}` (known: {})",
        quoted(known)
    ))
}

fn quoted(names: &[&str]) -> String {
    names
        .iter()
        .map(|name| format!("\"{name}\""))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use usfm_span::Span;

    fn config(text: &str) -> Config {
        text.parse().unwrap()
    }

    fn error(text: &str) -> String {
        text.parse::<Config>().unwrap_err().to_string()
    }

    fn diagnostics(codes: &[Code]) -> Vec<Diagnostic> {
        codes
            .iter()
            .map(|code| Diagnostic::new(*code, Span::new(0, 0), "message"))
            .collect()
    }

    #[test]
    fn an_empty_file_changes_nothing() {
        let config = config("");
        assert!(config.lint.is_empty());
        let mut list = diagnostics(&[Code::CharacterStyleNotClosed]);
        let before = list.clone();
        config.lint.apply(&mut list);
        assert_eq!(list, before);
    }

    #[test]
    fn a_rule_is_switched_off_or_given_a_level() {
        let config = config(
            "[lint.rules]\n\
             character-style-not-closed = \"off\"\n\
             content-outside-paragraph = \"error\"\n\
             unknown-marker = \"info\"\n",
        );
        let mut list = diagnostics(&[
            Code::ContentOutsideParagraph,
            Code::CharacterStyleNotClosed,
            Code::UnknownMarker,
            Code::MissingId,
        ]);
        config.lint.apply(&mut list);
        let reported: Vec<(Code, Severity)> = list.iter().map(|d| (d.code, d.severity)).collect();
        assert_eq!(
            reported,
            [
                (Code::ContentOutsideParagraph, Severity::Error),
                (Code::UnknownMarker, Severity::Info),
                // Not named: as the toolchain reports it.
                (Code::MissingId, Code::MissingId.severity()),
            ]
        );
    }

    /// A misspelling is refused by name rather than ignored.
    #[test]
    fn what_the_file_does_not_know_is_an_error() {
        assert!(error("[lint.rules]\nno-such-code = \"off\"").contains("`no-such-code`"));
        assert!(error("[lint.rules]\nunknown-marker = \"warn\"").contains("\"warning\""));
        assert!(error("[lint.rules]\nunknown-marker = false").contains("unknown-marker"));
        assert!(error("[lint]\nrule = {}").contains("`rule`"));
        assert!(error("[format]\nwidth = 80").contains("`format`"));
        assert!(error("lint = 1").contains("not a table"));
        assert!(!error("[lint").is_empty());
    }

    #[test]
    fn the_nearest_file_above_is_found() {
        let root = std::env::temp_dir().join(format!("usfm_config_{}", std::process::id()));
        let inner = root.join("a").join("b");
        std::fs::create_dir_all(&inner).unwrap();
        assert_eq!(Config::find(&inner).filter(|p| p.starts_with(&root)), None);
        std::fs::write(root.join(FILE_NAME), "[lint.rules]\nmissing-id = \"off\"\n").unwrap();
        assert_eq!(Config::find(&inner), Some(root.join(FILE_NAME)));
        std::fs::write(root.join("a").join(FILE_NAME), "").unwrap();
        assert_eq!(Config::find(&inner), Some(root.join("a").join(FILE_NAME)));
        assert!(Config::discover(&inner).unwrap().lint.is_empty());
        std::fs::write(root.join("a").join(FILE_NAME), "[lint").unwrap();
        let message = Config::discover(&inner).unwrap_err().to_string();
        assert!(message.contains(FILE_NAME), "{message}");
        std::fs::remove_dir_all(&root).unwrap();
    }
}
