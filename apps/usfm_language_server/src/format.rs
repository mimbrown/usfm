//! `textDocument/formatting`: the document, written back out by
//! `usfm_codegen` (ticket 31).
//!
//! The same formatter as `usfm format` (`apps/usfm_cli/src/format.rs`), down
//! to the trailing newline, so that formatting on save in the editor and
//! `usfm format --write` in a script leave the same bytes and neither one
//! keeps reformatting what the other wrote. There are no options: the writer's
//! spelling of each construct is the formatted shape, and the document is
//! replaced whole.
//!
//! # When it refuses
//!
//! [`refusal`] is `usfm format --write`'s rule without `--force`: a parse that
//! reported an Error was repaired, and the repaired tree is not the author's
//! text — a marker the parser dropped would be gone from the file, silently,
//! on a save. The CLI has `--force` for the times that is wanted; the editor
//! has no such flag, so the server says why and leaves the file alone.

use usfm::ast::Document;
use usfm::diagnostics::{Diagnostic, Severity};

/// The canonical USFM text of a document, ending with one newline.
///
/// The writer ends every block with one, so the push only fires for a document
/// with no blocks at all — an empty file, which formats to an empty file
/// rather than to a lone newline. This is `usfm_cli::format::formatted`, and
/// the two are the same text on purpose.
pub fn formatted(document: &Document<'_>) -> String {
    let mut text = usfm::codegen::to_usfm_string(document);
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text
}

/// Why the document must not be formatted, as the sentence to show, or `None`
/// when it may be.
pub fn refusal(diagnostics: &[Diagnostic]) -> Option<String> {
    let errors = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity >= Severity::Error)
        .count();
    if errors == 0 {
        return None;
    }
    // The count and the first message: the Problems panel has the rest, and a
    // notification that listed them all would be unreadable.
    let first = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.severity >= Severity::Error)
        .expect("there is at least one error");
    let plural = if errors == 1 { "" } else { "s" };
    Some(format!(
        "usfm: not formatted — the file has {errors} error{plural} \
         and formatting it would write the repaired text over yours \
         (first: {}). Fix them, or run `usfm format --write --force`.",
        first.message,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clean_document_is_formatted_and_ends_with_one_newline() {
        let result = usfm::parse("\\id GEN\n\\c 1\n\\p \\v 1 a\n");
        assert_eq!(refusal(&result.diagnostics), None);
        let text = formatted(&result.document);
        assert!(text.ends_with("\\p \\v 1 a\n"), "{text:?}");
        assert!(!text.ends_with("\n\n"), "{text:?}");
    }

    /// The refusal is the CLI's rule: an Error, and only an Error, stops it.
    #[test]
    fn an_error_refuses_and_a_warning_does_not() {
        // `\qqq` is in no stylesheet: `unknown-marker`, an Error, and the
        // marker is dropped — which is exactly the edit a save must not make.
        let result = usfm::parse("\\id GEN\n\\c 1\n\\p \\v 1 text \\qqq more\n");
        let message = refusal(&result.diagnostics).expect("a refusal");
        assert!(message.contains("not formatted"), "{message}");
        assert!(message.contains("\\qqq"), "{message}");
        assert!(message.contains("1 error"), "{message}");
        // And the repair is real: the marker is not in the formatted text.
        assert!(!formatted(&result.document).contains("\\qqq"));

        // `\zqq` is the custom-marker namespace: `unknown-custom-marker`, a
        // Warning, so the file is formatted.
        let result = usfm::parse("\\id GEN\n\\c 1\n\\p \\v 1 text \\zqq more\n");
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity == Severity::Warning),
        );
        assert_eq!(refusal(&result.diagnostics), None);
    }

    /// More than one error is counted, and the sentence stays one sentence.
    #[test]
    fn several_errors_are_counted() {
        let result = usfm::parse("\\id GEN\n\\c 1\n\\p \\v 1 a \\qqq b \\www c\n");
        let message = refusal(&result.diagnostics).expect("a refusal");
        assert!(message.contains("2 errors"), "{message}");
    }

    /// Formatting is a fixed point, through the trailing newline this module
    /// adds: what the editor saves is what a second save would write.
    #[test]
    fn formatting_is_a_fixed_point() {
        let once = formatted(&usfm::parse("\\id GEN\n\\c 1\n\\p \\v 1 a \\nd b\n").document);
        let twice = formatted(&usfm::parse(&once).document);
        assert_eq!(once, twice);
    }
}
