//! The USFM toolchain, behind one name.
//!
//! The workspace is split one job per crate
//! (`docs/adr/0001-oxc-style-crate-layout.md`). This crate re-exports those
//! layers under short module names and adds the calls a consumer usually
//! wants — [`parse`] and [`parse_with`] for USFM, [`parse_usx`] and
//! [`parse_usx_with`] for USX (feature `usx`) — so an application depends
//! on `usfm` instead of on five to nine separate crates:
//!
//! | module | crate | what it holds |
//! |---|---|---|
//! | [`span`] | `usfm_span` | [`Span`], `LineIndex` |
//! | [`style`] | `usfm_style` | [`StyleSheet`], `StyleRule`, `StyleId` |
//! | [`ast`] | `usfm_ast` | [`Document`] and its nodes, `Visit`/`VisitMut`/`Fold`, `Cursor` |
//! | [`diagnostics`] | `usfm_diagnostics` | [`Diagnostic`], [`Code`], [`Severity`], [`ParseResult`] |
//! | [`parser`] | `usfm_parser` | the lexer and the parser |
//! | [`semantic`] | `usfm_semantic` | the checks that read a finished tree, and [`ReferenceIndex`] |
//! | [`usx`] | `usfm_usx` | AST to USX and USX back to AST (feature `usx`) |
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
    with_semantic(
        usfm_parser::parser::Parser::new(source)
            .parse_with_options(style_sheet, insert_end_milestones),
    )
}

/// Read USX with the [default stylesheet](DEFAULT_STYLESHEET): a Paratext or
/// DBL export, or anything [`usx::to_usx_string`] wrote.
///
/// The USX counterpart of [`parse`], and it never fails either: a file that
/// is not well-formed XML is one `usx-not-well-formed` Error and an empty
/// document, and everything else the reader repairs or finds is a
/// [`Diagnostic`]. The diagnostics are the reader's and
/// [`usfm_semantic::analyze`]'s, merged the way [`parse_with_options`] merges
/// the parser's (the reader's first, then sorted by `span.start`, stably), so
/// a USX file and its USFM report the same semantic findings. Spans are byte
/// ranges into the XML.
///
/// ```
/// let usx = r#"<usx version="3.0"><book code="GEN" style="id"/>
///   <chapter number="1" style="c" sid="GEN 1"/>
///   <para style="p"><verse number="1" style="v" sid="GEN 1:1"/>In the beginning.<verse eid="GEN 1:1"/></para>
///   <chapter eid="GEN 1"/>
/// </usx>"#;
/// let result = usfm::parse_usx(usx);
/// assert!(result.diagnostics.is_empty());
///
/// let index = usfm::semantic::ReferenceIndex::new(&result.document);
/// assert_eq!(index.verse(1, 1).unwrap().text().trim(), "In the beginning.");
///
/// // A verse the chapter already has is the semantic pass's to report.
/// let usx = usx.replace("<verse eid=\"GEN 1:1\"/>", "<verse eid=\"GEN 1:1\"/><verse number=\"1\" style=\"v\"/>");
/// let codes: Vec<_> = usfm::parse_usx(&usx).diagnostics.iter().map(|d| d.code).collect();
/// assert!(codes.contains(&usfm::Code::DuplicateVerseNumber), "{codes:?}");
/// ```
#[cfg(feature = "usx")]
pub fn parse_usx(source: &str) -> ParseResult<'_> {
    parse_usx_with(source, &DEFAULT_STYLESHEET)
}

/// Read USX with a caller-supplied stylesheet, as [`parse_with`] parses USFM
/// with one. USX names a style and carries no sheet, so this is the sheet the
/// `style` attributes are resolved against; the returned document owns it,
/// extended with any style the reader had to derive.
#[cfg(feature = "usx")]
pub fn parse_usx_with<'a>(source: &'a str, style_sheet: &Arc<StyleSheet>) -> ParseResult<'a> {
    with_semantic(usfm_usx::read_usx_with(source, style_sheet))
}

/// The union: `result`'s diagnostics, then [`usfm_semantic::analyze`]'s over
/// its document, sorted stably by `span.start` (see [`parse_with_options`]).
fn with_semantic(mut result: ParseResult<'_>) -> ParseResult<'_> {
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
        assert!(Arc::ptr_eq(
            &DEFAULT_STYLESHEET,
            result.document.style_sheet()
        ));
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
        assert!(
            parser_only.diagnostics.is_empty(),
            "{:?}",
            parser_only.diagnostics
        );

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
            result
                .diagnostics
                .windows(2)
                .all(|w| w[0].span.start <= w[1].span.start),
            "{:?}",
            result.diagnostics
        );
    }

    /// The same union for USX: `unlisted-book-code` is the semantic pass's,
    /// which `usfm_usx::read_usx` alone does not report.
    #[cfg(feature = "usx")]
    #[test]
    fn parse_usx_returns_the_union_of_both_passes() {
        let source = r#"<usx version="3.0"><book code="ZZZ" style="id"/><chapter number="1" style="c"/><para style="p"><verse number="1" style="v"/>a</para></usx>"#;
        let reader_only = usfm_usx::read_usx(source);
        assert!(
            reader_only.diagnostics.is_empty(),
            "{:?}",
            reader_only.diagnostics
        );

        let codes: Vec<Code> = parse_usx(source)
            .diagnostics
            .iter()
            .map(|d| d.code)
            .collect();
        assert_eq!(codes, vec![Code::UnlistedBookCode]);
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
