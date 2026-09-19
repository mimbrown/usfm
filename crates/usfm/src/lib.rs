//! The USFM toolchain, behind one name.
//!
//! The workspace is split one job per crate
//! (`docs/adr/0001-oxc-style-crate-layout.md`). This crate re-exports those
//! layers under short module names and adds the two calls a consumer usually
//! wants, so an application depends on `usfm` instead of on five to nine
//! separate crates:
//!
//! | module | crate | what it holds |
//! |---|---|---|
//! | [`span`] | `usfm_span` | [`Span`], `LineIndex` |
//! | [`style`] | `usfm_style` | [`StyleSheet`], `StyleRule`, `StyleId` |
//! | [`ast`] | `usfm_ast` | [`Document`] and its nodes, `Visit`/`VisitMut`/`Fold`, `Cursor`, `ReferenceIndex` |
//! | [`diagnostics`] | `usfm_diagnostics` | [`Diagnostic`], [`Code`], [`Severity`], [`ParseResult`] |
//! | [`parser`] | `usfm_parser` | the lexer and the parser |
//! | [`usx`] | `usfm_usx` | AST to USX (feature `usx`) |
//! | [`html`] | `usfm_html` | AST to HTML (feature `html`) |
//! | [`json`] | `usfm_json` | AST to JSON (feature `json`) |
//! | [`pipeline`] | `usfm_pipeline` | text replacements, sectioning, diglot, output dispatch (feature `pipeline`) |
//!
//! The four output features are on by default; turning them off leaves a
//! parser-only build. The crates under `crates/` never depend on this one —
//! the facade is for `apps/` and `tasks/`.

use std::sync::Arc;

pub use usfm_ast as ast;
pub use usfm_diagnostics as diagnostics;
pub use usfm_parser as parser;
pub use usfm_span as span;
pub use usfm_style as style;

#[cfg(feature = "html")]
pub use usfm_html as html;
#[cfg(feature = "json")]
pub use usfm_json as json;
#[cfg(feature = "pipeline")]
pub use usfm_pipeline as pipeline;
#[cfg(feature = "usx")]
pub use usfm_usx as usx;

// The names that appear in nearly every signature, at the root so a caller
// does not have to remember which layer each one lives in.
pub use usfm_ast::Document;
pub use usfm_diagnostics::{Code, Diagnostic, ParseResult, Severity};
pub use usfm_span::Span;
pub use usfm_style::StyleSheet;

/// The stylesheet `parse` uses: USFM 3.x as `usfm.sty` defines it, plus this
/// project's additions, built once by `usfm_parser`'s build script.
pub use usfm_parser::DEFAULT_STYLESHEET;

/// Parse USFM with the [default stylesheet](DEFAULT_STYLESHEET).
///
/// Parsing never fails. Malformed input is repaired and every repair is
/// reported as a [`Diagnostic`] on the returned [`ParseResult`]; use
/// [`ParseResult::strict`] to reject a repaired parse.
///
/// ```
/// let result = usfm::parse("\\id GEN\n\\c 1\n\\p\n\\v 1 In the beginning.\n");
/// assert!(result.diagnostics.is_empty());
///
/// let index = result.document.reference_index();
/// assert_eq!(index.verse(1, 1).unwrap().text().trim(), "In the beginning.");
/// ```
pub fn parse(source: &str) -> ParseResult<'_> {
    parse_with(source, &DEFAULT_STYLESHEET)
}

/// Parse USFM with a caller-supplied stylesheet, for a project that ships its
/// own `custom.sty`.
///
/// The returned document owns the sheet its `StyleId`s resolve against —
/// `style_sheet` extended with anything the parser had to derive — so read it
/// back with `Document::style_sheet()` rather than reusing `style_sheet`.
pub fn parse_with<'a>(source: &'a str, style_sheet: &Arc<StyleSheet>) -> ParseResult<'a> {
    usfm_parser::parser::Parser::new(source).parse(style_sheet)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_uses_the_default_stylesheet() {
        let result = parse("\\id GEN\n\\c 1\n\\p\n\\v 1 Text.\n");
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        // The parser only clones the sheet when it has to derive a style, and
        // this input derives none, so the document carries the same `Arc`.
        assert!(Arc::ptr_eq(&DEFAULT_STYLESHEET, result.document.style_sheet()));
    }

    #[test]
    fn parse_with_takes_a_caller_supplied_sheet() {
        let sheet = Arc::clone(&DEFAULT_STYLESHEET);
        let result = parse_with("\\id GEN\n\\c 1\n\\p\n\\v 1 Text.\n", &sheet);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert!(Arc::ptr_eq(&sheet, result.document.style_sheet()));
    }

    #[test]
    fn a_repair_is_reported_not_raised() {
        let result = parse("\\id GEN\n\\c 1\n\\p\n\\v 01 Text.\n");
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.code == Code::NumberHasLeadingZero),
            "{:?}",
            result.diagnostics
        );
        // The document is there either way; the policy is the caller's.
        assert!(!result.document.blocks.is_empty());
    }
}
