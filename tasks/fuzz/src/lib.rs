//! What every fuzz target checks, in one place.
//!
//! The parser never fails: it recovers and reports `Diagnostic`s. So the
//! properties a fuzz target can assert are the ones that hold for *any* input,
//! valid or not:
//!
//! 1. parsing does not panic;
//! 2. every span points at the source it was read from
//!    (`usfm_parser::span_check`, the same invariants `usfm_parser/tests/spans.rs`
//!    asserts over hand-written inputs);
//! 3. serializing the recovered tree to USX does not panic and produces
//!    well-formed XML.
//!
//! Nothing here catches a panic: a panic *is* the finding, and libFuzzer wants
//! to see it.

use usfm_parser::parser::Parser;
use usfm_parser::usx::to_usx_string;
use usfm_parser::{DEFAULT_STYLESHEET, span_check};

/// Run every check over one input.
pub fn check_source(source: &str) {
    let result = Parser::new(source).parse(&DEFAULT_STYLESHEET);
    span_check::check_parse(source, &result);
    let usx = to_usx_string(&result.document);
    assert_well_formed(&usx, source);
}

/// Parse `usx` back with `xml-rs` and drain it to the end of the document.
///
/// A serializer that forgets to escape something, or writes a character XML
/// does not allow, shows up here as a reader error rather than as invalid
/// output nobody looked at.
fn assert_well_formed(usx: &str, source: &str) {
    let parser = xml::EventReader::from_str(usx);
    for event in parser {
        match event {
            Ok(xml::reader::XmlEvent::EndDocument) => return,
            Ok(_) => {}
            Err(error) => {
                panic!("USX is not well-formed XML: {error}\nUSX:\n{usx}\nsource: {source:?}")
            }
        }
    }
}
