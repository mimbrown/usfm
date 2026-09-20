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
//! | [`ast`] | `usfm_ast` | [`Document`] and its nodes, `Visit`/`VisitMut`/`Fold`, `Cursor` |
//! | [`diagnostics`] | `usfm_diagnostics` | [`Diagnostic`], [`Code`], [`Severity`], [`ParseResult`] |
//! | [`parser`] | `usfm_parser` | the lexer and the parser |
//! | [`semantic`] | `usfm_semantic` | the checks that read a finished tree, and [`ReferenceIndex`] |
//! | [`usx`] | `usfm_usx` | AST to USX (feature `usx`) |
//! | [`html`] | `usfm_html` | AST to HTML (feature `html`) |
//! | [`json`] | `usfm_json` | AST to JSON (feature `json`) |
//! | [`codegen`] | `usfm_codegen` | AST back to USFM (feature `codegen`) |
//! | [`pipeline`] | `usfm_pipeline` | text replacements, sectioning, diglot, output dispatch (feature `pipeline`) |
//!
//! The five output features are on by default; turning them off leaves a
//! parser-only build. `semantic` has no feature of its own: [`parse`] is a
//! parse *and* the checks over what it produced, so the semantic pass is part
//! of this crate's idea of parsing rather than an output to switch off. The
//! crates under `crates/` never depend on this one — the facade is for `apps/`
//! and `tasks/`. (`usfm_semantic`'s own tests are the one exception, a
//! dev-dependency cycle cargo allows: they check the union, which only exists
//! here.)

use std::sync::Arc;

pub use usfm_ast as ast;
pub use usfm_diagnostics as diagnostics;
pub use usfm_parser as parser;
pub use usfm_semantic as semantic;
pub use usfm_span as span;
pub use usfm_style as style;

#[cfg(feature = "codegen")]
pub use usfm_codegen as codegen;
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
pub use usfm_semantic::ReferenceIndex;
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
/// The diagnostics are the **union** of the parser's and
/// [`usfm_semantic::analyze`]'s (see [`parse_with_options`]).
///
/// ```
/// let result = usfm::parse("\\id GEN\n\\c 1\n\\p\n\\v 1 In the beginning.\n");
/// assert!(result.diagnostics.is_empty());
///
/// let index = usfm::semantic::ReferenceIndex::new(&result.document);
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
    parse_with_options(source, style_sheet, true)
}

/// Parse with chapter and verse end milestone insertion switched on or off.
///
/// The one knob `usfm_parser::parser::Parser::parse_with_options` exposes,
/// carried through the facade for the conformance harness, which compares a
/// tree without end milestones against reference USX that has none.
///
/// # The union
///
/// This is where the two halves of "parsing" are put together
/// (`.scratch/oxc-layout/spec.md`, M4). The parser reports what it had to
/// repair; [`usfm_semantic::analyze`] reads the finished tree and reports what
/// is worth saying about a document that parsed exactly as written. A caller
/// of this crate wants both, so the returned `diagnostics` are the two lists
/// concatenated and sorted by `span.start`.
///
/// The sort is stable and the parser's diagnostics go in first, so where the
/// two report at the same offset the parser — which is talking about a repair,
/// the more urgent thing — comes first. A caller that wants one half alone
/// calls `usfm_parser::parser::Parser::parse` or `usfm_semantic::analyze`
/// directly; neither of them changed.
pub fn parse_with_options<'a>(
    source: &'a str,
    style_sheet: &Arc<StyleSheet>,
    insert_end_milestones: bool,
) -> ParseResult<'a> {
    let mut result = usfm_parser::parser::Parser::new(source)
        .parse_with_options(style_sheet, insert_end_milestones);
    result
        .diagnostics
        .extend(semantic::analyze(&result.document));
    result
        .diagnostics
        .sort_by_key(|diagnostic| diagnostic.span.start);
    result
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

    /// The union: `unlisted-book-code` is the semantic pass's, and only this
    /// crate's `parse` reports both halves.
    #[test]
    fn parse_returns_the_union_of_both_passes() {
        let source = "\\id ZZZ Some book\n\\c 1\n\\p \\v 1 a\n";
        let parser_only = usfm_parser::parser::Parser::new(source).parse(&DEFAULT_STYLESHEET);
        assert!(parser_only.diagnostics.is_empty(), "{:?}", parser_only.diagnostics);

        let codes: Vec<Code> = parse(source).diagnostics.iter().map(|d| d.code).collect();
        assert_eq!(codes, vec![Code::UnlistedBookCode]);
    }

    /// Both halves at once, in span order however they were produced: the
    /// parser reports `\\v 01` at offset 26 and the semantic pass reports the
    /// book code at offset 0.
    #[test]
    fn the_union_is_sorted_by_span_start() {
        let result = parse("\\id ZZZ Some book\n\\c 1\n\\p \\v 01 a\n");
        let codes: Vec<Code> = result.diagnostics.iter().map(|d| d.code).collect();
        assert_eq!(
            codes,
            vec![Code::UnlistedBookCode, Code::NumberHasLeadingZero],
            "{:?}",
            result.diagnostics
        );
        assert!(
            result.diagnostics.windows(2).all(|w| w[0].span.start <= w[1].span.start),
            "{:?}",
            result.diagnostics
        );
    }

    #[test]
    fn parse_with_options_can_leave_the_end_milestones_out() {
        let source = "\\id GEN\n\\c 1\n\\p \\v 1 a\n";
        let with = parse_with_options(source, &DEFAULT_STYLESHEET, true);
        let without = parse_with_options(source, &DEFAULT_STYLESHEET, false);
        assert_ne!(
            format!("{:?}", with.document.blocks),
            format!("{:?}", without.document.blocks)
        );
    }
}
