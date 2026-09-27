//! A Paratext project folder: what is in it and where (ticket 53).
//!
//! A Paratext project is a directory holding one file per book, a
//! `Settings.xml` that says how those files are named and which books are
//! present, a `BookNames.xml` with each book's names, and the stylesheets.
//! [`Project::open`] reads the two XML files; [`Project::book_path`] and
//! [`Project::read_book`] find a book's file by the project's naming rule, and
//! [`Project::style_sheet`] is the sheet its books are parsed against — the
//! default sheet with the project's `custom.sty` read over it, the way the
//! language server and `usfm parse --custom-stylesheet` read one (ticket 51).
//!
//! Nothing here parses USFM: hand the text and the sheet to
//! `usfm::parse_with`.
//!
//! ```no_run
//! use usfm_paratext::Project;
//! use usfm_ast::BookCode;
//!
//! let project = Project::open("Paratext/SSV")?;
//! let sheet = project.style_sheet()?;
//! for code in project.books() {
//!     let text = project.read_book(code)?;
//!     // usfm::parse_with(&text, &sheet)
//! }
//! # Ok::<(), usfm_paratext::Error>(())
//! ```

pub mod canon;
mod error;
mod names;
mod settings;

use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use usfm_ast::BookCode;
use usfm_style::{DEFAULT_STYLESHEET, StyleSheet};

pub use error::Error;
pub use names::{BookName, BookNames};
pub use settings::{BookNameForm, Naming, Settings};

/// The file holding a project's settings, which every project has.
pub const SETTINGS_FILE: &str = "Settings.xml";
/// The file holding the books' names, which a project may leave out.
pub const BOOK_NAMES_FILE: &str = "BookNames.xml";
/// The project's own stylesheet, read over the one `Settings.xml` names.
pub const CUSTOM_STYLESHEET_FILE: &str = "custom.sty";

/// One Paratext project, read from its folder.
#[derive(Debug, Clone)]
pub struct Project {
    dir: PathBuf,
    settings: Settings,
    book_names: BookNames,
}

impl Project {
    /// Read the project in `dir`: its `Settings.xml`, which must be there,
    /// and its `BookNames.xml` if it has one.
    pub fn open(dir: impl AsRef<Path>) -> Result<Self, Error> {
        let dir = dir.as_ref().to_path_buf();
        let settings_path = dir.join(SETTINGS_FILE);
        let settings =
            Settings::from_xml(&read_text(&settings_path)?).map_err(|message| Error::Xml {
                path: settings_path,
                message,
            })?;

        let names_path = dir.join(BOOK_NAMES_FILE);
        let book_names = if names_path.is_file() {
            BookNames::from_xml(&read_text(&names_path)?).map_err(|message| Error::Xml {
                path: names_path,
                message,
            })?
        } else {
            BookNames::default()
        };

        Ok(Self {
            dir,
            settings,
            book_names,
        })
    }

    /// The folder the project was read from.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// The books' names from `BookNames.xml`; empty if the project has none.
    pub fn book_names(&self) -> &BookNames {
        &self.book_names
    }

    /// The books `Settings.xml`'s `BooksPresent` marks as present, in
    /// Paratext's canon order.
    pub fn books(&self) -> Vec<BookCode> {
        self.settings.books_present()
    }

    /// Where the book's file is by the project's naming rule, whether or not
    /// the file exists; `None` when the book is not in Paratext's canon or the
    /// naming form is one this crate does not know.
    pub fn book_path(&self, code: BookCode) -> Option<PathBuf> {
        self.settings
            .naming()
            .file_name(code)
            .map(|name| self.dir.join(name))
    }

    /// The text of the book's file, with a leading byte-order mark removed.
    /// Paratext writes UTF-8 unless `Settings.xml`'s `Encoding` says
    /// otherwise; a project in a legacy encoding is reported as
    /// [`Error::Encoding`] rather than decoded wrongly.
    pub fn read_book(&self, code: BookCode) -> Result<String, Error> {
        if let Some(encoding) = self.settings.encoding()
            && encoding != UTF8_CODE_PAGE
        {
            return Err(Error::Encoding {
                encoding: encoding.to_string(),
            });
        }
        let path = self.book_path(code).ok_or(Error::NoFileName { code })?;
        read_text(&path)
    }

    /// The sheet the project's books are parsed against: the sheet
    /// `Settings.xml`'s `StyleSheet` names when the project has that file and
    /// it is not Paratext's own `usfm.sty` (which is the built-in sheet),
    /// otherwise the built-in sheet; then `custom.sty`, if the project has
    /// one, read over it — an entry for a marker the sheet has amends it, a
    /// new marker is added.
    pub fn style_sheet(&self) -> Result<Arc<StyleSheet>, Error> {
        let mut sheet = match self.own_style_sheet_path() {
            Some(path) => {
                StyleSheet::from_str(&read_text(&path)?).map_err(|e| Error::Stylesheet {
                    path,
                    message: format!("{e:?}"),
                })?
            }
            None => (**DEFAULT_STYLESHEET).clone(),
        };
        let custom = self.dir.join(CUSTOM_STYLESHEET_FILE);
        if custom.is_file() {
            sheet
                .extend_from_str(&read_text(&custom)?)
                .map_err(|e| Error::Stylesheet {
                    path: custom,
                    message: format!("{e:?}"),
                })?;
        }
        Ok(Arc::new(sheet))
    }

    /// The project's own base sheet, if it names one other than Paratext's
    /// and ships it.
    fn own_style_sheet_path(&self) -> Option<PathBuf> {
        let name = self.settings.style_sheet()?;
        if BUILT_IN_SHEETS
            .iter()
            .any(|built_in| name.eq_ignore_ascii_case(built_in))
        {
            return None;
        }
        let path = self.dir.join(name);
        path.is_file().then_some(path)
    }
}

/// `Settings.xml`'s `Encoding` for UTF-8 (a Windows code page number).
const UTF8_CODE_PAGE: &str = "65001";

/// The names under which Paratext ships the sheet `DEFAULT_STYLESHEET` is
/// (`usfm_sb.sty` is the same sheet with study-Bible markers, which the
/// built-in one is built from).
const BUILT_IN_SHEETS: [&str; 2] = ["usfm.sty", "usfm_sb.sty"];

/// A file's text, its byte-order mark removed.
fn read_text(path: &Path) -> Result<String, Error> {
    let text = std::fs::read_to_string(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(match text.strip_prefix('\u{feff}') {
        Some(rest) => rest.to_string(),
        None => text,
    })
}
