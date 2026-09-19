//! HTML output for a parsed USFM [`Document`](usfm_ast::Document).
//!
//! Two pieces, moved here from `usfm_parser` by ticket 14:
//!
//! * [`mod@serialize_html`] — [`ToHtml`], the per-node HTML writer, and
//!   [`SerializeHtml`], the same walk with one overridable hook per node kind;
//! * [`context`] — [`Context`], the state a walk carries: the stylesheet to
//!   resolve a `StyleId` against, where in the book it is, the note counters
//!   behind footnote markers and the counters behind generated ids.
//!
//! A third, `serialize.rs`, is gone: the generic `Serialize` trait an output
//! format other than HTML would have been written against had no implementor
//! anywhere — the SILE output has gone through the USX tree since ticket 06 —
//! and ticket 15 deleted it rather than carry a trait no caller could have
//! been using.
//!
//! Nothing here depends on the parser or on `usfm_usx`: a `Document` carries
//! the stylesheet its styles resolve against (hardening plan D3), which is all
//! a serializer needs.

mod escape;
pub use escape::{write_escaped, write_escaped_attribute};

pub mod context;
pub mod serialize_html;

pub use context::Context;
pub use serialize_html::{
    HtmlElement, SerializeHtml, ToHtml, XmlAttributes, serialize_html, to_html_string,
};
