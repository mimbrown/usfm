//! Everything between a parsed [`Document`](usfm_ast::Document) and the bytes
//! a tool writes out: text replacements, sectioning, diglot weaving, the
//! prompt format, and the dispatch from an output format to the crate that
//! writes it.
//!
//! The pipeline layer of `docs/adr/0001-oxc-style-crate-layout.md`. It sits
//! above the output crates — it may depend on `usfm_usx` and `usfm_html`,
//! which may not depend on each other — and below `apps/usfm_cli`, which owns
//! the argument parsing, the file reading, watch mode and the diagnostics
//! printing. Everything here is a function or a small adapter over documents,
//! so it can be tested without a process.
//!
//! ```
//! # use usfm_pipeline::{OutputFormat, render};
//! # fn show(document: &usfm_ast::Document<'_>, style_sheet: &usfm_style::StyleSheet) {
//! let usx = render(document, style_sheet, OutputFormat::Usx).unwrap();
//! # let _ = usx;
//! # }
//! ```
//!
//! Every stylesheet a function here takes is the document's own
//! (`document.style_sheet()`): the parser extends the sheet it is given with
//! any style it had to derive, and only that sheet resolves every `StyleId` in
//! the tree (hardening plan D3).

pub mod diglot;
mod render;
pub mod sections;
pub mod sile;
pub mod text_replacements;

pub use diglot::{
    DocumentSectionHtmlSerializer, DocumentWeaver, Side, serialize_html_diglot, weave_prompt,
};
pub use render::{OutputFormat, RenderError, render, render_diglot};
pub use sections::{IterSections, document_sections, sections};
pub use sile::to_sile_string;
pub use text_replacements::TextReplacement;

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use usfm_ast::Document;
    use usfm_parser::{DEFAULT_STYLESHEET, parser::Parser};
    use usfm_style::StyleSheet;

    /// Parse a short input for a test, and hand back the document together
    /// with the stylesheet its styles resolve against — the pair every
    /// function in this crate takes.
    ///
    /// Diagnostics are dropped: these tests are about what comes out of the
    /// pipeline, and the parser recovers from everything.
    pub(crate) fn parse(source: &str) -> (Document<'_>, Arc<StyleSheet>) {
        let document = Parser::new(source).parse(&DEFAULT_STYLESHEET).document;
        let style_sheet = Arc::clone(document.style_sheet());
        (document, style_sheet)
    }
}
