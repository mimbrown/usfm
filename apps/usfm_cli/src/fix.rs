//! `usfm fix`: apply the fixes `usfm_fix` has for what `usfm parse` reports.
//!
//! One file at a time, like [`crate::format`] and for its reason: a fix is
//! an edit to a file, and a book split across three files is fixed as three.
//! The fixes themselves are `usfm_fix`'s — the same ones the language server
//! offers in an editor — and this module only decides which to ask for,
//! applies them, and says what it did.
//!
//! # A dry run unless `--write`
//!
//! Without `--write` nothing is written. Each fix is listed on standard
//! output as `file:line:col: fix[code]: title`, where the position is the
//! diagnostic's, and a count goes to standard error. With `--write` the file
//! is fixed in place and the same lines say what was done.
//!
//! # Which fixes
//!
//! All of them, unless `--code` names the ones wanted. A `--code` that is
//! not a diagnostic code, or is one with no fix, is a usage error that lists
//! the codes that have one: a typo must not look like a clean run.
//!
//! # Until nothing is left
//!
//! A fix can uncover another diagnostic, and two fixes whose edits touch are
//! not applied in one pass. So the file is parsed again after each pass and
//! asked again, to a fixed point. The dry run walks the same passes in
//! memory, so what it lists is what `--write` would do. A file whose fixes
//! do not settle in [`MAX_PASSES`] is reported and left as it was.
//!
//! # Exit codes
//!
//! 0 when every file was read (and, with `--write`, written); 1 when one
//! could not be, or did not settle. Fixes being available is not a failure.

use std::sync::Arc;

use usfm::Code;
use usfm::fix::{FIXABLE, Fix, is_fixable};
use usfm::parser::DEFAULT_STYLESHEET;
use usfm::span::LineIndex;
use usfm::style::StyleSheet;

use crate::args::FixArgs;
use crate::driver::{extended_sheet, read_source, read_stylesheet};
use crate::error::Error;

/// More passes than any real file needs: each pass applies every fix that
/// does not touch another, so the count is the depth of fixes uncovering
/// fixes, not the number of fixes.
const MAX_PASSES: usize = 16;

/// Fix, or list the fixes for, every file `args` names.
pub fn run(args: &FixArgs) -> Result<(), Error> {
    let only = wanted_codes(&args.code)?;
    let style_sheet = style_sheet(args)?;

    let mut failed = false;
    let (mut total, mut files_with_fixes) = (0, 0);
    for path in &args.files {
        let label = path.display().to_string();
        let source = match read_source(path) {
            Ok(source) => source,
            Err(e) => {
                eprintln!("Error: {label}: {}", message(&e));
                failed = true;
                continue;
            }
        };
        let Some(fixed) = fix_source(&source, &style_sheet, only.as_deref()) else {
            eprintln!("Error: {label}: the fixes did not settle; left as it was");
            failed = true;
            continue;
        };
        for line in &fixed.lines {
            println!("{label}:{line}");
        }
        if fixed.lines.is_empty() {
            continue;
        }
        total += fixed.lines.len();
        files_with_fixes += 1;
        if args.write
            && let Err(e) = std::fs::write(path, &fixed.text)
        {
            eprintln!("Error: {label}: {e}");
            failed = true;
        }
    }

    let files = if files_with_fixes == 1 {
        "file"
    } else {
        "files"
    };
    let fixes = if total == 1 { "fix" } else { "fixes" };
    if total == 0 {
        eprintln!("nothing to fix");
    } else if args.write {
        eprintln!("applied {total} {fixes} in {files_with_fixes} {files}");
    } else {
        eprintln!(
            "{total} {fixes} available in {files_with_fixes} {files}; nothing written (pass --write to apply)"
        );
    }
    if failed { Err(Error::Consumed) } else { Ok(()) }
}

/// A file's text with its fixes applied, and a line for each.
struct Fixed {
    text: String,
    /// `line:col: fix[code]: title`, in the order applied. Positions are in
    /// the text of the pass the fix was found in, which for the first pass —
    /// nearly all of them — is the file as it stands.
    lines: Vec<String>,
}

/// Apply fixes to `source` until none is left. `None` if that does not
/// happen in [`MAX_PASSES`].
fn fix_source(source: &str, style_sheet: &Arc<StyleSheet>, only: Option<&[Code]>) -> Option<Fixed> {
    let mut text = source.to_owned();
    let mut lines = Vec::new();
    for _ in 0..MAX_PASSES {
        let result = usfm::parse_with(&text, style_sheet);
        let fixes = usfm::fix::fixes(&result.document, &text, &result.diagnostics, only);
        if fixes.is_empty() {
            return Some(Fixed { text, lines });
        }
        let (next, taken) = usfm::fix::apply(&text, &fixes);
        if taken.is_empty() {
            return None;
        }
        let index = LineIndex::new(&text);
        lines.extend(taken.iter().map(|&at| line(&index, &fixes[at])));
        // The source is dropped before `text` is replaced: the parse borrows
        // it.
        drop(result);
        text = next;
    }
    None
}

fn line(index: &LineIndex, fix: &Fix) -> String {
    let (line, col) = index.line_col(fix.span.start);
    format!("{line}:{col}: fix[{}]: {}", fix.code, fix.title)
}

/// `--code` as codes: `None` for all of them.
fn wanted_codes(names: &[String]) -> Result<Option<Vec<Code>>, Error> {
    if names.is_empty() {
        return Ok(None);
    }
    let fixable = || {
        FIXABLE
            .iter()
            .map(Code::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    };
    names
        .iter()
        .map(|name| match name.parse::<Code>() {
            Ok(code) if is_fixable(code) => Ok(code),
            Ok(code) => Err(Error::Custom(format!(
                "`{code}` has no fix; the codes that have one are: {}",
                fixable()
            ))),
            Err(_) => Err(Error::Custom(format!(
                "`{name}` is not a diagnostic code; the codes that have a fix are: {}",
                fixable()
            ))),
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

/// The sheet to parse against, as `format` builds it; a sheet that was named
/// and cannot be read is fatal, since every file would be fixed against the
/// wrong one.
fn style_sheet(args: &FixArgs) -> Result<Arc<StyleSheet>, Error> {
    let named = |path: &std::path::Path, e: &Error| {
        Error::Custom(format!("{}: {}", path.display(), message(e)))
    };
    let base = match args.stylesheet.as_deref() {
        Some(path) => read_stylesheet(path)
            .map_err(|e| named(path, &e))?
            .unwrap_or_else(|| Arc::clone(&DEFAULT_STYLESHEET)),
        None => Arc::clone(&DEFAULT_STYLESHEET),
    };
    match args.custom_stylesheet.as_deref() {
        Some(path) => {
            let custom = read_source(path).map_err(|e| named(path, &e))?;
            extended_sheet(base, Some(&custom)).map_err(|e| named(path, &e))
        }
        None => Ok(base),
    }
}

fn message(error: &Error) -> String {
    match error {
        Error::Io(e) => e.to_string(),
        Error::Custom(e) => e.clone(),
        Error::Consumed => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed(source: &str, only: Option<&[Code]>) -> Fixed {
        fix_source(source, &DEFAULT_STYLESHEET, only).expect("the fixes settle")
    }

    /// Fixes whose edits touch take a pass each, and the lines list both.
    #[test]
    fn a_file_is_fixed_until_nothing_is_left() {
        let result = fixed("\\id GEN\n\\c 1\n\\p \\v 1 a \\foo \\bar b\n", None);
        assert_eq!(result.text, "\\id GEN\n\\c 1\n\\p \\v 1 a b\n");
        assert_eq!(
            result.lines,
            [
                "3:11: fix[unknown-marker]: Delete `\\foo`",
                "3:11: fix[unknown-marker]: Delete `\\bar`",
            ]
        );
    }

    #[test]
    fn a_clean_file_is_left_alone() {
        let source = "\\id GEN\n\\c 1\n\\p \\v 1 a\n";
        let result = fixed(source, None);
        assert_eq!(result.text, source);
        assert!(result.lines.is_empty());
    }

    #[test]
    fn only_the_named_codes_are_fixed() {
        let source = "\\id GEN\n\\c 1\n\\p \\v 1 a \\foo b \\w word|lemma=grace\\w*\n";
        let result = fixed(source, Some(&[Code::AttributeValueNotQuoted]));
        assert_eq!(
            result.text,
            "\\id GEN\n\\c 1\n\\p \\v 1 a \\foo b \\w word|lemma=\"grace\"\\w*\n"
        );
        assert_eq!(result.lines.len(), 1);
    }

    #[test]
    fn a_code_that_is_unknown_or_has_no_fix_is_refused() {
        assert!(wanted_codes(&[]).unwrap().is_none());
        assert_eq!(
            wanted_codes(&["unknown-marker".to_owned()]).unwrap(),
            Some(vec![Code::UnknownMarker])
        );
        for bad in ["missing-id", "no-such-code"] {
            let Err(Error::Custom(text)) = wanted_codes(&[bad.to_owned()]) else {
                panic!("{bad} was accepted");
            };
            assert!(text.contains("unknown-marker"), "{text}");
        }
    }
}
