//! USFM output: a [`Document`](usfm_ast::Document) written back as the markup
//! it was parsed from.
//!
//! ```
//! # use usfm_ast::Document;
//! # fn show(document: &Document<'_>) {
//! let text = usfm_codegen::to_usfm_string(document);
//! # let _ = text;
//! # }
//! ```
//!
//! The property this crate exists for (M5 in `.scratch/oxc-layout/spec.md`) is
//! that `parse(to_usfm_string(&parse(source).document))` has the same tree as
//! `parse(source)`, ignoring spans. It is *not* that the output is the source
//! byte for byte: the writer emits one canonical spelling of each construct,
//! so an implicitly closed character style comes back with its closing marker,
//! a nested one with its `\+`, and an unquoted attribute value in quotes. See
//! [`usfm`] for the whole table of what is written and why.

pub mod usfm;

pub use usfm::{to_usfm_string, write_usfm};
