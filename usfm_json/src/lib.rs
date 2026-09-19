//! JSON output: a [`Document`](usfm_ast::Document) as a `serde_json::Value`,
//! and that value as text.
//!
//! ```
//! # use usfm_ast::Document;
//! # fn show(document: &Document<'_>) {
//! let text = usfm_json::to_json_string(document);
//! # let _ = text;
//! # }
//! ```
//!
//! The shape is the AST as it is (ticket 16): one object per node, no
//! USX-flavoured renaming, every field under the name the AST field has. It is
//! what the language server and any non-Rust consumer reads, so it is
//! described in full on [`json`], and the vocabulary of `"type"` strings is
//! [`TYPES`].
//!
//! `usfm_ast` stays free of serde: nothing here is a `Serialize` derive, the
//! value is built node by node, and the AST gains no dependency.

pub mod json;

pub use json::{TYPES, to_json_string, to_json_string_pretty, to_json_value};
