//! From the toolchain's positions and diagnostics to the protocol's.
//!
//! Positions go both ways here and nothing else happens: a [`Span`] (a byte
//! range in the source) becomes an LSP [`Range`] (a pair of line/character
//! positions), an LSP [`Position`] becomes a byte [`offset`] again — which is
//! how a request that arrives with a position finds the node it is about — and
//! a [`usfm::Diagnostic`] becomes an LSP [`LspDiagnostic`]. All of them are
//! pure functions over the source and its [`LineIndex`], so they are tested
//! without a server and reused by every feature that has to point at a place
//! in a file (hover here, and ticket 32's).

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

/// The whole of `source` as a protocol [`Range`], from `0:0` to the position
/// after its last character.
///
/// What a formatting edit that replaces the document covers (ticket 31). The
/// end is past the last line's last character, and on a source ending in a
/// newline that is the start of the empty line after it — which is the
/// position the editor puts the caret at, and the only end that leaves nothing
/// behind.
pub fn whole_document(index: &LineIndex, source: &str) -> Range {
    Range::new(Position::new(0, 0), position(index, source.len() as u32))
}

/// One protocol [`Position`] as a byte offset into `source`: the inverse of
/// [`position`].
///
/// A request arrives with a position and the server answers about the tree,
/// whose nodes carry byte offsets, so every position-taking request — hover
/// here, and the requests of ticket 32 — starts by coming back this way.
///
/// It is the inverse where it can be. Where it cannot, it clamps, because a
/// client is allowed to send a position that is not in the document and a
/// server may not fall over on one:
///
/// * a line past the last is the end of the source;
/// * a character past the end of its line is the end of that line, *before*
///   its line break, so a click past the text of a line stays on that line;
/// * a character that lands inside a surrogate pair — inside an emoji, whose
///   UTF-16 length is 2 — is that character's own start, the same rounding
///   [`LineIndex::line_col_utf16`] does in the other direction.
///
/// `source` rather than the [`LineIndex`], which keeps line starts but hands
/// out neither them nor the text: the scan is over one line once per request.
pub fn offset(source: &str, position: Position) -> u32 {
    // The start of the wanted line, or the end of the source when there are
    // fewer lines than that.
    let mut start = 0;
    for _ in 0..position.line {
        match source[start..].find('\n') {
            Some(break_at) => start += break_at + 1,
            None => return source.len() as u32,
        }
    }

    // Its text, without the line break: a character past the end of a line
    // clamps to the end of *that* line rather than running into the next.
    let line = &source[start..];
    let line = match line.find('\n') {
        Some(end) => &line[..end],
        None => line,
    };

    let mut units = 0;
    for (byte, character) in line.char_indices() {
        if units >= position.character {
            return (start + byte) as u32;
        }
        units += character.len_utf16() as u32;
        if units > position.character {
            // The position fell *inside* this character — between the halves
            // of a surrogate pair — so it is this character's own start.
            return (start + byte) as u32;
        }
    }
    (start + line.len()) as u32
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
    fn an_offset_is_the_inverse_of_a_position() {
        let source = "\\id GEN\n\\v 1 é😀 text\n";
        let index = LineIndex::new(source);
        assert_eq!(offset(source, Position::new(0, 0)), 0);
        // The start of the second line, which is byte 8.
        assert_eq!(offset(source, Position::new(1, 0)), 8);
        // `text` is at unit 9 of that line: `\v 1 ` is five, `é` one and the
        // emoji two, then the space.
        let text = source.find("text").unwrap() as u32;
        assert_eq!(offset(source, Position::new(1, 9)), text);
        // Every offset in the source comes back from its own position.
        for byte in 0..=source.len() {
            if !source.is_char_boundary(byte) {
                continue;
            }
            let byte = byte as u32;
            assert_eq!(offset(source, position(&index, byte)), byte, "byte {byte}");
        }
    }

    #[test]
    fn a_position_outside_the_document_clamps() {
        let source = "\\id GEN\n\\p text\n";
        // A character past the end of its line is the end of that line,
        // before the break: `\p text` is seven units long, and byte 8 + 7 is
        // where the line's `\n` sits.
        assert_eq!(offset(source, Position::new(1, 7)), 15);
        assert_eq!(offset(source, Position::new(1, 99)), 15);
        assert_eq!(&source[15..], "\n");
        // A line past the last is the end of the source, whatever the column
        // says. (Line 2 is the empty line after the trailing break.)
        assert_eq!(offset(source, Position::new(2, 0)), source.len() as u32);
        assert_eq!(offset(source, Position::new(2, 4)), source.len() as u32);
        assert_eq!(offset(source, Position::new(99, 0)), source.len() as u32);
        // An empty source has one position and one offset.
        assert_eq!(offset("", Position::new(0, 0)), 0);
        assert_eq!(offset("", Position::new(7, 7)), 0);
    }

    #[test]
    fn a_position_inside_a_surrogate_pair_rounds_to_its_character() {
        // `😀` is two UTF-16 units; a position between them is the emoji's
        // own start, which is how `line_col_utf16` rounds the other way.
        let source = "a😀b";
        assert_eq!(offset(source, Position::new(0, 1)), 1);
        assert_eq!(offset(source, Position::new(0, 2)), 1);
        assert_eq!(offset(source, Position::new(0, 3)), 5);
    }

    #[test]
    fn the_whole_document_range_ends_where_the_text_does() {
        let source = "\\id GEN\n\\p text\n";
        assert_eq!(
            whole_document(&LineIndex::new(source), source),
            // The empty line after the trailing break.
            Range::new(Position::new(0, 0), Position::new(2, 0)),
        );

        // With no trailing break the end is past the last character.
        let source = "\\id GEN";
        assert_eq!(
            whole_document(&LineIndex::new(source), source),
            Range::new(Position::new(0, 0), Position::new(0, 7)),
        );

        let source = "";
        assert_eq!(
            whole_document(&LineIndex::new(source), source),
            Range::new(Position::new(0, 0), Position::new(0, 0)),
        );
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
