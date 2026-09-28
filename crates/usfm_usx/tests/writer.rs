//! `UsxWriter` driven one block at a time, with a caller's own elements and
//! hooks (ticket 59). The documents come from USX through `read_usx`, since
//! this crate has no USFM parser.

use usfm_ast::{Block, Char, Para};
use usfm_usx::{UsxHooks, UsxOptions, UsxWriter, XmlNode, read_usx, to_usx_node};

/// Verse 1 runs from a `\p` into a `\m`, so the `\m` carries `vid`.
const SOURCE: &str = r#"<usx version="3.1"><book code="GEN" style="id"/>
<chapter number="1" style="c" sid="GEN 1"/>
<para style="p"><verse number="1" style="v" sid="GEN 1:1"/>In the beginning</para>
<para style="m">God <char style="nd">Lord <char style="w" lemma="x">made</char></char><verse eid="GEN 1:1"/></para>
<para style="p"><verse number="2" style="v" sid="GEN 1:2"/>and<verse eid="GEN 1:2"/></para>
<chapter eid="GEN 1"/></usx>"#;

fn only(nodes: Vec<XmlNode>) -> XmlNode {
    assert_eq!(nodes.len(), 1, "{nodes:?}");
    nodes.into_iter().next().unwrap()
}

#[test]
fn block_by_block_under_a_root_of_ones_own_is_the_whole_document() {
    let document = read_usx(SOURCE).document;
    let mut usx = UsxWriter::new(document.style_sheet(), UsxOptions::default());
    usx.open("usx", &[("version", "3.1")]);
    for block in &document.blocks {
        usx.block(block);
    }
    usx.close();
    assert_eq!(
        only(usx.finish()).to_string(),
        to_usx_node(&document).to_string()
    );
}

#[test]
fn an_element_between_blocks_leaves_the_verse_open() {
    let document = read_usx(SOURCE).document;
    let mut usx = UsxWriter::new(document.style_sheet(), UsxOptions::default());
    usx.open("usx", &[("version", "3.1")]);
    for block in &document.blocks {
        let m = matches!(block, Block::Para(para) if usx.marker(para.style) == "m");
        if m {
            usx.open("cartouche", &[("kind", "a & b")]);
            assert_eq!(usx.verse().map(ToString::to_string).as_deref(), Some("1"));
            usx.block(block);
            usx.close();
        } else {
            usx.block(block);
        }
    }
    let written = only(usx.finish()).to_string();
    // The wrapped paragraph still continues verse 1 and still ends it.
    assert!(
        written.contains(r#"<cartouche kind="a &amp; b">"#),
        "{written}"
    );
    let inside =
        &written[written.find("<cartouche").unwrap()..written.find("</cartouche>").unwrap()];
    assert!(
        inside.contains(r#"<para style="m" vid="GEN 1:1">"#),
        "{inside}"
    );
    assert!(inside.contains(r#"<verse eid="GEN 1:1" />"#), "{inside}");
    assert!(written.contains(r#"sid="GEN 1:2""#), "{written}");
}

/// `\nd` as `<name>`, and a count of the `\w` words written, nested ones
/// included: a hook's children go through the hooks too.
#[derive(Default)]
struct Names {
    words: usize,
}

impl UsxHooks for Names {
    fn char(writer: &mut UsxWriter<'_, Self>, char: &Char<'_>) {
        match writer.marker(char.style) {
            "nd" => {
                writer.open("name", &[]);
                for child in &char.children {
                    writer.inline(child);
                }
                writer.close();
            }
            "w" => {
                writer.hooks_mut().words += 1;
                writer.write_char(char);
            }
            _ => writer.write_char(char),
        }
    }
}

#[test]
fn a_hook_takes_over_one_kind_of_node() {
    let document = read_usx(SOURCE).document;
    let mut usx = UsxWriter::with_hooks(
        document.style_sheet(),
        UsxOptions::default(),
        Names::default(),
    );
    usx.document(&document);
    assert_eq!(usx.hooks().words, 1);
    let written = only(usx.finish()).to_string();
    assert!(
        written.contains(r#"<name>Lord <char style="w" lemma="x">made</char></name>"#),
        "{written}"
    );
    assert!(!written.contains(r#"style="nd""#), "{written}");
}

#[test]
fn finishing_closes_what_is_still_open() {
    let document = read_usx(SOURCE).document;
    let mut usx = UsxWriter::new(document.style_sheet(), UsxOptions::default());
    usx.open("outer", &[]);
    usx.open("inner", &[]);
    usx.text("x");
    usx.empty("br", &[]);
    let written = only(usx.finish()).to_string();
    assert!(written.contains("<inner>x<br /></inner>"), "{written}");
}

/// A paragraph hook that opens an element and returns without closing it.
struct Unbalanced;

impl UsxHooks for Unbalanced {
    fn para(writer: &mut UsxWriter<'_, Self>, para: &Para<'_>) {
        writer.write_para(para);
        writer.open("left-open", &[]);
    }
}

#[test]
#[should_panic(expected = "left an element it opened unclosed")]
fn a_hook_that_leaves_an_element_open_inside_a_node_is_a_bug() {
    let document = read_usx(
        r#"<usx version="3.1"><book code="GEN" style="id"/><table><row style="tr"><cell style="tc1" align="start"><char style="bd">x</char></cell></row></table><sidebar style="esb"><para style="p">x</para></sidebar></usx>"#,
    )
    .document;
    let mut usx = UsxWriter::with_hooks(document.style_sheet(), UsxOptions::default(), Unbalanced);
    usx.document(&document);
}
