//! The shape, end to end: two documents snapshotted in full, the output's
//! well-formedness, and a count over a real book.
//!
//! The inputs are parsed with `usfm_parser` (a dev-dependency: the crate
//! itself reads only the AST), so what is snapshotted is what a caller gets
//! for source it actually has.

use serde_json::Value;
use usfm_json::{to_json_string, to_json_string_pretty, to_json_value};
use usfm_parser::DEFAULT_STYLESHEET;
use usfm_parser::parser::Parser;

/// A paragraph with a verse, a `\w` with attributes, a note with `\fr`/`\ft`,
/// an optional break, `\cp`/`\vp` and a `\usfm` version line.
const SCRIPTURE: &str = r#"\id GEN Genesis
\usfm 3.1
\c 1
\cp A
\p
\v 1 \vp 1a\vp* In the beginning \nd God\nd* \w created|lemma="create" strong="H1254"\w* the heavens//and the earth.\f + \fr 1.1 \ft Or: a note.\f*
"#;

/// The block-level structures: a periph with an attribute list, a sidebar with
/// a category, quotation milestones and a table.
const STRUCTURE: &str = r#"\id FRT
\periph Title Page|id="title"
\p \qt-s |Narrator\*Once told.\qt-e\*
\esb \cat People\cat*
\p An aside.
\esbe
\tr \th1 Tribe \thr2 Number
\tr \tc1 Reuben \tcr2 46,500
"#;

/// Parse and write, the way a caller would.
fn json(source: &str) -> String {
    let document = Parser::new(source).parse(&DEFAULT_STYLESHEET).document;
    to_json_string_pretty(&document)
}

#[test]
fn scripture_snapshot() {
    insta::assert_snapshot!(json(SCRIPTURE));
}

#[test]
fn structure_snapshot() {
    insta::assert_snapshot!(json(STRUCTURE));
}

/// Whatever the tree holds, the bytes are JSON: `to_json_string` is one line
/// that parses back to the value it was written from.
#[test]
fn the_output_is_well_formed_json() {
    for source in [SCRIPTURE, STRUCTURE] {
        let document = Parser::new(source).parse(&DEFAULT_STYLESHEET).document;
        let text = to_json_string(&document);
        assert!(!text.contains('\n'), "the compact form is one line");
        let parsed: Value = serde_json::from_str(&text).expect("the output parses back");
        assert_eq!(parsed, to_json_value(&document));
        assert_eq!(parsed["type"], "document");
    }
}

/// Every object in `value`, itself included.
fn objects<'a>(value: &'a Value, found: &mut Vec<&'a serde_json::Map<String, Value>>) {
    match value {
        Value::Object(map) => {
            found.push(map);
            for child in map.values() {
                objects(child, found);
            }
        }
        Value::Array(items) => {
            for item in items {
                objects(item, found);
            }
        }
        _ => {}
    }
}

/// The notes of a real book, counted by walking the value rather than by
/// searching the text: the same 268 footnotes `usfm_html`'s `tests/footnotes.
/// rs` numbers 1..268 in this file.
#[test]
fn a_whole_book_keeps_every_note() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tasks/benchmark/corpus/web/71-WIS.usfm");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    let document = Parser::new(&source).parse(&DEFAULT_STYLESHEET).document;

    let value = to_json_value(&document);
    let mut found = Vec::new();
    objects(&value, &mut found);
    let notes = found.iter().filter(|map| map["type"] == "note").count();
    assert_eq!(notes, 268);

    // And every object the walk found is a node: `"type"` and `"span"` are on
    // all of them, the attribute pairs aside.
    for map in &found {
        if map.contains_key("name") && map.contains_key("value") {
            continue;
        }
        assert!(map.contains_key("type"), "{map:?}");
        assert!(map.contains_key("span"), "{map:?}");
    }
}
