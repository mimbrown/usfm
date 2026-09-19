//! HTML output for a parsed USFM [`Document`](usfm_ast::Document).
//!
//! Three pieces, moved here from `usfm_parser` by ticket 14:
//!
//! * [`mod@serialize_html`] — [`ToHtml`], the per-node HTML writer, and
//!   [`SerializeHtml`], the same walk with one overridable hook per node kind;
//! * [`mod@serialize`] — [`Serialize`], the generic serializer trait an output
//!   format other than HTML would be written against. Nothing in this
//!   repository implements it today: the CLI's SILE output has gone through
//!   the USX tree since ticket 06, and the HTML writers use [`ToHtml`] /
//!   [`SerializeHtml`]. It is kept as published API for one milestone, like
//!   the parser's re-exports; ticket 15 or 17 decides whether it stays;
//! * [`context`] — [`Context`], the state a walk carries: the stylesheet to
//!   resolve a `StyleId` against, where in the book it is, the note counters
//!   behind footnote markers and the counters behind generated ids.
//!
//! Nothing here depends on the parser or on `usfm_usx`: a `Document` carries
//! the stylesheet its styles resolve against (hardening plan D3), which is all
//! a serializer needs.

mod escape;
pub use escape::{write_escaped, write_escaped_attribute};

pub mod context;
pub mod serialize;
pub mod serialize_html;

pub use context::Context;
pub use serialize::{Serialize, SerializeSelf, serialize};
pub use serialize_html::{
    HtmlElement, SerializeHtml, ToHtml, XmlAttributes, serialize_html, to_html_string,
};
