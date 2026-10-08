//! A Paratext project folder: what is in it and where (ticket 53).
//!
//! A Paratext project is a directory holding one file per book, a
//! `Settings.xml` that says how those files are named and which books are
//! present, a `BookNames.xml` with each book's names, and the stylesheets.
//! [`Project::open`] reads the two XML files; [`Project::book_path`] and
//! [`Project::read_book`] find a book's file by the project's naming rule, and
//! [`Project::style_sheets`] are the sheets its books are parsed against — the
//! default sheet with the project's `custom.sty` read over it, the way
//! `usfm parse --custom-stylesheet` reads one (ticket 51), and for a
//! peripheral book its `frtbak.sty` over that.
//!
//! Nothing here parses USFM: hand the text and the sheet to
//! `usfm::parse_with`.
//!
//! A project's interlinear glosses (ticket 56) are two more files:
//! [`Project::interlinear`] reads a book's `Interlinear_{language}_{book}.xml`
//! ([`InterlinearBook`]), [`Project::lexicon`] its `Lexicon.xml`
//! ([`Lexicon`]), and [`anchor::anchor`] places a verse's clusters on the
//! words of the verse's text, which is the part a range cannot do alone.
//!
//! ```no_run
//! use usfm_paratext::Project;
//! use usfm_ast::BookCode;
//!
//! let project = Project::open("Paratext/SSV")?;
//! let sheets = project.style_sheets()?;
//! for code in project.books() {
//!     let text = project.read_book(code)?;
//!     // usfm::parse_with(&text, sheets.for_book(code))
//! }
//! # Ok::<(), usfm_paratext::Error>(())
//! ```

pub mod anchor;
pub mod canon;
mod error;
pub mod interlinear;
mod lexicon;
mod names;
mod settings;

use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use usfm_ast::BookCode;
use usfm_style::{DEFAULT_STYLESHEET, StyleSheet};

pub use error::Error;
pub use interlinear::{
    Cluster, ClusterKind, ClusterLexeme, InterlinearBook, InterlinearVerse, LexemeKey, LexemeType,
    Punctuation, TextRange,
};
pub use lexicon::{Gloss, Lexicon, LexiconEntry, Sense, WordAnalysis};
pub use names::{BookName, BookNames};
pub use settings::{BookNameForm, Naming, Settings};

/// The file holding a project's settings, which every project has.
pub const SETTINGS_FILE: &str = "Settings.xml";
/// The file holding the books' names, which a project may leave out.
pub const BOOK_NAMES_FILE: &str = "BookNames.xml";
/// The project's own stylesheet, read over the one `Settings.xml` names.
pub const CUSTOM_STYLESHEET_FILE: &str = "custom.sty";
/// The project's stylesheet for its peripheral books, read over
/// `custom.sty` for those books only; see [`Project::style_sheets`].
pub const FRTBAK_STYLESHEET_FILE: &str = "frtbak.sty";

/// A project's two sheets: [`Project::style_sheets`].
#[derive(Clone)]
pub struct ProjectSheets {
    /// For a book of Scripture: `custom.sty` alone.
    pub main: Arc<StyleSheet>,
    /// For a peripheral book: `custom.sty`, then `frtbak.sty`, which wins.
    pub peripheral: Arc<StyleSheet>,
}

impl ProjectSheets {
    /// The sheet to parse the book `code` against.
    pub fn for_book(&self, code: BookCode) -> &Arc<StyleSheet> {
        if code.is_non_scripture() {
            &self.peripheral
        } else {
            &self.main
        }
    }
}
/// The project's lexicon, which its interlinear glosses point into.
pub const LEXICON_FILE: &str = "Lexicon.xml";

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

    /// The sheets the project's books are parsed against.
    ///
    /// Each starts from the sheet `Settings.xml`'s `StyleSheet` names, when
    /// the project has that file and it is not Paratext's own `usfm.sty`
    /// (which is the built-in sheet), otherwise from the built-in sheet. The
    /// project's `custom.sty` is then read over it for every book — an entry
    /// for a marker the sheet has amends it, a new marker is added — and for
    /// a peripheral book (`FRT`, `INT`, `GLO`, `XXA`, …:
    /// [`BookCode::is_non_scripture`]) its `frtbak.sty` over that, so
    /// `frtbak.sty` wins there where the two disagree. A book of Scripture
    /// is not read against `frtbak.sty` at all: such a file is a whole
    /// sheet for front and back matter, and would amend the standard markers
    /// of the text. A project with no `frtbak.sty` has one sheet for both.
    pub fn style_sheets(&self) -> Result<ProjectSheets, Error> {
        let base = match self.own_style_sheet_path() {
            Some(path) => {
                StyleSheet::from_str(&read_text(&path)?).map_err(|e| Error::Stylesheet {
                    path,
                    message: format!("{e:?}"),
                })?
            }
            None => (**DEFAULT_STYLESHEET).clone(),
        };
        let own = |name: &str| -> Result<Option<(PathBuf, String)>, Error> {
            let path = self.dir.join(name);
            if !path.is_file() {
                return Ok(None);
            }
            let text = read_text(&path)?;
            Ok(Some((path, text)))
        };
        let custom = own(CUSTOM_STYLESHEET_FILE)?;
        let frtbak = own(FRTBAK_STYLESHEET_FILE)?;
        let layered = |order: [&Option<(PathBuf, String)>; 2]| -> Result<Arc<StyleSheet>, Error> {
            let mut sheet = base.clone();
            for (path, text) in order.into_iter().flatten() {
                sheet.extend_from_str(text).map_err(|e| Error::Stylesheet {
                    path: path.clone(),
                    message: format!("{e:?}"),
                })?;
            }
            Ok(Arc::new(sheet))
        };
        let main = layered([&custom, &None])?;
        let peripheral = if frtbak.is_some() {
            layered([&custom, &frtbak])?
        } else {
            Arc::clone(&main)
        };
        Ok(ProjectSheets { main, peripheral })
    }

    /// The sheet a book of Scripture is parsed against:
    /// [`style_sheets`](Self::style_sheets)' `main`. A project with a
    /// `frtbak.sty` has another for its peripheral books, so a caller that
    /// reads those asks `style_sheets` once and
    /// [`ProjectSheets::for_book`] per book.
    pub fn style_sheet(&self) -> Result<Arc<StyleSheet>, Error> {
        Ok(self.style_sheets()?.main)
    }

    /// The gloss languages the project has interlinear files for: each
    /// `Interlinear_{language}` folder, sorted.
    pub fn interlinear_languages(&self) -> Result<Vec<String>, Error> {
        let entries = std::fs::read_dir(&self.dir).map_err(|source| Error::Io {
            path: self.dir.clone(),
            source,
        })?;
        let mut languages: Vec<String> = entries
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_dir())
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                let language = name.strip_prefix(INTERLINEAR_PREFIX)?;
                (!language.is_empty()).then(|| language.to_string())
            })
            .collect();
        languages.sort();
        Ok(languages)
    }

    /// Where the book's interlinear file for `language` is: Paratext 9's
    /// `Interlinear_{language}/Interlinear_{language}_{book}.xml`, or, when
    /// only that exists, the same name at the project's top level, where
    /// older projects keep it. `None` when there is neither.
    pub fn interlinear_path(&self, language: &str, code: BookCode) -> Option<PathBuf> {
        let name = format!("{INTERLINEAR_PREFIX}{language}_{code}.xml");
        [
            self.dir
                .join(format!("{INTERLINEAR_PREFIX}{language}"))
                .join(&name),
            self.dir.join(&name),
        ]
        .into_iter()
        .find(|path| path.is_file())
    }

    /// The book's interlinear glosses in `language`; `None` when the project
    /// has none.
    pub fn interlinear(
        &self,
        language: &str,
        code: BookCode,
    ) -> Result<Option<InterlinearBook>, Error> {
        let Some(path) = self.interlinear_path(language, code) else {
            return Ok(None);
        };
        InterlinearBook::from_xml(&read_text(&path)?)
            .map(Some)
            .map_err(|message| Error::Xml { path, message })
    }

    /// The project's `Lexicon.xml`; `None` when it has none.
    pub fn lexicon(&self) -> Result<Option<Lexicon>, Error> {
        let path = self.dir.join(LEXICON_FILE);
        if !path.is_file() {
            return Ok(None);
        }
        Lexicon::from_xml(&read_text(&path)?)
            .map(Some)
            .map_err(|message| Error::Xml { path, message })
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

/// The start of an interlinear folder's and file's name.
const INTERLINEAR_PREFIX: &str = "Interlinear_";

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
