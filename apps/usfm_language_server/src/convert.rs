//! From the toolchain's positions and diagnostics to the protocol's.
//!
//! Two things are converted here and nothing else: a [`Span`] (a byte range in
//! the source) becomes an LSP [`Range`] (a pair of line/character positions),
//! and a [`usfm::Diagnostic`] becomes an LSP [`LspDiagnostic`]. Both are pure
//! functions over a [`LineIndex`], so they are tested without a server and
//! reused by every later feature that has to point at a place in a file
//! (tickets 31 and 32).

use tower_lsp_server::ls_types::{
    Diagnostic as LspDiagnostic, DiagnosticSeverity, NumberOrString, Position, Range,
};

use usfm::diagnostics::{Diagnostic, Severity};
use usfm::span::{LineIndex, Span};

/// The source this server publishes diagnostics under, so an editor can tell
/// ours from another extension's.
pub const SOURCE: &str = "usfm";

/// One byte offset as a protocol [`Position`].
///
/// `LineIndex` counts from 1 and the protocol counts from 0, and the column is
/// in UTF-16 code units because that is the protocol's default encoding — the
/// one [`crate::Backend::initialize`] advertises. An offset past the end of
/// the source clamps to the last position rather than panicking, which is what
/// keeps a diagnostic on a synthesized node (`usfm_span::SPAN`, the empty span
/// at 0) and one at end of file from ever taking the server down.
pub fn position(index: &LineIndex, offset: u32) -> Position {
    let (line, column) = index.line_col_utf16(offset);
    Position::new(line as u32 - 1, column as u32 - 1)
}

/// One span as a protocol [`Range`].
pub fn range(index: &LineIndex, span: Span) -> Range {
    Range::new(position(index, span.start), position(index, span.end))
}

/// The severity mapping, one to one.
///
/// `usfm_diagnostics::Severity` has exactly three levels and each has an
/// obvious counterpart; there is no `Hint` on our side to map, and nothing is
/// ever published without a severity.
pub fn severity(severity: Severity) -> DiagnosticSeverity {
    match severity {
        Severity::Error => DiagnosticSeverity::ERROR,
        Severity::Warning => DiagnosticSeverity::WARNING,
        Severity::Info => DiagnosticSeverity::INFORMATION,
    }
}

/// One diagnostic as the protocol's.
///
/// The `code` is the kebab-case name of the [`usfm::Code`]
/// (`Code`'s `Display`), which is the same string
/// `usfm parse --diagnostics json` writes and the same one a later code action
/// will match on. `tags` stay empty: none of the codes is a deprecation or an
/// unused-code marker, the only two the protocol defines.
pub fn diagnostic(index: &LineIndex, diagnostic: &Diagnostic) -> LspDiagnostic {
    LspDiagnostic {
        range: range(index, diagnostic.span),
        severity: Some(severity(diagnostic.severity)),
        code: Some(NumberOrString::String(diagnostic.code.to_string())),
        code_description: None,
        source: Some(SOURCE.to_owned()),
        message: diagnostic.message.clone(),
        related_information: None,
        tags: None,
        data: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use usfm::Code;

    #[test]
    fn a_position_is_zero_based_and_counted_in_utf16_units() {
        let source = "\\id GEN\n\\v 1 é😀 text\n";
        let index = LineIndex::new(source);
        // The first byte of the file.
        assert_eq!(position(&index, 0), Position::new(0, 0));
        // The `\v` of the second line.
        assert_eq!(position(&index, 8), Position::new(1, 0));
        // `é` is at character 5 of that line (0-based) and two bytes long; the
        // emoji after it is one character but two UTF-16 units, so `text`
        // starts at 5 + 1 + 2 + 1 = 9.
        let text = source.find("text").unwrap() as u32;
        assert_eq!(position(&index, text), Position::new(1, 9));
        // Past the end of the source clamps to the last position.
        assert_eq!(position(&index, 9_999), Position::new(2, 0));
    }

    #[test]
    fn a_range_is_the_span_with_non_ascii_text_before_it() {
        // The span of `\zzz` in a line whose earlier text is multi-byte: the
        // protocol wants code units, so `é` (2 bytes) is one and `😀`
        // (4 bytes) is two.
        let source = "\\id GEN\n\\p\n\\v 1 é😀 \\zzz more\n";
        let index = LineIndex::new(source);
        let start = source.find("\\zzz").unwrap() as u32;
        let span = Span::new(start, start + 4);
        // `\v 1 ` is five units, `é` one, `😀` two and the space one, so the
        // marker starts at unit 9 — where counting characters would say 8.
        assert_eq!(
            range(&index, span),
            Range::new(Position::new(2, 9), Position::new(2, 13)),
        );
        assert_eq!(index.line_col(span.start), (3, 9));
    }

    #[test]
    fn a_diagnostic_carries_its_kebab_case_code_and_our_source() {
        let source = "\\id GEN\n\\p\n\\v 1 é \\zzz more\n";
        let index = LineIndex::new(source);
        let start = source.find("\\zzz").unwrap() as u32;
        let converted = diagnostic(
            &index,
            &Diagnostic::new(
                Code::UnknownMarker,
                Span::new(start, start + 4),
                "unknown marker `\\zzz`",
            ),
        );
        assert_eq!(
            converted.code,
            Some(NumberOrString::String("unknown-marker".to_owned()))
        );
        assert_eq!(converted.source.as_deref(), Some("usfm"));
        assert_eq!(converted.severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(converted.message, "unknown marker `\\zzz`");
        assert_eq!(converted.tags, None);
        // The `é` before it is one code unit, so `\zzz` starts at 7.
        assert_eq!(
            converted.range,
            Range::new(Position::new(2, 7), Position::new(2, 11)),
        );
    }

    #[test]
    fn every_severity_maps_to_one_of_the_protocol_s() {
        assert_eq!(severity(Severity::Error), DiagnosticSeverity::ERROR);
        assert_eq!(severity(Severity::Warning), DiagnosticSeverity::WARNING);
        assert_eq!(severity(Severity::Info), DiagnosticSeverity::INFORMATION);
    }

    #[test]
    fn a_synthesized_span_lands_at_the_start_of_the_file() {
        // `usfm_span::SPAN`, the span a node with no source carries.
        let index = LineIndex::new("\\id GEN\n");
        assert_eq!(
            range(&index, usfm::span::SPAN),
            Range::new(Position::new(0, 0), Position::new(0, 0)),
        );
    }
}
