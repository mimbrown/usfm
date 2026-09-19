//! Attribute values as the parser reads them, tested on the tree.
//!
//! The default (unnamed) value is taken verbatim, quotes and trailing
//! whitespace included, because that is what Paratext does: for
//! `\w x|"y"\w*` it writes `lemma="&quot;y&quot;"` and for `\w x|y \w*`
//! `lemma="y "` (tcdocs `special-cases/empty-attributes`,
//! `paratextTests/WordlistMarkerMissingFromGlossaryCitationForms`). A named
//! value is what is between its quotes.

use usfm_parser::DEFAULT_STYLESHEET;
use usfm_parser::ast::*;
use usfm_parser::parser::Parser;

/// The attribute pairs of the first character style in `\p \v 1 {body}`.
fn attributes(body: &str) -> Vec<(String, String)> {
    let source = format!("\\id GEN\n\\c 1\n\\p \\v 1 {body}");
    let document = Parser::new(&source)
        .parse_with_options(&DEFAULT_STYLESHEET, false)
        .document;
    let Some(Block::Para(para)) = document.blocks.get(2) else {
        panic!("no paragraph in {source:?}");
    };
    let char = para
        .children
        .iter()
        .find_map(|inline| match inline {
            Inline::Char(char) => Some(char),
            _ => None,
        })
        .expect("a character style");
    char.attributes
        .as_ref()
        .map(|attributes| {
            attributes
                .pairs
                .iter()
                .map(|pair| (pair.name.to_string(), pair.value.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn named_value_is_what_is_between_the_quotes() {
    assert_eq!(
        attributes("\\w x|lemma=\"y\"\\w*"),
        [("lemma".to_string(), "y".to_string())]
    );
}

#[test]
fn default_value_is_verbatim_quotes_included() {
    assert_eq!(
        attributes("\\w x|\"y\"\\w*"),
        [(String::new(), "\"y\"".to_string())]
    );
}

#[test]
fn default_value_keeps_trailing_whitespace() {
    assert_eq!(attributes("\\w x|y \\w*"), [(String::new(), "y ".to_string())]);
}

#[test]
fn default_value_stops_before_a_named_attribute() {
    assert_eq!(
        attributes("\\w x|y strong=\"H1\"\\w*"),
        [
            (String::new(), "y ".to_string()),
            ("strong".to_string(), "H1".to_string())
        ]
    );
}

#[test]
fn double_slash_in_a_default_value_is_not_a_line_break() {
    assert_eq!(
        attributes("\\jmp text|https://example.org/a\\jmp*"),
        [(String::new(), "https://example.org/a".to_string())]
    );
}
