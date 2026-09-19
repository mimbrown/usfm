//! Shared helpers for integration tests: parse a source and render the
//! resulting tree and diagnostics as reviewable text.
//!
//! The same renderer as `usfm_parser/tests/common/mod.rs`, deliberately
//! byte-for-byte in its output so that a snapshot moving between the two
//! suites (ticket 19 moved `unlisted_book_code`) changes only its header. The
//! one difference is the parse: this copy goes through `usfm::parse`, so what
//! it renders is the union of the parser's diagnostics and
//! `usfm_semantic::analyze`'s — which is the thing under test here.

#![allow(dead_code)]

use std::fmt::Write;
use std::panic::{self, AssertUnwindSafe};

use usfm::ast::*;
use usfm::diagnostics::Code;
use usfm::span::{SPAN, Span};
use usfm::style::StyleSheet;

/// Parse `source` and render the tree followed by any diagnostics.
/// A panic is rendered as `PANIC: ...` so snapshot tests can record it.
pub fn render(source: &str) -> String {
    let result = panic::catch_unwind(AssertUnwindSafe(|| usfm::parse(source)));
    match result {
        Ok(result) => {
            let mut out = String::new();
            // Resolve against the document's own stylesheet, not the base one:
            // they differ whenever the parser derived a style (plan D3).
            Printer::new(result.document.style_sheet()).document(&mut out, &result.document);
            if !result.diagnostics.is_empty() {
                out.push_str("--- diagnostics\n");
                for diagnostic in &result.diagnostics {
                    let _ = writeln!(out, "{diagnostic}");
                }
            }
            out
        }
        Err(payload) => {
            let msg = payload
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "unknown panic".to_string());
            format!("PANIC: {msg}\n")
        }
    }
}

/// The diagnostic codes emitted for `source`, in order.
pub fn codes(source: &str) -> Vec<Code> {
    usfm::parse(source)
        .diagnostics
        .iter()
        .map(|d| d.code)
        .collect()
}

/// The codes the parser alone emits, for asserting that a moved check is no
/// longer the parser's.
pub fn parser_codes(source: &str) -> Vec<Code> {
    usfm::parser::parser::Parser::new(source)
        .parse(&usfm::DEFAULT_STYLESHEET)
        .diagnostics
        .iter()
        .map(|d| d.code)
        .collect()
}

/// `@start..end`, or nothing for a synthesized node. Rendering spans is what
/// makes the corpus a regression test for them.
fn span(span: Span) -> String {
    if span == SPAN {
        String::new()
    } else {
        format!("@{}..{}", span.start, span.end)
    }
}

/// Compact, indentation-based AST renderer. Style indices are resolved to
/// marker names so snapshots survive changes to style representation.
pub struct Printer<'s> {
    style_sheet: &'s StyleSheet,
}

impl<'s> Printer<'s> {
    pub fn new(style_sheet: &'s StyleSheet) -> Self {
        Self { style_sheet }
    }

    fn marker(&self, style: StyleId) -> String {
        match self.style_sheet.rules.get(style.index()) {
            Some(rule) => rule.marker.clone(),
            None => format!("{style}"),
        }
    }

    pub fn document(&self, out: &mut String, document: &Document) {
        for block in &document.blocks {
            self.block(out, block, 0);
        }
    }

    fn block(&self, out: &mut String, block: &Block, depth: usize) {
        let pad = "  ".repeat(depth);
        match block {
            Block::Book(book) => {
                let _ = writeln!(
                    out,
                    "{pad}Book {} {:?} {}",
                    book.code,
                    book.description,
                    span(book.span)
                );
            }
            Block::ChapterStart(chapter) => {
                let _ = write!(
                    out,
                    "{pad}ChapterStart {}{}",
                    chapter.number,
                    span(chapter.span)
                );
                if let Some(alt) = &chapter.alt_number {
                    let _ = write!(out, " alt={alt}");
                }
                if let Some(pub_number) = &chapter.pub_number {
                    let _ = write!(out, " pub={pub_number:?}");
                }
                out.push('\n');
            }
            Block::ChapterEnd(chapter) => {
                let _ = writeln!(
                    out,
                    "{pad}ChapterEnd {}{}",
                    chapter.number,
                    span(chapter.span)
                );
            }
            Block::Milestone(milestone) => {
                let _ = write!(
                    out,
                    "{pad}Milestone {}{}",
                    self.marker(milestone.style),
                    span(milestone.span)
                );
                self.attributes(out, &milestone.attributes);
                out.push('\n');
            }
            Block::Para(para) => {
                let _ = writeln!(
                    out,
                    "{pad}Para {}{}",
                    self.marker(para.style),
                    span(para.span)
                );
                self.inlines(out, &para.children, depth + 1);
            }
            Block::Periph(periph) => {
                let _ = write!(
                    out,
                    "{pad}Periph {}{}",
                    self.marker(periph.style),
                    span(periph.span)
                );
                if let Some(title) = &periph.title {
                    let _ = write!(out, " title={:?}{}", title.content, span(title.span));
                }
                if let Some(attributes) = &periph.attributes {
                    self.attributes(out, attributes);
                }
                out.push('\n');
                for block in &periph.blocks {
                    self.block(out, block, depth + 1);
                }
            }
            Block::Sidebar(sidebar) => {
                let _ = write!(
                    out,
                    "{pad}Sidebar {}{}",
                    self.marker(sidebar.style),
                    span(sidebar.span)
                );
                if let Some(category) = &sidebar.category {
                    let _ = write!(out, " category={:?}{}", category.content, span(category.span));
                }
                out.push('\n');
                for block in &sidebar.blocks {
                    self.block(out, block, depth + 1);
                }
            }
            Block::Table(table) => {
                let _ = writeln!(out, "{pad}Table{}", span(table.span));
                for row in &table.rows {
                    let _ = writeln!(out, "{pad}  Row{}", span(row.span));
                    for cell in &row.cells {
                        let _ = writeln!(
                            out,
                            "{pad}    Cell header={} align={} column={} colspan={}{}",
                            cell.header,
                            cell.alignment,
                            cell.column,
                            cell.colspan,
                            span(cell.span)
                        );
                        self.inlines(out, &cell.children, depth + 3);
                    }
                }
            }
        }
    }

    fn inlines(&self, out: &mut String, inlines: &[Inline], depth: usize) {
        for inline in inlines {
            self.inline(out, inline, depth);
        }
    }

    fn attributes(&self, out: &mut String, attributes: &Attributes) {
        for attr in &attributes.pairs {
            if attr.name.is_empty() {
                let _ = write!(out, " {:?}", attr.value);
            } else {
                let _ = write!(out, " {}={:?}", attr.name, attr.value);
            }
        }
    }

    fn inline(&self, out: &mut String, inline: &Inline, depth: usize) {
        let pad = "  ".repeat(depth);
        match inline {
            Inline::Text(text) => {
                let _ = writeln!(out, "{pad}Text {:?}{}", text.content, span(text.span));
            }
            Inline::VerseStart(verse) => {
                let _ = write!(out, "{pad}VerseStart {}{}", verse.number, span(verse.span));
                if let Some(alt) = &verse.alt_number {
                    let _ = write!(out, " alt={alt}");
                }
                if let Some(pub_number) = &verse.pub_number {
                    let _ = write!(out, " pub={pub_number:?}");
                }
                out.push('\n');
            }
            Inline::VerseEnd(verse) => {
                let _ = writeln!(out, "{pad}VerseEnd {}{}", verse.number, span(verse.span));
            }
            Inline::Char(char) => {
                let _ = write!(
                    out,
                    "{pad}Char {}{}",
                    self.marker(char.style),
                    span(char.span)
                );
                if let Some(attributes) = &char.attributes {
                    self.attributes(out, attributes);
                }
                out.push('\n');
                self.inlines(out, &char.children, depth + 1);
            }
            Inline::Note(note) => {
                let _ = write!(
                    out,
                    "{pad}Note {} caller={}{}",
                    self.marker(note.style),
                    note.caller,
                    span(note.span)
                );
                if let Some(category) = &note.category {
                    let _ = write!(out, " category={:?}{}", category.content, span(category.span));
                }
                out.push('\n');
                self.inlines(out, &note.children, depth + 1);
            }
            Inline::Milestone(milestone) => {
                let _ = write!(
                    out,
                    "{pad}Milestone {}{}",
                    self.marker(milestone.style),
                    span(milestone.span)
                );
                self.attributes(out, &milestone.attributes);
                out.push('\n');
            }
            Inline::OptBreak(opt_break) => {
                let _ = writeln!(out, "{pad}OptBreak{}", span(opt_break.span));
            }
        }
    }
}
