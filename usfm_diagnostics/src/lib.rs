//! What the parser had to say about the input: [`Diagnostic`], its stable
//! [`Code`], its [`Severity`], the [`ParseResult`] that carries them, and the
//! two renderings tools print.
//!
//! Split out of `usfm_parser/src/diagnostics.rs` (ticket 12) so the semantic
//! pass, the output crates and the CLI can report and render diagnostics
//! without depending on the parser. `usfm_parser` re-exports this crate as
//! `usfm_parser::diagnostics`, so every existing path still resolves.

mod diagnostics;

pub use diagnostics::*;
