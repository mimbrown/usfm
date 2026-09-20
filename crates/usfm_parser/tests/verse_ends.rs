//! Where verse and chapter end milestones go.
//!
//! The parser emits `VerseEnd` and `ChapterEnd` as it goes (plan D4). These
//! tests are the placement rules in executable form; tcdocs checks the same
//! rules against Paratext's USX at scale, but a rule should be readable in
//! one line here rather than reverse-engineered from a corpus diff.
//!
//! Notation in the expected strings: one block per line, `p:` for a
//! paragraph marker, `tr:` for a table row with cells in `[…]`, `esb:` for a
//! sidebar with its blocks indented; `v1` a verse start, `/1` a verse end,
//! `c1`/`/c1` a chapter start and end, quoted text, `{f …}` a note, `(em …)`
//! a character style.

use usfm_parser::DEFAULT_STYLESHEET;
use usfm_parser::ast::*;
use usfm_parser::parser::Parser;

fn render(source: &str) -> String {
    let document = Parser::new(source).parse(&DEFAULT_STYLESHEET).document;
    let mut out = String::new();
    blocks(&document, &document.blocks, "", &mut out);
    out.trim_end().to_string()
}

fn blocks(document: &Document, list: &[Block], indent: &str, out: &mut String) {
    for block in list {
        out.push_str(indent);
        match block {
            Block::Book(book) => out.push_str(&format!("id {:?}\n", book.code)),
            Block::ChapterStart(c) => out.push_str(&format!("c{}\n", c.number)),
            Block::ChapterEnd(c) => out.push_str(&format!("/c{}\n", c.number)),
            Block::Milestone(m) => out.push_str(&format!("ms {}\n", document.marker(m.style))),
            Block::Para(para) => {
                out.push_str(&format!("{}:", document.marker(para.style)));
                inlines(document, &para.children, out);
                out.push('\n');
            }
            Block::Table(table) => {
                for row in &table.rows {
                    out.push_str(indent);
                    out.push_str("tr:");
                    for cell in &row.cells {
                        out.push_str(" [");
                        inlines(document, &cell.children, out);
                        out.push_str(" ]");
                    }
                    out.push('\n');
                }
            }
            Block::Sidebar(sidebar) => {
                out.push_str("esb:\n");
                blocks(document, &sidebar.blocks, &format!("{indent}  "), out);
            }
            Block::Periph(periph) => {
                out.push_str("periph:\n");
                blocks(document, &periph.blocks, &format!("{indent}  "), out);
            }
        }
    }
}

fn inlines(document: &Document, list: &[Inline], out: &mut String) {
    for inline in list {
        out.push(' ');
        match inline {
            Inline::Text(text) => out.push_str(&format!("{:?}", text.content)),
            Inline::VerseStart(v) => out.push_str(&format!("v{}", v.number)),
            Inline::VerseEnd(v) => out.push_str(&format!("/{}", v.number)),
            Inline::Char(char) => {
                out.push_str(&format!("({}", document.marker(char.style)));
                inlines(document, &char.children, out);
                out.push(')');
            }
            Inline::Note(note) => {
                out.push_str(&format!("{{{}", document.marker(note.style)));
                inlines(document, &note.children, out);
                out.push('}');
            }
            Inline::Milestone(m) => out.push_str(&format!("ms {}", document.marker(m.style))),
            Inline::OptBreak(_) => out.push_str("//"),
        }
    }
}

#[track_caller]
fn check(source: &str, expected: &str) {
    assert_eq!(render(source), expected.trim(), "source:\n{source}");
}

#[test]
fn the_next_verse_in_the_same_paragraph_ends_the_verse_before_it() {
    // The whitespace before `\v` moves after the end, so USX reads
    // `text<verse eid/> <verse number/>`.
    check(
        "\\id GEN\n\\c 1\n\\p \\v 1 In the beginning \\v 2 the earth",
        r#"
id Gen
c1
p: v1 "In the beginning" /1 " " v2 "the earth" /2
/c1"#,
    );
}

#[test]
fn a_verse_starting_a_paragraph_ends_the_verse_before_at_the_end_of_the_previous_paragraph() {
    check(
        "\\id GEN\n\\c 1\n\\p \\v 1 In the beginning\n\\q1 \\v 2 the earth",
        r#"
id Gen
c1
p: v1 "In the beginning" /1
q1: v2 "the earth" /2
/c1"#,
    );
}

#[test]
fn a_verse_crossing_paragraphs_ends_in_the_last_one() {
    check(
        "\\id GEN\n\\c 1\n\\p \\v 1 In the\n\\q1 beginning\n\\q2 God\n\\p \\v 2 the earth",
        r#"
id Gen
c1
p: v1 "In the"
q1: "beginning"
q2: "God" /1
p: v2 "the earth" /2
/c1"#,
    );
}

#[test]
fn a_heading_never_holds_a_verse_end() {
    // `\s` and `\r` are not verse text: the end goes in the last paragraph
    // that is, and the heading gets no `vid` either.
    check(
        "\\id GEN\n\\c 1\n\\p \\v 1 In the beginning\n\\s Heading\n\\r (Luke 1)\n\\p \\v 2 the earth",
        r#"
id Gen
c1
p: v1 "In the beginning" /1
s: "Heading"
r: "(Luke 1)"
p: v2 "the earth" /2
/c1"#,
    );
}

#[test]
fn a_verse_starting_inside_a_non_verse_text_paragraph_ends_the_previous_verse_before_it() {
    // `\lit` is not verse text, so verse 1 ends before it even though verse
    // 2 starts after some text in it …
    check(
        "\\id PSA\n\\c 1\n\\p \\v 1 In the beginning\n\\lit Glory: \\v 2 the earth",
        r#"
id Psa
c1
p: v1 "In the beginning" /1
lit: "Glory: " v2 "the earth" /2
/c1"#,
    );
    // … unless the open verse started in that paragraph too, which makes it
    // the paragraph's own text.
    check(
        "\\id PSA\n\\c 1\n\\lit \\v 1 Glory: \\v 2 the earth",
        r#"
id Psa
c1
lit: v1 "Glory:" /1 " " v2 "the earth" /2
/c1"#,
    );
}

#[test]
fn a_verse_starting_a_character_style_ends_the_previous_verse_before_the_style() {
    check(
        "\\id GEN\n\\c 1\n\\p \\v 1 In the \\nd \\v 2 Lord\\nd* said",
        r#"
id Gen
c1
p: v1 "In the" /1 " " (nd v2 "Lord") " said" /2
/c1"#,
    );
    // Inside the style but after its text, the end goes inside too.
    check(
        "\\id GEN\n\\c 1\n\\p \\v 1 In the \\nd Lord \\v 2 God\\nd* said",
        r#"
id Gen
c1
p: v1 "In the " (nd "Lord" /1 " " v2 "God") " said" /2
/c1"#,
    );
}

#[test]
fn a_note_directly_before_the_next_verse_leaves_no_space() {
    check(
        "\\id GEN\n\\c 1\n\\p \\v 1 In the beginning\\f + \\ft note\\f*\\v 2 the earth",
        r#"
id Gen
c1
p: v1 "In the beginning" {f (ft "note")} /1 v2 "the earth" /2
/c1"#,
    );
}

#[test]
fn verses_in_tables_end_in_the_previous_cell() {
    // At the start of a cell: the end of the previous cell. At the start of
    // a row: the last cell of the previous row. Mid-cell: in the cell.
    check(
        "\\id NUM\n\\c 1\n\\p \\v 1 The count\n\\tr \\tc1 \\v 2 Reuben \\tc2 46,500\n\\tr \\tc1 \\v 3 Simeon \\v 4 Gad \\tc2 59,300\n\\p \\v 5 After",
        r#"
id Num
c1
p: v1 "The count" /1
tr: [ v2 "Reuben" ] [ "46,500" /2 ]
tr: [ v3 "Simeon" /3 " " v4 "Gad" ] [ "59,300" /4 ]
p: v5 "After" /5
/c1"#,
    );
}

#[test]
fn a_chapter_boundary_ends_the_open_verse_and_chapter() {
    // The verse ends in the last verse-text paragraph of its chapter; the
    // chapter ends right before the next one, after any headings.
    check(
        "\\id GEN\n\\c 1\n\\p \\v 1 In the beginning\n\\s Heading\n\\c 2\n\\p \\v 1 The heavens",
        r#"
id Gen
c1
p: v1 "In the beginning" /1
s: "Heading"
/c1
c2
p: v1 "The heavens" /1
/c2"#,
    );
}

#[test]
fn a_sidebar_neither_ends_nor_holds_a_verse() {
    // The verse open before `\esb` is still open after `\esbe`; a `\v`
    // inside the sidebar is not tracked at all.
    check(
        "\\id GEN\n\\c 1\n\\p \\v 1 In the beginning\n\\esb\n\\p Aside \\v 9 stray\n\\esbe\n\\p God \\v 2 the earth",
        r#"
id Gen
c1
p: v1 "In the beginning"
esb:
  p: "Aside " v9 "stray"
p: "God" /1 " " v2 "the earth" /2
/c1"#,
    );
}

/// Ticket 28, found by ticket 27's round-trip fuzz target on machine.py's
/// `41MATTes.SFM`. `\esbe` opens a paragraph like any other marker, so a `\v`
/// on the next line with no paragraph marker of its own belongs to *that*
/// paragraph — and the end of the verse open before `\esb`, which cannot go
/// inside the sidebar, was being placed into the `\esbe` paragraph's own empty
/// block list and silently dropped. It goes where it goes when a `\p` does
/// follow `\esbe`: at the end of the last verse-text block before the sidebar.
#[test]
fn a_verse_after_esbe_with_no_paragraph_marker_ends_the_one_before_the_sidebar() {
    check(
        "\\id GEN\n\\c 1\n\\p \\v 1 In the beginning\n\\esb\n\\p Aside\n\\esbe\n\\v 2 the earth",
        r#"
id Gen
c1
p: v1 "In the beginning" /1
esb:
  p: "Aside"
p: v2 "the earth" /2
/c1"#,
    );
}

/// Content *on* the `\esbe` line becomes an implicit `\p`, so a verse there
/// places its predecessor's end the way a `\p` would — inline, after the text
/// already in the paragraph — not before the sidebar. Only a verse with
/// nothing before it on that line ends the one before it outside the sidebar,
/// which is the case above. The round-trip fuzz target found the difference
/// (ticket 27): the two spellings must agree, since the writer turns the first
/// into the second.
#[test]
fn a_verse_after_text_on_the_esbe_line_ends_the_previous_one_inline() {
    let expected = r#"
id Gen
c1
p: v1 "In the beginning"
esb:
p: "God" /1 " " v2 "the earth" /2
/c1"#;
    check(
        "\\id GEN\n\\c 1\n\\p \\v 1 In the beginning\n\\esb\n\\esbe God \\v 2 the earth",
        expected,
    );
    check(
        "\\id GEN\n\\c 1\n\\p \\v 1 In the beginning\n\\esb\n\\esbe\n\\p God \\v 2 the earth",
        expected,
    );
}

/// The same input with the `\p` the marker did not need: the two must agree,
/// which is the whole of ticket 28.
#[test]
fn a_paragraph_marker_after_esbe_does_not_change_where_the_verse_ends() {
    check(
        "\\id GEN\n\\c 1\n\\p \\v 1 In the beginning\n\\esb\n\\p Aside\n\\esbe\n\\p \\v 2 the earth",
        r#"
id Gen
c1
p: v1 "In the beginning" /1
esb:
  p: "Aside"
p: v2 "the earth" /2
/c1"#,
    );
}

/// Peripheral matter closes no verse, and that has to include the `\periph`
/// line itself. A `\v` there used to emit the open verse's end into the
/// paragraph *before* the periph while its own `VerseStart` was thrown away
/// with the rest of the title line, leaving a `VerseEnd` with no `VerseStart`
/// — a tree no source produces and no writer can write back. The round-trip
/// fuzz target found it on `" i\periph\v 2"`, and verse tracking is suspended
/// across a periph since (ticket 27).
///
/// The `VerseStart` itself is kept now (ticket 35): only text is the title,
/// and everything else on the line opens an implicit `\p` at the head of the
/// division rather than vanishing. That is the same tree `\periph` followed by
/// `\p \v 2` has always given, which is what the writer writes — and no end
/// comes with it either way, because the periph suspends tracking.
#[test]
fn a_verse_on_a_periph_line_opens_no_verse_end() {
    let expected = r#"
id Gen
c1
p: "a"
periph:
  p: v2
/c1"#;
    check("\\id GEN\n\\c 1\n\\p a\n\\periph\\v 2", expected);
    // The spelling the writer produces, which must read back the same.
    check("\\id GEN\n\\c 1\n\\p a\n\\periph\n\\p \\v 2", expected);
}

#[test]
fn end_milestones_can_be_switched_off() {
    let document = Parser::new("\\id GEN\n\\c 1\n\\p \\v 1 In the beginning \\v 2 the earth")
        .parse_with_options(&DEFAULT_STYLESHEET, false)
        .document;
    let mut out = String::new();
    blocks(&document, &document.blocks, "", &mut out);
    assert_eq!(
        out.trim_end(),
        r#"
id Gen
c1
p: v1 "In the beginning " v2 "the earth""#
            .trim()
    );
}
