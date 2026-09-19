//! The span invariants, checked mechanically.
//!
//! Spans point at the source they were read from. The snapshot corpus records
//! span values, but a recorded value is only as good as the eye that accepted
//! it, so these checks say what a span must satisfy no matter what the input
//! was: in bounds, not inverted, on a character boundary, and — for every node
//! but `Text` — starting at the marker the node was read from, or at its
//! content when the parser had to open the node itself to recover.
//!
//! The module is shared: `usfm_parser/tests/spans.rs` runs it over hand-written
//! inputs, and `tasks/fuzz` runs it over whatever libFuzzer invents. It is
//! behind the `testing` feature, so the default build does not carry it.

use std::sync::Arc;

use usfm_style::StyleSheet;

use crate::ast::*;
use crate::diagnostics::{Code, ParseResult};
use crate::parser::Parser;

/// What the start of a node's span has to look like in the source.
pub enum Prefix {
    /// Nothing to check beyond the span itself: the node is synthesized, or it
    /// is a `Text`, whose content is not its source range verbatim (plan D2).
    Anything,
    /// The span must start with this text: the marker the node was read from.
    Marker(String),
    /// The span must start with this marker unless the parser opened the node
    /// itself to recover, which it reports as `code`. An implicit `\p` has no
    /// marker in the source: it must then start at or before the content it
    /// holds, which begins at `content_start` (`None` when every child is
    /// synthesized and there is no content to start at).
    MarkerOrImplicit {
        marker: String,
        code: Code,
        content_start: Option<u32>,
    },
}

/// One node, as the checker sees it.
pub struct NodeSpan {
    /// How the node is named in a failure message.
    pub label: String,
    pub span: Span,
    pub prefix: Prefix,
}

fn push(out: &mut Vec<NodeSpan>, label: impl Into<String>, span: Span, prefix: Prefix) {
    out.push(NodeSpan {
        label: label.into(),
        span,
        prefix,
    });
}

/// The marker a node was read from.
fn read_from(marker: impl std::fmt::Display) -> Prefix {
    Prefix::Marker(marker.to_string())
}

/// Where the first child that was read from the source begins, if any.
fn content_start(children: &[Inline<'_>]) -> Option<u32> {
    children
        .iter()
        .map(|child| match child {
            Inline::Text(text) => text.span,
            Inline::VerseStart(verse) => verse.span,
            Inline::VerseEnd(verse) => verse.span,
            Inline::Char(char) => char.span,
            Inline::Note(note) => note.span,
            Inline::Milestone(milestone) => milestone.span,
            Inline::OptBreak(opt_break) => opt_break.span,
        })
        .find(|span| *span != SPAN)
        .map(|span| span.start)
}

/// Every node in `document`, in document order.
pub fn nodes<'a>(document: &'a Document<'a>) -> Vec<NodeSpan> {
    let mut out = Vec::new();
    blocks(document, &document.blocks, &mut out);
    out
}

fn blocks<'a>(document: &'a Document<'a>, list: &'a [Block<'a>], out: &mut Vec<NodeSpan>) {
    for block in list {
        match block {
            Block::Book(book) => push(out, "Book", book.span, read_from("\\id")),
            Block::ChapterStart(c) => push(out, "ChapterStart", c.span, read_from("\\c")),
            Block::ChapterEnd(c) => push(out, "ChapterEnd", c.span, Prefix::Anything),
            Block::Milestone(milestone) => {
                let name = document.marker(milestone.style);
                push(
                    out,
                    format!("Milestone {name}"),
                    milestone.span,
                    read_from(format_args!("\\{name}")),
                );
            }
            Block::Para(para) => {
                let name = document.marker(para.style);
                // Content outside a paragraph is kept in an implicit `\p`
                // (`Code::ContentOutsideParagraph`), which has no marker of its
                // own: its span starts where its content does.
                let prefix = Prefix::MarkerOrImplicit {
                    marker: format!("\\{name}"),
                    code: Code::ContentOutsideParagraph,
                    content_start: content_start(&para.children),
                };
                push(out, format!("Para {name}"), para.span, prefix);
                inlines(document, &para.children, out);
            }
            Block::Table(table) => {
                push(out, "Table", table.span, read_from("\\tr"));
                for row in &table.rows {
                    push(out, "TableRow", row.span, read_from("\\tr"));
                    for cell in &row.cells {
                        // `\tr` with no cell marker opens an implicit `\tc1`
                        // (`Code::ExpectedTableCell`), as above.
                        let prefix = Prefix::MarkerOrImplicit {
                            marker: "\\t".into(),
                            code: Code::ExpectedTableCell,
                            content_start: content_start(&cell.children),
                        };
                        push(out, "TableCell", cell.span, prefix);
                        inlines(document, &cell.children, out);
                    }
                }
            }
            Block::Periph(periph) => {
                let name = document.marker(periph.style);
                push(
                    out,
                    format!("Periph {name}"),
                    periph.span,
                    read_from(format_args!("\\{name}")),
                );
                if let Some(title) = &periph.title {
                    // The title is a `Text`: its span is the source it was
                    // read from and its content is that source normalised —
                    // trimmed, and with any markers on the line left out
                    // (`\periph\* n` has a title of "n" spanning " n"). So
                    // there is nothing to check but the span itself, as for
                    // every other `Text` (plan D2).
                    push(out, "Periph title", title.span, Prefix::Anything);
                }
                blocks(document, &periph.blocks, out);
            }
            Block::Sidebar(sidebar) => {
                let name = document.marker(sidebar.style);
                push(
                    out,
                    format!("Sidebar {name}"),
                    sidebar.span,
                    read_from(format_args!("\\{name}")),
                );
                if let Some(category) = &sidebar.category {
                    push(out, "Sidebar category", category.span, read_from("\\cat"));
                }
                blocks(document, &sidebar.blocks, out);
            }
        }
    }
}

fn inlines<'a>(document: &'a Document<'a>, children: &'a [Inline<'a>], out: &mut Vec<NodeSpan>) {
    for child in children {
        match child {
            // Text is the one node whose span is a source range but whose
            // content is not that range verbatim, so there is no prefix to
            // check beyond the span being in bounds.
            Inline::Text(text) => {
                push(
                    out,
                    format!("Text {:?}", text.content),
                    text.span,
                    Prefix::Anything,
                );
            }
            Inline::VerseStart(verse) => push(out, "VerseStart", verse.span, read_from("\\v")),
            Inline::VerseEnd(verse) => {
                assert_eq!(verse.span, SPAN, "verse ends are synthesized");
                push(out, "VerseEnd", verse.span, Prefix::Anything);
            }
            Inline::Char(char) => {
                let name = document.marker(char.style);
                push(
                    out,
                    format!("Char {name}"),
                    char.span,
                    read_from(format_args!("\\{name}")),
                );
                inlines(document, &char.children, out);
            }
            Inline::Note(note) => {
                let name = document.marker(note.style);
                push(
                    out,
                    format!("Note {name}"),
                    note.span,
                    read_from(format_args!("\\{name}")),
                );
                if let Some(category) = &note.category {
                    push(out, "Note category", category.span, read_from("\\cat"));
                }
                inlines(document, &note.children, out);
            }
            Inline::Milestone(milestone) => {
                let name = document.marker(milestone.style);
                push(
                    out,
                    format!("Milestone {name}"),
                    milestone.span,
                    read_from(format_args!("\\{name}")),
                );
            }
            Inline::OptBreak(opt_break) => push(out, "OptBreak", opt_break.span, read_from("//")),
        }
    }
}

/// Assert the span invariants for `result`, the parse of `source`.
///
/// Panics with the offending node, its span and the source on the first
/// violation.
pub fn check_parse(source: &str, result: &ParseResult<'_>) {
    let reported = |code: Code| result.diagnostics.iter().any(|d| d.code == code);
    for NodeSpan {
        label,
        span,
        prefix,
    } in nodes(&result.document)
    {
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
        assert!(
            source.is_char_boundary(span.start as usize)
                && source.is_char_boundary(span.end as usize),
            "{label}: span {span:?} splits a character in {source:?}"
        );
        let slice = &source[span.start as usize..span.end as usize];
        let (marker, implicit) = match prefix {
            Prefix::Anything => continue,
            Prefix::Marker(marker) => (marker, None),
            Prefix::MarkerOrImplicit {
                marker,
                code,
                content_start,
            } => (marker, Some((code, content_start))),
        };
        // A nested character style is written `\+add`, so accept the `+`.
        let nested = marker.replacen('\\', "\\+", 1);
        if slice.starts_with(&marker) || slice.starts_with(&nested) {
            continue;
        }
        if let Some((code, content_start)) = implicit {
            // The node has no marker in the source. That is allowed only if
            // the parser said it opened one to recover, and then the span
            // still has to begin no later than the content it holds.
            assert!(
                reported(code),
                "{label}: span {span:?} is {slice:?}, which does not start with \
                 {marker:?}, and no {code} was reported"
            );
            if let Some(content_start) = content_start {
                // It starts where the parser opened it, which is at or before
                // the first child that survived — `\v\` opens a paragraph at
                // the `\v`, then drops the verse for want of a number.
                assert!(
                    span.start <= content_start,
                    "{label}: span {span:?} has no marker, so it should start at \
                     or before its content ({content_start}) in {source:?}"
                );
            }
            continue;
        }
        panic!(
            "{label}: span {span:?} is {slice:?}, which does not start with \
             {marker:?} or {nested:?}"
        );
    }
}

/// Parse `source` with `style_sheet` and assert the span invariants.
pub fn check_with(source: &str, style_sheet: &Arc<StyleSheet>) {
    check_parse(source, &Parser::new(source).parse(style_sheet));
}

/// Parse `source` with the default stylesheet and assert the span invariants.
pub fn check(source: &str) {
    check_with(source, &crate::DEFAULT_STYLESHEET);
}
