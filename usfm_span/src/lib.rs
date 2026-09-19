//! Source positions: [`Span`], the byte range a node was read from, and
//! [`LineIndex`], the line/column lookup built once per source.
//!
//! This is the leaf of the crate graph in
//! `docs/adr/0001-oxc-style-crate-layout.md` and depends on nothing.
//! `usfm_ast` re-exports it as `usfm_ast::span`, so `usfm_ast::Span`,
//! `usfm_ast::span::Span` and `usfm_parser::lexer::span::Span` all name the
//! type defined here.

mod line_index;
mod span;

pub use line_index::LineIndex;
pub use span::*;
