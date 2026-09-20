//! `usfm format`: the formatter (ticket 26).
//!
//! One file at a time — read, parse with the facade (so the semantic
//! diagnostics print as well as the parser's), write the tree back out with
//! `usfm_codegen`, and then do one of three things with the result: print it,
//! replace the file with it, or compare it. That is the whole command; there
//! is no [`Driver`](crate::driver::Driver) here because a formatter has none
//! of what the driver exists for. The driver concatenates its inputs into one
//! document, applies replacements, renders once and writes once, optionally
//! watching. A formatter must keep the files apart: rewriting a book split
//! across three files has to produce three files, and a file that cannot be
//! read or whose parse is broken must not stop the other two.
//!
//! What is read *is* shared with the driver — [`crate::driver::read_source`],
//! [`crate::driver::read_stylesheet`] and
//! [`crate::driver::print_diagnostics`] are the same three functions the
//! `parse` command uses, so the two commands accept the same stylesheets and
//! file the same diagnostic lines.
//!
//! # The canonical shape
//!
//! The formatted text of a file is `usfm_codegen::to_usfm_string` of its
//! parse, with a trailing newline if the writer did not already end with one
//! (it does for every non-empty document). There is one shape and no options:
//! the writer's spelling of each construct is the formatter's, and
//! `usfm parse --format usfm` writes exactly the same bytes for the same
//! document.
//!
//! # Exit codes
//!
//! 0 when every file was printed, rewritten or found already formatted; 1 when
//! any file could not be read, was left alone because its parse reported an
//! error, or (under `--check`) differs from its formatted text. One bad file
//! does not stop the rest: the run goes on and the code is decided at the end.

use std::io::Write;
use std::sync::Arc;

use usfm::ast::Document;
use usfm::diagnostics::Severity;
use usfm::parser::DEFAULT_STYLESHEET;

use crate::args::FormatArgs;
use crate::driver::{print_diagnostics, read_source, read_stylesheet};
use crate::error::Error;

/// Format every file `args` names.
pub fn run(args: &FormatArgs) -> Result<(), Error> {
    let style_sheet = match args.stylesheet.as_deref() {
        // Fatal, unlike an unreadable input: a sheet that was named and could
        // not be read would silently format every file against the wrong one.
        Some(path) => read_stylesheet(path)
            .map_err(|e| Error::Custom(format!("{}: {}", path.display(), io_message(&e))))?
            .unwrap_or_else(|| Arc::clone(&DEFAULT_STYLESHEET)),
        None => Arc::clone(&DEFAULT_STYLESHEET),
    };

    // Set once by anything that makes the run fail, and read once at the end:
    // every file is looked at whatever the ones before it did.
    let mut failed = false;
    let mut stdout = std::io::stdout().lock();

    for path in &args.files {
        let label = path.display().to_string();
        let source = match read_source(path) {
            Ok(source) => source,
            Err(e) => {
                eprintln!("Error: {label}: {}", io_message(&e));
                failed = true;
                continue;
            }
        };

        // The facade, not the parser: `unlisted-book-code` and the rest of the
        // semantic pass are as much a reason to look at a file as a repair is
        // (ticket 19).
        let result = usfm::parse_with(&source, &style_sheet);
        print_diagnostics(&label, &source, &result.diagnostics, args.diagnostics);
        let has_error = result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity >= Severity::Error);
        let formatted = formatted(&result.document);

        if args.check {
            if formatted != source {
                // The one line `--check` exists to print, on stderr so that a
                // script can keep stdout for something else. Nothing is
                // written either way.
                eprintln!("would reformat {label}");
                failed = true;
            }
        } else if args.write {
            // A parse that reported an error was repaired, and the repaired
            // tree is not the author's text: writing it over the file would
            // turn a mistake into a silent edit. `--force` says to do it
            // anyway.
            if has_error && !args.force {
                eprintln!(
                    "Error: {label}: not rewritten, the parse reported an error \
                     (pass --force to rewrite it anyway)"
                );
                failed = true;
                continue;
            }
            // Only when it differs: a formatted file keeps its timestamp, so a
            // `format --write` over a tree does not rebuild everything.
            if formatted != source
                && let Err(e) = std::fs::write(path, &formatted)
            {
                eprintln!("Error: {label}: {}", io_message(&e.into()));
                failed = true;
            }
        } else if let Err(e) = stdout.write_all(formatted.as_bytes()) {
            // A closed pipe is the usual one, and there is no point going on
            // to the next file with nowhere to write it.
            return Err(Error::from(e));
        }
    }

    if failed { Err(Error::Consumed) } else { Ok(()) }
}

/// The canonical USFM text of a document: what the writer produces, ending
/// with a newline.
///
/// The writer ends every block it writes with one, so the push only fires for
/// a document with no blocks at all — an empty file, which formats to an empty
/// file rather than to a lone newline.
fn formatted(document: &Document<'_>) -> String {
    let mut text = usfm::codegen::to_usfm_string(document);
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text
}

/// The message inside an [`Error`], for a line that already names the file.
fn io_message(error: &Error) -> String {
    match error {
        Error::Io(e) => e.to_string(),
        Error::Custom(e) => e.clone(),
        Error::Consumed => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The formatted text ends with exactly one newline, and an empty document
    /// formats to nothing rather than to a newline.
    #[test]
    fn the_formatted_text_ends_with_one_newline() {
        let result = usfm::parse("\\id GEN\n\\c 1\n\\p \\v 1 a\n");
        let text = formatted(&result.document);
        assert!(text.ends_with("\\p \\v 1 a\n"), "{text:?}");
        assert!(!text.ends_with("\n\n"), "{text:?}");

        assert_eq!(formatted(&usfm::parse("").document), "");
    }

    /// Formatting is idempotent: the formatted text of a formatted document is
    /// itself. (The fixed point is `usfm_codegen`'s property, proven over the
    /// whole conformance corpus in its round-trip test; this is the same
    /// property through the trailing newline this module adds.)
    #[test]
    fn formatting_is_a_fixed_point() {
        let once = formatted(&usfm::parse("\\id GEN\n\\c 1\n\\p \\v 1 a \\nd b\n").document);
        let twice = formatted(&usfm::parse(&once).document);
        assert_eq!(once, twice);
    }
}
