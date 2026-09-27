use std::fmt;
use std::path::PathBuf;

use usfm_ast::BookCode;

/// Why a project, or one of its files, could not be read.
#[derive(Debug)]
pub enum Error {
    /// A file could not be read.
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// `Settings.xml` or `BookNames.xml` is not the XML it should be.
    Xml { path: PathBuf, message: String },
    /// A stylesheet could not be parsed.
    Stylesheet { path: PathBuf, message: String },
    /// The project's naming rule gives the book no file name: it is not in
    /// Paratext's canon, or the naming form is unknown.
    NoFileName { code: BookCode },
    /// The books are in a legacy encoding (a Windows code page other than
    /// UTF-8's 65001), which this crate does not decode.
    Encoding { encoding: String },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Error::Xml { path, message } => write!(f, "{}: {message}", path.display()),
            Error::Stylesheet { path, message } => {
                write!(
                    f,
                    "{}: cannot parse the stylesheet: {message}",
                    path.display()
                )
            }
            Error::NoFileName { code } => {
                write!(f, "the project's naming rule gives {code} no file name")
            }
            Error::Encoding { encoding } => write!(
                f,
                "the project's books are in code page {encoding}; only UTF-8 (65001) is read"
            ),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
