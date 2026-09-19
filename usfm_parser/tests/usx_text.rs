//! USX as text: what `to_usx_string` writes, which is what the CLI prints.
//!
//! The tcdocs harness compares trees, so it cannot see the writer. These pin
//! the three things the writer owns: word-level attributes reach the output,
//! reserved characters are escaped, and no whitespace is added where
//! whitespace is content.

use usfm_parser::DEFAULT_STYLESHEET;
use usfm_parser::parser::Parser;
use usfm_parser::usx::to_usx_string;
use usfm_parser::xml_document::XmlDocument;

fn usx(source: &str) -> String {
    to_usx_string(&Parser::new(source).parse(&DEFAULT_STYLESHEET).document)
}

#[test]
fn word_level_attributes_are_written() {
    let output = usx("\\id GEN\n\\c 1\n\\p \\v 1 \\w beginning|lemma=\"start\" strong=\"H7225\"\\w*");
    assert!(
        output.contains(r#"<char style="w" lemma="start" strong="H7225">beginning</char>"#),
        "{output}"
    );
}

#[test]
fn reserved_characters_are_escaped() {
    let output = usx("\\id GEN\n\\c 1\n\\p \\v 1 Tom & <Jerry> \\w a|lemma=\"x&y\"\\w*");
    assert!(output.contains("Tom &amp; &lt;Jerry&gt; "), "{output}");
    assert!(output.contains(r#"lemma="x&amp;y""#), "{output}");
    XmlDocument::from(output.as_bytes()).expect("well-formed XML");
}

#[test]
fn text_content_gets_no_added_whitespace() {
    let output = usx("\\id GEN\n\\c 1\n\\p \\v 1 Word\\f + \\ft note\\f*");
    assert!(
        output.contains(
            r#"  <para style="p"><verse number="1" style="v" sid="GEN 1:1" />Word<note caller="+" style="f"><char style="ft">note</char></note><verse eid="GEN 1:1" /></para>"#
        ),
        "{output}"
    );
}
