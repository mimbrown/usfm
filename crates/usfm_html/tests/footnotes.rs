//! Footnote numbering, over a whole book.
//!
//! `Context::get_and_increment_note_number` keeps two sequences — one for
//! `\f + …`, the callers a renderer numbers, and one shared by every other
//! caller — and the HTML writer puts the first sequence's number in the
//! button a reader clicks and in the popover's id. A book's worth of notes is
//! the only way to see that the two sequences stay in document order and that
//! neither leaks into the other.

use std::path::PathBuf;
use std::sync::Arc;

use usfm_html::to_html_string;
use usfm_parser::{DEFAULT_STYLESHEET, parser::Parser};

/// A file from the benchmark corpus, by path relative to this crate.
fn corpus_file(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tasks/benchmark/corpus")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Render with the document's own stylesheet, which is the one its styles
/// resolve against (hardening plan D3).
fn html(source: &str) -> String {
    let document = Parser::new(source).parse(&DEFAULT_STYLESHEET).document;
    let style_sheet = Arc::clone(document.style_sheet());
    to_html_string(&document, &style_sheet)
}

/// Every note button in document order, as (id, the text between the tags).
/// A note is `<button class="note f-trigger" popovertarget="note-3">3</button>`
/// followed by the popover that id points at.
fn note_buttons(html: &str) -> Vec<(String, String)> {
    let mut buttons = Vec::new();
    let mut rest = html;
    while let Some(start) = rest.find("<button class=\"note ") {
        rest = &rest[start..];
        let open_end = rest.find('>').expect("an open tag ends");
        let open = &rest[..open_end];
        let id = open
            .split_once("popovertarget=\"")
            .map(|(_, tail)| tail.split_once('"').expect("a quoted value").0)
            .expect("a note button carries popovertarget")
            .to_string();
        let body = &rest[open_end + 1..];
        let close = body.find("</button>").expect("a button is closed");
        buttons.push((id, body[..close].to_string()));
        rest = &body[close..];
    }
    buttons
}

/// The 268 `\f + …` notes of Wisdom are numbered 1, 2, 3 … in document order,
/// in the marker a reader sees and in the id that marker points at.
#[test]
fn footnotes_are_numbered_in_document_order() {
    let html = html(&corpus_file("web/71-WIS.usfm"));
    let buttons = note_buttons(&html);
    assert_eq!(buttons.len(), 268, "one button per note");

    let expected: Vec<(String, String)> = (1..=268)
        .map(|n| (format!("note-{n}"), n.to_string()))
        .collect();
    assert_eq!(buttons, expected);

    // The popover each button points at is in the output too, once.
    for (id, _) in &buttons {
        let opening = format!("<span id=\"{id}\" class=\"f\" popover>");
        assert_eq!(html.matches(&opening).count(), 1, "one popover for {id}");
    }
}

/// What a caller other than `+` does to the numbering, as the code stands:
/// `\f - …` and `\f * …` share a **second** counter, so they take no number
/// from the `+` sequence — the `+` notes either side of them are still 1 and
/// 2 — and their own ids are `note-c1`, `note-c2` … A `-` caller shows no
/// marker at all; a custom one shows the caller as written.
#[test]
fn a_custom_caller_does_not_consume_a_plus_number() {
    let html = html(concat!(
        "\\id GEN\n\\c 1\n\\p \\v 1 one\\f + \\ft plus one\\f*",
        " two\\f - \\ft minus\\f*",
        " three\\f ※ \\ft custom\\f*",
        " four\\f + \\ft plus two\\f*\n",
    ));
    assert_eq!(
        note_buttons(&html),
        vec![
            ("note-1".to_string(), "1".to_string()),
            // `-` asks for no marker, and takes its id from the second run.
            ("note-c1".to_string(), String::new()),
            ("note-c2".to_string(), "※".to_string()),
            // The `+` run is untouched by the two above.
            ("note-2".to_string(), "2".to_string()),
        ]
    );
}

/// Written one block at a time with one `Context`, and two of the blocks
/// wrapped in markup of the caller's own, a book comes out as the whole
/// document does, wrapper aside (ticket 61): the note numbers carry across
/// the wrapper because they live in the `Context`, not in the walk.
#[test]
fn blocks_written_one_at_a_time_share_the_note_numbers() {
    use usfm_ast::Block;
    use usfm_html::{Context, ToHtml};

    let source = "\\id GEN\n\\c 1\n\\p \\v 1 a\\f + \\ft one\\f*\n\\m \\v 2 b\\f + \\ft two\\f*\n\\m c\\f + \\ft three\\f*\n\\p \\v 3 d\\f + \\ft four\\f*\n";
    let document = Parser::new(source).parse(&DEFAULT_STYLESHEET).document;
    let whole = to_html_string(&document, document.style_sheet());

    let mut context = Context::new(document.style_sheet());
    let mut piecewise = String::new();
    let mut in_cartouche = false;
    for block in &document.blocks {
        let m = matches!(block, Block::Para(para) if context.rule(para.style).marker == "m");
        if m != in_cartouche {
            piecewise.push_str(if m {
                "<section class=\"cartouche\">"
            } else {
                "</section>"
            });
            in_cartouche = m;
        }
        block.to_html(&mut piecewise, &mut context).unwrap();
    }

    assert!(
        piecewise.contains("<section class=\"cartouche\">"),
        "{piecewise}"
    );
    let numbers: Vec<String> = note_buttons(&piecewise)
        .into_iter()
        .map(|(_, n)| n)
        .collect();
    assert_eq!(numbers, ["1", "2", "3", "4"]);
    let unwrapped = piecewise
        .replace("<section class=\"cartouche\">", "")
        .replace("</section>", "");
    assert_eq!(unwrapped, whole);
}
