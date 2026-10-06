//! `textDocument/codeAction`: the quick fixes for the repairs the toolchain
//! reports (ticket 32).
//!
//! A pass-through. The fixes are `usfm_fix`'s, the same ones `usfm fix`
//! applies from the command line; the server holds no fix of its own, as it
//! holds no diagnostic of its own. What is left to do here is in `main.rs`:
//! match the editor's diagnostic back to the parse's, and turn a fix's
//! spans into protocol ranges.

pub use usfm::fix::fix;
