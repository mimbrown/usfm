//! USX output: a [`Document`](usfm_ast::Document) as an XML tree, and that
//! tree as text.
//!
//! ```
//! # use usfm_ast::Document;
//! # fn show(document: &Document<'_>) {
//! let text = usfm_usx::to_usx_string(document);
//! # let _ = text;
//! # }
//! ```
//!
//! The crate owns both halves of the job (ticket 13):
//!
//! * [`usx`] walks the AST and builds the tree, holding its own private
//!   state — the book code, the open chapter and verse, and the stylesheet the
//!   document carries. It does not depend on `usfm_parser`, and there is no
//!   shared `Context`.
//! * [`xml_document`] is the [`XmlNode`] tree itself, its writer (escaping,
//!   the characters XML forbids, indentation) and a reader that parses USX
//!   back into the same tree, which the conformance harness compares against
//!   the reference files.

pub mod usx;
pub mod xml_document;

pub use usx::{
    DEFAULT_USX_VERSION, UsxOptions, to_usx_node, to_usx_node_with_options, to_usx_string,
};
pub use xml_document::{XmlDocument, XmlElement, XmlNode};
