//! Every `"type"` in [`usfm_json::TYPES`] is reachable.
//!
//! `usfm_ast::test_fixtures::sample_document` would have been the input, but
//! it is `#[cfg(test)]` inside `usfm_ast` and so cannot be linked from here,
//! and it has no periph and no optional break either. The document below is
//! written as USFM instead, which has the advantage that it is a document a
//! user could write: nothing is reachable in the output that the parser cannot
//! produce.

use serde_json::Value;
use usfm_json::{TYPES, to_json_value};
use usfm_parser::DEFAULT_STYLESHEET;
use usfm_parser::parser::Parser;

/// One of every `Block`, `Inline` and `Note` the AST has:
///
/// * `Block`: `\id` (book), `\c` (chapter start, and the chapter end the
///   parser synthesizes at the end), `\p` (para), `\tr` (table, row, cell),
///   `\ts\*` between blocks (milestone), `\esb` (sidebar), `\periph`.
/// * `Inline`: text, `\v` (verse start, and the verse end the parser
///   synthesizes), `\nd` and `\w` (char), `\f` (note), `\qt-s` (milestone),
///   `//` (opt break).
///
/// The `\periph` is last because a peripheral division runs to the next
/// `\periph` or `\id`: anything written after it is inside it, and peripheral
/// matter has no verses, so a verse or chapter left open there is never ended.
const EVERYTHING: &str = r#"\id FRT Front matter
\c 1
\ts\*
\p
\v 1 In the beginning//God \w created|lemma="create"\w* \nd the heavens\nd*\f + \ft A note.\f* \qt-s |Narrator\*and the earth.\qt-e\*
\tr \th1 Tribe \tc2 Reuben
\esb \cat People\cat*
\p An aside.
\esbe
\periph Title Page|id="title"
\p A title page.
"#;

/// Every `"type"` string in the value, in no particular order.
fn types(value: &Value, found: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(kind)) = map.get("type") {
                found.push(kind.clone());
            }
            for child in map.values() {
                types(child, found);
            }
        }
        Value::Array(items) => {
            for item in items {
                types(item, found);
            }
        }
        _ => {}
    }
}

/// The vocabulary is complete in both directions: every string in `TYPES`
/// appears, and nothing appears that is not in `TYPES`.
#[test]
fn every_type_is_reachable() {
    let document = Parser::new(EVERYTHING).parse(&DEFAULT_STYLESHEET).document;
    let mut found = Vec::new();
    types(&to_json_value(&document), &mut found);

    for kind in TYPES {
        assert!(
            found.iter().any(|seen| seen == kind),
            "no {kind:?} in the output of the coverage document; found {found:?}"
        );
    }
    for kind in &found {
        assert!(
            TYPES.contains(&kind.as_str()),
            "{kind:?} is written but not listed in TYPES"
        );
    }
}
