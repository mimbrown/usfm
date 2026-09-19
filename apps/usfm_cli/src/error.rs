//! What can go wrong outside the parser: a file that cannot be read, a
//! combination of flags that has no output, a write that fails.
//!
//! Parse diagnostics are not errors: the parser recovers from everything and
//! reports what it did, which `--strict` and `--deny-warnings` may then turn
//! into one of these.

/// An error the CLI reports and exits 1 on.
#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Custom(String),
    /// Already printed: the run failed after one or more errors were reported
    /// as they happened, and there is nothing more to say.
    Consumed,
}

impl Error {
    pub fn print(&self) {
        match self {
            Error::Io(e) => eprintln!("IO Error: {}", e),
            Error::Custom(e) => eprintln!("Error: {}", e),
            Error::Consumed => {}
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<&str> for Error {
    fn from(value: &str) -> Self {
        Self::Custom(value.to_string())
    }
}

impl From<String> for Error {
    fn from(value: String) -> Self {
        Self::Custom(value)
    }
}

impl From<usfm::pipeline::RenderError> for Error {
    fn from(value: usfm::pipeline::RenderError) -> Self {
        Self::Custom(value.to_string())
    }
}
