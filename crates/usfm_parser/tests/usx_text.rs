//! USX as text: what `to_usx_string` writes, which is what the CLI prints.
//!
//! The tcdocs harness compares trees, so it cannot see the writer. These pin
//! the three things the writer owns: word-level attributes reach the output,
//! reserved characters are escaped, and no whitespace is added where
//! whitespace is content.

use usfm_parser::DEFAULT_STYLESHEET;
use usfm_parser::parser::Parser;
use usfm_usx::to_usx_string;
use usfm_usx::xml_document::XmlDocument;

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

/// A `*` caller (`\f * \ft ...\f*`, which occurs in published texts) reaches
/// the output as `caller="*"`, and the `*` is not left in the note's text.
#[test]
fn star_note_caller_is_written() {
    let output = usx("\\id GEN\n\\c 1\n\\p \\v 1 Word\\f * \\ft note\\f*");
    assert!(
        output.contains(r#"<note caller="*" style="f"><char style="ft">note</char></note>"#),
        "{output}"
    );
    XmlDocument::from(output.as_bytes()).expect("well-formed XML");
}

/// USX is XML, so `to_usx_string` has to write something an XML reader
/// accepts even when the source does not. XML 1.0 has no way to write a C0
/// control character — not even as `&#0;` — so the writer replaces it with
/// U+FFFD, in text and in attribute values alike. The fuzzer found this on a
/// source of one byte, a NUL.
#[test]
fn characters_xml_forbids_are_replaced() {
    let output = usx("\\id GEN\n\\c 1\n\\p \\v 1 a\u{0}b \\w c|lemma=\"d\u{c}e\"\\w*");
    assert!(output.contains("a\u{fffd}b "), "{output}");
    assert!(output.contains("lemma=\"d\u{fffd}e\""), "{output}");
    assert!(!output.contains('\u{0}') && !output.contains('\u{c}'), "{output}");
    XmlDocument::from(output.as_bytes()).expect("well-formed XML");
}

/// XML has no repeated attribute, so the writer keeps the first value of a
/// name and drops later ones (`duplicate-attribute`).
#[test]
fn repeated_attributes_are_written_once() {
    let output = usx("\\id GEN\n\\c 1\n\\p \\v 1 \\w a|lemma=\"first\" lemma=\"second\"\\w*");
    assert!(output.contains(r#"<char style="w" lemma="first">a</char>"#), "{output}");
    XmlDocument::from(output.as_bytes()).expect("well-formed XML");
}

/// An attribute whose name is not an XML name cannot be written at all, so
/// the parser reports `malformed-attribute-name` (tested in `recovery.rs`)
/// and the serializer drops the attribute rather than emit broken XML.
#[test]
fn attributes_that_are_not_xml_names_are_dropped() {
    let output = usx("\\id GEN\n\\c 1\n\\p \\v 1 \\w a|b<c=\"1\" strong=\"H1\"\\w*");
    assert!(output.contains(r#"<char style="w" strong="H1">a</char>"#), "{output}");
    XmlDocument::from(output.as_bytes()).expect("well-formed XML");
}

/// `cell@style` keeps the marker's alignment letter, not just the `align`
/// attribute: USX gives it the pattern `t[hc][rc]?\d+(-\d+)?` and tcdocs
/// writes `<cell style="tcr3" align="end">` for `\tcr3`. The centred forms
/// `\tcc`/`\thc` (USFM 3.1) follow the same rule.
#[test]
fn table_cell_style_keeps_its_alignment_letter() {
    let output =
        usx("\\id GEN\n\\c 1\n\\tr \\thc1 a \\th2 b \\thr3 c\n\\tr \\tcc1 d \\tc2 e \\tcr3 f");
    for cell in [
        r#"<cell style="thc1" align="center">"#,
        r#"<cell style="th2" align="start">"#,
        r#"<cell style="thr3" align="end">"#,
        r#"<cell style="tcc1" align="center">"#,
        r#"<cell style="tc2" align="start">"#,
        r#"<cell style="tcr3" align="end">"#,
    ] {
        assert!(output.contains(cell), "missing {cell} in {output}");
    }
}

/// The default (unnamed) attribute is named after its marker by the
/// `usfm:propval` annotations in tcdocs' `grammar/usx.rnc`: `lang` for the
/// transliteration and foreign-word styles USFM 3.1.2 defined it for, and
/// `ref` for the `\vid` milestone.
#[test]
fn default_attribute_names_follow_the_schema() {
    let output = usx("\\id MAT\n\\c 1\n\\p \\v 1 \\tl Eli\\tl* and \\tl Eli|Aramic\\tl*");
    assert!(
        output.contains(r#"<char style="tl">Eli</char>"#),
        "{output}"
    );
    assert!(
        output.contains(r#"<char style="tl" lang="Aramic">Eli</char>"#),
        "{output}"
    );

    let output = usx("\\id MAT\n\\c 1\n\\p \\v 1 \\wl ro|ro\\wl*");
    assert!(
        output.contains(r#"<char style="wl" lang="ro">ro</char>"#),
        "{output}"
    );

    let output = usx("\\id MRK\n\\c 4\n\\vid|MRK 4:26\\*\n\\p \\v 26 a");
    assert!(
        output.contains(r#"<ms style="vid" ref="MRK 4:26" />"#),
        "{output}"
    );
}

/// A milestone written with a pipe and no attributes (`\ts-s |\*`, how
/// unfoldingWord's aligned texts write a translation section) is the same
/// milestone as `\ts-s\*`: the empty list adds nothing to write, so the two
/// sources give byte-identical USX. The parser reports
/// `empty-milestone-attribute-list` on the first, at warning severity.
#[test]
fn an_empty_attribute_list_on_a_milestone_writes_the_same_usx() {
    let with_pipe = usx("\\id GEN\n\\c 1\n\\p \\v 1 a \\ts-s |\\* b");
    let without_pipe = usx("\\id GEN\n\\c 1\n\\p \\v 1 a \\ts-s\\* b");
    assert!(
        with_pipe.contains(r#"<ms style="ts-s" />"#),
        "{with_pipe}"
    );
    assert_eq!(with_pipe, without_pipe);
    XmlDocument::from(with_pipe.as_bytes()).expect("well-formed XML");
}
