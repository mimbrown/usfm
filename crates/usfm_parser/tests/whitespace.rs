//! The whitespace rules, tested on the tree rather than through USX.
//!
//! The rules are listed on `Text` in `usfm_ast`; each test here is one of
//! them. The tcdocs suite exercises the same rules end to end, but its
//! reference files are inconsistent about note ends and the harness
//! normalises those away, so this file is where the behaviour is pinned.

use usfm_parser::DEFAULT_STYLESHEET;
use usfm_parser::ast::*;
use usfm_parser::parser::Parser;

/// Render the inline children of every paragraph, one paragraph per line.
/// Text is quoted verbatim so its edges are visible; containers are
/// bracketed.
fn render(source: &str) -> String {
    let document = Parser::new(source)
        .parse_with_options(&DEFAULT_STYLESHEET, false)
        .document;
    let mut lines = vec![];
    for block in &document.blocks {
        match block {
            Block::Para(para) => lines.push(inlines(&para.children)),
            Block::Table(table) => {
                for row in &table.rows {
                    let cells: Vec<String> = row.cells.iter().map(|c| inlines(&c.children)).collect();
                    lines.push(cells.join(" | "));
                }
            }
            _ => {}
        }
    }
    lines.join("\n")
}

fn inlines(children: &[Inline<'_>]) -> String {
    children
        .iter()
        .map(|inline| match inline {
            Inline::Text(text) => format!("\"{}\"", text.content),
            Inline::Char(char) => format!("[{}]", inlines(&char.children)),
            Inline::Note(note) => format!("{{{}}}", inlines(&note.children)),
            Inline::VerseStart(_) => "<v>".to_string(),
            Inline::VerseEnd(_) => "</v>".to_string(),
            Inline::Milestone(_) => "<ms>".to_string(),
            Inline::OptBreak(_) => "//".to_string(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn para(body: &str) -> String {
    render(&format!("\\id GEN\n\\c 1\n\\p {body}"))
}

#[test]
fn ascii_whitespace_runs_collapse_to_one_space() {
    assert_eq!(para("a \t b\nc\r\n  d"), r#""a b c d""#);
}

/// `//` is an optional line break, its own node; the whitespace around it
/// stays in the text on either side (USX `Man <optbreak/> Who`).
#[test]
fn a_double_slash_is_an_optional_line_break() {
    assert_eq!(
        para("Jesus Heals a Man // Who Could Not Walk"),
        r#""Jesus Heals a Man " // " Who Could Not Walk""#
    );
    assert_eq!(para("Man//Who"), r#""Man" // "Who""#);
}

#[test]
fn a_line_break_inside_a_paragraph_is_a_space() {
    assert_eq!(para("one\ntwo"), r#""one two""#);
}

#[test]
fn only_ascii_whitespace_is_normalised() {
    // NBSP and ideographic space are content: not collapsed, not trimmed,
    // even at the end of the paragraph.
    assert_eq!(
        para("a\u{a0}\u{a0}b \u{3000}c\u{3000}"),
        "\"a\u{a0}\u{a0}b \u{3000}c\u{3000}\""
    );
}

#[test]
fn tilde_is_a_no_break_space() {
    assert_eq!(para("verse text~with space"), "\"verse text\u{a0}with space\"");
    // Rewriting the content leaves the span on the source run.
    let document = Parser::new("\\id GEN\n\\c 1\n\\p a~b")
        .parse(&DEFAULT_STYLESHEET)
        .document;
    let Block::Para(para) = &document.blocks[2] else {
        panic!("expected a paragraph");
    };
    let Inline::Text(text) = &para.children[0] else {
        panic!("expected text");
    };
    assert_eq!(text.content, "a\u{a0}b");
    assert_eq!(text.span, usfm_parser::lexer::span::Span::new(16, 19));
}

#[test]
fn escapes_resolve_to_the_literal() {
    assert_eq!(para(r#"a\\b \|c \"d"#), r#""a\b |c "d""#);
}

#[test]
fn trailing_whitespace_is_dropped_before_a_paragraph_marker() {
    assert_eq!(render("\\id GEN\n\\c 1\n\\p one \n\\p two \t\n\\q1 three"), "\"one\"\n\"two\"\n\"three\"");
}

#[test]
fn trailing_whitespace_is_dropped_before_a_chapter_marker_and_at_eof() {
    assert_eq!(render("\\id GEN\n\\c 1\n\\p one \n\\c 2\n\\p two \n"), "\"one\"\n\"two\"");
    assert_eq!(render("\\id GEN\n\\c 1\n\\p end   "), "\"end\"");
}

#[test]
fn trailing_whitespace_is_dropped_before_the_next_table_cell() {
    assert_eq!(
        render("\\id GEN\n\\c 1\n\\tr \\tc1 one \\tc2 two \n\\p"),
        "\"one\" | \"two\"\n"
    );
}

#[test]
fn trailing_whitespace_is_kept_before_a_closing_marker() {
    assert_eq!(para("\\add foo \\add* bar"), r#"["foo "] " bar""#);
    // Also when the closed style ends the paragraph: the run ended at
    // `\add*`, and the newline after it is dropped on its own.
    assert_eq!(render("\\id GEN\n\\c 1\n\\p a \\bdit (Jas 3:9) \\bdit*\n\\p b"), "\"a \" [\"(Jas 3:9) \"]\n\"b\"");
}

#[test]
fn trailing_whitespace_is_kept_before_a_sibling_character_style() {
    // An unclosed `\nd` is a sibling that implicitly closes `\add`.
    assert_eq!(para("\\add foo \\nd bar"), r#"["foo "] ["bar"]"#);
}

#[test]
fn trailing_whitespace_is_kept_before_a_note_and_inside_it() {
    // Paratext keeps `text ` before `\f*`; so do we (the tcdocs harness
    // ignores note ends, this test does not).
    assert_eq!(
        para("word \\f + \\fr 1.1 \\ft text \\f* after"),
        r#""word " {["1.1 "] ["text "]} " after""#
    );
    assert_eq!(
        para("\\f + \\ft \\+em \\+pn name\\+pn* stuff \\+em*\\f*"),
        r#"{[[["name"] " stuff "]]}"#
    );
}

#[test]
fn trailing_whitespace_is_kept_before_a_verse_and_a_milestone() {
    assert_eq!(para("\\v 1 one \\v 2 two"), r#"<v> "one " <v> "two""#);
    assert_eq!(
        para("one\n\\qt-s |who=\"Pilate\"\\*two\\qt-e\\*"),
        r#""one " <ms> "two" <ms>"#
    );
}

#[test]
fn an_unclosed_character_style_is_trimmed_at_the_paragraph_end() {
    // The run inside `\add` ended at `\p`, a paragraph-level boundary.
    assert_eq!(render("\\id GEN\n\\c 1\n\\p a \\add foo \n\\p b"), "\"a \" [\"foo\"]\n\"b\"");
}

#[test]
fn leading_whitespace_after_a_marker_is_not_text() {
    assert_eq!(para("  \\v 1   text"), r#"<v> "text""#);
}
