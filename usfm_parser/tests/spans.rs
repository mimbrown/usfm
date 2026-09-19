//! Spans point at the source they were read from.
//!
//! The snapshot corpus records span values, but a recorded value is only as
//! good as the eye that accepted it. These tests check the invariants
//! mechanically, so a span cannot be silently wrong in a way that still looks
//! plausible in a snapshot.

mod common;

use usfm_parser::DEFAULT_STYLESHEET;
use usfm_parser::ast::*;
use usfm_parser::parser::Parser;

/// Every node in `source`, as (label, span, expected source prefix).
/// A prefix of `None` means the node is synthesized and must carry `SPAN`.
fn nodes<'a>(document: &'a Document<'a>) -> Vec<(String, Span, Option<String>)> {
    let mut out = Vec::new();
    blocks(document, &document.blocks, &mut out);
    out
}

fn blocks<'a>(
    document: &'a Document<'a>,
    list: &'a [Block<'a>],
    out: &mut Vec<(String, Span, Option<String>)>,
) {
    for block in list {
        match block {
            Block::Book(book) => out.push(("Book".into(), book.span, Some("\\id".into()))),
            Block::ChapterStart(c) => out.push(("ChapterStart".into(), c.span, Some("\\c".into()))),
            Block::ChapterEnd(c) => out.push(("ChapterEnd".into(), c.span, None)),
            Block::Milestone(m) => {
                let marker = document.marker(m.style).to_string();
                out.push((
                    format!("Milestone {marker}"),
                    m.span,
                    Some(format!("\\{marker}")),
                ));
            }
            Block::Para(para) => {
                let marker = document.marker(para.style).to_string();
                out.push((
                    format!("Para {marker}"),
                    para.span,
                    Some(format!("\\{marker}")),
                ));
                inlines(document, &para.children, out);
            }
            Block::Table(table) => {
                out.push(("Table".into(), table.span, Some("\\tr".into())));
                for row in &table.rows {
                    out.push(("TableRow".into(), row.span, Some("\\tr".into())));
                    for cell in &row.cells {
                        out.push(("TableCell".into(), cell.span, Some("\\t".into())));
                        inlines(document, &cell.children, out);
                    }
                }
            }
            Block::Periph(periph) => {
                let marker = document.marker(periph.style).to_string();
                out.push((
                    format!("Periph {marker}"),
                    periph.span,
                    Some(format!("\\{marker}")),
                ));
                if let Some(title) = &periph.title {
                    out.push(("Periph title".into(), title.span, Some(title.content.chars().next().unwrap().to_string())));
                }
                blocks(document, &periph.blocks, out);
            }
            Block::Sidebar(sidebar) => {
                let marker = document.marker(sidebar.style).to_string();
                out.push((
                    format!("Sidebar {marker}"),
                    sidebar.span,
                    Some(format!("\\{marker}")),
                ));
                if let Some(category) = &sidebar.category {
                    out.push(("Sidebar category".into(), category.span, Some("\\cat".into())));
                }
                blocks(document, &sidebar.blocks, out);
            }
        }
    }
}

fn inlines<'a>(
    document: &'a Document<'a>,
    children: &'a [Inline<'a>],
    out: &mut Vec<(String, Span, Option<String>)>,
) {
    for child in children {
        match child {
            // Text is the one node whose span is a source range but whose
            // content is not that range verbatim, so there is no prefix to
            // check beyond the span being in bounds.
            Inline::Text(text) => out.push((format!("Text {:?}", text.content), text.span, None)),
            Inline::VerseStart(v) => out.push(("VerseStart".into(), v.span, Some("\\v".into()))),
            Inline::VerseEnd(v) => {
                assert_eq!(v.span, SPAN, "verse ends are synthesized");
                out.push(("VerseEnd".into(), v.span, None));
            }
            Inline::Char(char) => {
                let marker = document.marker(char.style).to_string();
                out.push((
                    format!("Char {marker}"),
                    char.span,
                    Some(format!("\\{marker}")),
                ));
                inlines(document, &char.children, out);
            }
            Inline::Note(note) => {
                let marker = document.marker(note.style).to_string();
                out.push((
                    format!("Note {marker}"),
                    note.span,
                    Some(format!("\\{marker}")),
                ));
                if let Some(category) = &note.category {
                    out.push(("Note category".into(), category.span, Some("\\cat".into())));
                }
                inlines(document, &note.children, out);
            }
            Inline::Milestone(m) => {
                let marker = document.marker(m.style).to_string();
                out.push((
                    format!("Milestone {marker}"),
                    m.span,
                    Some(format!("\\{marker}")),
                ));
            }
            Inline::OptBreak(b) => out.push(("OptBreak".into(), b.span, Some("//".into()))),
        }
    }
}

fn check(source: &str) {
    let result = Parser::new(source).parse(&DEFAULT_STYLESHEET);
    let document = &result.document;
    for (label, span, prefix) in nodes(document) {
        assert!(
            span.start <= span.end,
            "{label}: span {span:?} is inverted in {source:?}"
        );
        assert!(
            span.end as usize <= source.len(),
            "{label}: span {span:?} runs past the end of {source:?}"
        );
        if span == SPAN {
            continue;
        }
        let slice = &source[span.start as usize..span.end as usize];
        if let Some(prefix) = prefix {
            // A nested character style is written `\+add`, so accept the `+`.
            let nested = prefix.replacen('\\', "\\+", 1);
            assert!(
                slice.starts_with(&prefix) || slice.starts_with(&nested),
                "{label}: span {span:?} is {slice:?}, which does not start with \
                 {prefix:?} or {nested:?}"
            );
        }
    }
}

#[test]
fn simple_paragraph() {
    check("\\id GEN\n\\c 1\n\\p \\v 1 In the beginning.\n");
}

#[test]
fn nested_character_styles() {
    check("\\id GEN\n\\c 1\n\\p \\v 1 a \\nd Lord\\+add ly\\+add*\\nd* b\n");
}

#[test]
fn implicitly_closed_character_style() {
    check("\\id GEN\n\\c 1\n\\p \\v 1 a \\em b\n\\p \\v 2 c\n");
}

#[test]
fn optional_line_breaks() {
    check("\\id GEN\n\\c 1\n\\s1 Jesus Heals a Man // Who Could Not Walk\n\\p \\v 1 a//b\n");
}

#[test]
fn notes_and_attributes() {
    check("\\id GEN\n\\c 1\n\\p \\v 1 \\w grace|lemma=\"grace\"\\w* \\f + \\ft note\\f* end\n");
}

#[test]
fn milestones_inline_and_between_blocks() {
    check("\\id GEN\n\\c 1\n\\ts\\*\n\\p \\v 1 a \\qt-s |who=\"Jesus\"\\* b \\qt-e\\* c\n");
}

#[test]
fn chapter_with_alternate_and_published_numbers() {
    check("\\id MAT\n\\c 1\n\\ca 2\\ca*\n\\cp M\n\\p \\v 1 text\n");
}

#[test]
fn table() {
    check("\\id GEN\n\\c 1\n\\tr \\th1 Header \\th2 Second\n\\tr \\tc1 One \\tc2 Two\n");
}

#[test]
fn malformed_input_still_has_sane_spans() {
    check("\\id GEN\n\\c 1\n\\p \\v 1 a \\em b \\foo c \\* d \\bk* e\n");
}

#[test]
fn normalized_whitespace_and_escapes() {
    check("\\id GEN\n\\c 1\n\\p \\v 1 a\\\\b   c\nd \\em e\\em*\n");
}

/// `into_owned` must detach the document from the source (plan D5): the
/// document has to still be usable, and unchanged, once the input is gone.
#[test]
fn into_owned_outlives_the_source() {
    fn parse_detached(source: &str) -> Document<'static> {
        // `source` is borrowed only inside this function, so the returned
        // document cannot be borrowing it — the signature is the assertion.
        Parser::new(source)
            .parse(&DEFAULT_STYLESHEET)
            .document
            .into_owned()
    }

    let source = String::from(
        "\\id GEN Genesis\n\\c 1\n\\ca 2\\ca*\n\\cp M\n\\p \\v 1 \\w grace|lemma=\"grace\"\\w* \
         \\f + \\ft note\\f* \\zaln-s |x-strong=\"G1\"\\* a \\zaln-e\\*\n\
         \\tr \\tc1 one \\tc2 two\n",
    );
    let borrowed = Parser::new(&source).parse(&DEFAULT_STYLESHEET).document;
    let rendered = format!("{borrowed:?}");

    let owned = parse_detached(&source);
    drop(source);

    // Same tree, and still resolvable: the stylesheet is shared, not copied.
    assert_eq!(format!("{owned:?}"), rendered);
    let Block::Para(para) = &owned.blocks[2] else {
        panic!("expected a paragraph, got {:?}", owned.blocks[2]);
    };
    assert_eq!(owned.marker(para.style), "p");
}
