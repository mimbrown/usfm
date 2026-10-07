//! The project's `usfm.toml` for an open document.
//!
//! The nearest one to the file, in its directory or one above
//! (`usfm::config::Config::discover`), read each time it is asked for: the
//! file is a few lines, and an edit to it then applies to the next change
//! without restarting the server. Its `[lint]` levels are applied to every
//! list of diagnostics before the server publishes or acts on it, so the
//! editor shows what `usfm parse` prints.
//!
//! A file that cannot be used never stops the server: the document gets the
//! defaults, and the reason is returned to be shown — once per distinct
//! message, not once per keystroke.

use std::path::Path;

use usfm::config::{Config, Lint};

/// What the server has already said about a `usfm.toml`.
#[derive(Default)]
pub struct Configs {
    warned: Option<String>,
}

impl Configs {
    /// The levels for the document at `path`, and a warning to show if its
    /// `usfm.toml` could not be used and that has not been said yet. A
    /// document with no file path (an unsaved buffer) has no project.
    pub fn for_document(&mut self, path: Option<&Path>) -> (Lint, Option<String>) {
        let Some(directory) = path.and_then(Path::parent) else {
            return (Lint::default(), None);
        };
        match Config::discover(directory) {
            Ok(config) => {
                self.warned = None;
                (config.lint, None)
            }
            Err(error) => {
                let message = format!("{error}; reporting every diagnostic at its own level");
                let news = self.warned.as_deref() != Some(message.as_str());
                self.warned = Some(message.clone());
                (Lint::default(), news.then_some(message))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usfm::Code;
    use usfm::config::{FILE_NAME, Level};

    #[test]
    fn the_nearest_usfm_toml_gives_the_levels_and_a_bad_one_warns_once() {
        let root = std::env::temp_dir().join(format!("usfm-lsp-config-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let book = root.join("book.usfm");
        let mut configs = Configs::default();

        assert!(configs.for_document(None).0.is_empty());

        std::fs::write(root.join(FILE_NAME), "[lint.rules]\nmissing-id = \"off\"\n").unwrap();
        let (lint, warning) = configs.for_document(Some(&book));
        assert_eq!(lint.level(Code::MissingId), Some(Level::Off));
        assert_eq!(warning, None);

        std::fs::write(root.join(FILE_NAME), "[lint.rules]\nnot-a-code = \"off\"\n").unwrap();
        let (lint, warning) = configs.for_document(Some(&book));
        assert!(lint.is_empty());
        assert!(warning.is_some_and(|w| w.contains("not-a-code")));
        assert_eq!(configs.for_document(Some(&book)).1, None, "said once");

        std::fs::remove_dir_all(&root).unwrap();
    }
}
