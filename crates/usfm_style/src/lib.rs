//! The USFM stylesheet: [`StyleSheet`], its [`StyleRule`]s, and the
//! [`DEFAULT_STYLESHEET`] every reader resolves a marker against when it is
//! given no other.
//!
//! The default sheet lives here rather than in the parser because it is not
//! the parser's: `usfm_usx`'s reader resolves a USX `style` through it too
//! (ticket 45), and an output crate does not depend on the parser. It is
//! generated from `usfm.sty` and `usfm-extra.sty` by `build.rs` into
//! `OUT_DIR`, so nothing generated is checked in, and `usfm_parser` re-exports
//! it under the name it has always had there.

mod sheet;

pub use sheet::*;

/// The generated default stylesheet.
mod generated {
    include!(concat!(env!("OUT_DIR"), "/default_stylesheet.rs"));
}

pub use generated::DEFAULT_STYLESHEET;
