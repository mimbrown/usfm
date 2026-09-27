//! USX: a [`Document`](usfm_ast::Document) as an XML tree and that tree as
//! text, and USX read back into a `Document`.
//!
//! ```
//! # use usfm_ast::Document;
//! # fn show(document: &Document<'_>) {
//! let text = usfm_usx::to_usx_string(document);
//! let again = usfm_usx::read_usx(&text);
//! # let _ = again;
//! # }
//! ```
//!
//! The crate owns both directions:
//!
//! * [`usx`] walks the AST and builds the tree, holding its own private
//!   state — the book code, the open chapter and verse, and the stylesheet the
//!   document carries. It does not depend on `usfm_parser`, and there is no
//!   shared `Context` (ticket 13).
//! * [`xml_document`] is the [`XmlNode`] tree itself, its writer (escaping,
//!   the characters XML forbids, indentation) and a reader that parses USX
//!   back into the same tree, which the conformance harness compares against
//!   the reference files.
//! * [`read`] is the reader (ticket 45, M7): [`read_usx`] and
//!   [`read_usx_with`] turn USX into a `Document` and the diagnostics a
//!   [`ParseResult`](usfm_diagnostics::ParseResult) carries, mapping every
//!   element onto the node [`usx`] writes it from. The XML itself is parsed by
//!   `roxmltree`, which gives every node the byte range its span needs.

pub mod read;
pub mod usx;
pub mod xml_document;

pub use read::{read_usx, read_usx_with};
pub use usx::{
    DEFAULT_USX_VERSION, UsxOptions, to_usx_node, to_usx_node_with_options, to_usx_string,
};
pub use xml_document::{XmlDocument, XmlElement, XmlNode};
