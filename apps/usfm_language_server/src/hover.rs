//! `textDocument/hover`: what the stylesheet and the tree say about the
//! marker under the cursor (ticket 31).
//!
//! Two kinds of answer, decided by the node [`locate`](crate::locate::locate)
//! found:
//!
//! * a node that carries a style — a paragraph, a character style, a note, a
//!   milestone, a sidebar, a `\periph`, a table cell — is answered with its
//!   stylesheet entry: the marker, its `\Name`, its `\Description` and its
//!   `\OccursUnder` list on one line. The entry comes from the *document's*
//!   own sheet (hardening plan D3), so a project `.sty` and a style the parser
//!   derived both describe themselves;
//! * a `\c` or a `\v` — and any text inside one — is answered with the
//!   reference, `GEN 1:1`.
//!
//! **The whole node's span, not only its marker.** Hovering anywhere in a
//! paragraph's own span answers for the paragraph, because the innermost node
//! at that offset already *is* the smallest thing there: inside `\p` the
//! text is a `Text` node, inside `\w …\w*` the `\w`, so the answer narrows
//! with the cursor without the marker's own extent ever being computed. What
//! this buys over a marker-only rule is the empty places — the space between
//! `\p` and its first word, the `|` of an attribute list — where a marker-only
//! rule would answer nothing.
//!
//! A `Text` run is where the two kinds meet, and the reference wins: text is
//! what a document is mostly made of, and "which verse am I in" is what a
//! translator hovers it for. With no chapter to name (an introduction, a file
//! with no `\c`) it falls back to the style that holds the run, so hovering
//! the words of an `\ip` still says what `\ip` is.

use usfm::ast::{Document, NodeRef, TableCell};
use usfm::span::Span;
use usfm::style::StyleRule;

use crate::locate::{Located, Location, locate, span_of};

/// One hover answer: the markdown to show and the range it is about.
#[derive(Debug, PartialEq)]
pub struct Hover {
    pub markdown: String,
    pub span: Span,
}

/// What to show for the cursor at `offset`, or `None` when there is nothing
/// under it.
pub fn hover(document: &Document<'_>, offset: u32) -> Option<Hover> {
    let location = locate(document, offset);
    let found = location.node.as_ref()?;
    let markdown = match found.node {
        // The reference, for the markers that carry one and for the text
        // between them.
        NodeRef::ChapterStart(_)
        | NodeRef::ChapterEnd(_)
        | NodeRef::VerseStart(_)
        | NodeRef::VerseEnd(_) => reference(&location)?,
        NodeRef::Text(_) | NodeRef::OptBreak(_) => match reference(&location) {
            Some(markdown) => markdown,
            // No chapter to name: answer for the style the run is in, so
            // hovering an introduction paragraph still says what its marker
            // is.
            None => enclosing_style(document, found)?,
        },
        NodeRef::Book(book) => {
            let code = book.code.as_str();
            match book.description.trim() {
                "" => format!("**{code}**"),
                description => format!("**{code}** — {description}"),
            }
        }
        node => style(document, node)?,
    };
    Some(Hover {
        markdown,
        span: span_of(found.node),
    })
}

/// The reference in force at the cursor, as a markdown line.
fn reference(location: &Location<'_>) -> Option<String> {
    location
        .reference()
        .map(|reference| format!("**{reference}**"))
}

/// The stylesheet entry of a node that carries a style, as markdown.
fn style(document: &Document<'_>, node: NodeRef<'_>) -> Option<String> {
    let rule = match node {
        NodeRef::Para(para) => Some(document.style(para.style)),
        NodeRef::Char(char) => Some(document.style(char.style)),
        NodeRef::Note(note) => Some(document.style(note.style)),
        NodeRef::Milestone(milestone) => Some(document.style(milestone.style)),
        NodeRef::Sidebar(sidebar) => Some(document.style(sidebar.style)),
        NodeRef::Periph(periph) => Some(document.style(periph.style)),
        // A cell carries no `StyleId`: its marker is spelled out of what it
        // is, exactly as `usfm_codegen` writes one back (`\tc1`, `\thr2`).
        NodeRef::TableCell(cell) => document
            .style_sheet()
            .get_rule_by_marker(&cell_marker(cell)),
        // A table or a row is not a marker of its own: `\tr` opens the row and
        // the cells follow it on the same line, and the cell is the innermost
        // node wherever one of them is.
        _ => None,
    }?;
    Some(describe(rule))
}

/// The entry of the nearest ancestor that carries a style.
fn enclosing_style(document: &Document<'_>, found: &Located<'_>) -> Option<String> {
    let mut path = found.path.as_slice();
    while let Some(parent) = path.split_last().map(|(_, rest)| rest) {
        let node = NodeRef::Document(document).descend(parent)?;
        if let Some(markdown) = style(document, node) {
            return Some(markdown);
        }
        path = parent;
    }
    None
}

/// A table cell's marker: `\tc1`, `\thr2`, and `\tc1-2` for a colspan.
fn cell_marker(cell: &TableCell<'_>) -> String {
    let kind = if cell.header { "h" } else { "c" };
    let alignment = match cell.alignment {
        usfm::ast::Alignment::Start => "",
        usfm::ast::Alignment::Center => "c",
        usfm::ast::Alignment::End => "r",
    };
    format!("t{kind}{alignment}{}", cell.column)
}

/// One stylesheet entry as markdown.
///
/// The marker first, in code, so that the line reads as the thing that is in
/// the file; then the sheet's own two lines, each left out when the sheet does
/// not say (a derived style, a marker from a bare project `.sty`); then the
/// `\OccursUnder` list, on one line, which is the question a marker in the
/// wrong place raises. An unrestricted marker says so rather than printing an
/// empty list.
fn describe(rule: &StyleRule) -> String {
    let mut markdown = format!("`\\{}`", rule.marker);
    if let Some(name) = &rule.name {
        markdown.push_str(" — ");
        markdown.push_str(name);
    }
    if let Some(description) = &rule.description {
        markdown.push_str("\n\n");
        markdown.push_str(description);
    }
    markdown.push_str("\n\nOccurs under: ");
    if rule.occurs_under.is_empty() {
        markdown.push_str("anywhere");
    } else {
        markdown.push_str(
            &rule
                .occurs_under
                .iter()
                .map(|marker| format!("`\\{marker}`"))
                .collect::<Vec<_>>()
                .join(" "),
        );
    }
    markdown
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str =
        "\\id GEN Genesis\n\\c 1\n\\p\n\\v 1 In \\w beginning|lemma=\"begin\"\\w* God\n";

    fn at(needle: &str) -> u32 {
        SOURCE.find(needle).expect("the needle is in the source") as u32
    }

    fn markdown(source: &str, offset: u32) -> String {
        let result = usfm::parse(source);
        hover(&result.document, offset)
            .unwrap_or_else(|| panic!("a hover at {offset} in {source:?}"))
            .markdown
    }

    /// A paragraph marker: the sheet's name, its description and where it may
    /// occur.
    #[test]
    fn a_paragraph_marker_shows_its_stylesheet_entry() {
        let markdown = markdown(SOURCE, at("\\p"));
        assert!(markdown.starts_with("`\\p` — p - Paragraph"), "{markdown}");
        assert!(
            markdown.contains("Paragraph text, with first line indent"),
            "{markdown}",
        );
        // The `\OccursUnder` list, written as markers: `\p` occurs under
        // `\c`, a sidebar and the section headings.
        assert!(
            markdown.contains("Occurs under: `\\c` `\\esb`"),
            "{markdown}"
        );
    }

    /// A character marker, and the range it is about: the whole `\w …\w*`.
    #[test]
    fn a_character_marker_shows_its_entry_and_its_whole_span() {
        let result = usfm::parse(SOURCE);
        let hover = hover(&result.document, at("\\w beginning")).expect("a hover on `\\w`");
        assert!(hover.markdown.starts_with("`\\w` — "), "{hover:#?}");
        assert!(hover.markdown.contains("Occurs under:"), "{hover:#?}");
        // From the marker to the end of `\w*`.
        let start = at("\\w beginning");
        let end = at("\\w*") + "\\w*".len() as u32;
        assert_eq!(hover.span, Span::new(start, end));
    }

    /// A `\v`, and the text after it, are the reference.
    #[test]
    fn a_verse_shows_the_reference() {
        assert_eq!(markdown(SOURCE, at("\\v 1")), "**GEN 1:1**");
        assert_eq!(markdown(SOURCE, at("In ")), "**GEN 1:1**");
        // The chapter marker, before any verse of it.
        assert_eq!(markdown(SOURCE, at("\\c 1")), "**GEN 1**");
    }

    /// Text inside a character style is still the reference: it is the
    /// commonest place a cursor sits, and the verse is what it wants.
    #[test]
    fn text_inside_a_character_style_is_the_reference() {
        assert_eq!(markdown(SOURCE, at("beginning")), "**GEN 1:1**");
    }

    /// With no chapter to name, text answers for the style around it.
    #[test]
    fn text_outside_a_chapter_falls_back_to_its_style() {
        let source = "\\id GEN\n\\ip An introduction\n";
        let markdown = markdown(source, source.find("introduction").unwrap() as u32);
        assert!(markdown.starts_with("`\\ip` — "), "{markdown}");
    }

    /// A note and a milestone carry styles like any other node.
    #[test]
    fn a_note_and_a_milestone_show_their_entries() {
        let source = "\\id GEN\n\\c 1\n\\p \\v 1 a\\f + \\ft note\\f* b \\qt-s |who=\"God\"\\*c\n";
        let note = markdown(source, source.find("\\f +").unwrap() as u32);
        assert!(note.starts_with("`\\f` — "), "{note}");
        let milestone = markdown(source, source.find("\\qt-s").unwrap() as u32);
        assert!(milestone.starts_with("`\\qt-s`"), "{milestone}");
    }

    /// A table cell has no `StyleId`; its marker is spelled from the cell.
    #[test]
    fn a_table_cell_shows_the_entry_of_the_marker_it_is_written_with() {
        let source = "\\id GEN\n\\c 1\n\\tr \\th1 Name \\thr2 Number\n";
        let header = markdown(source, source.find("\\th1").unwrap() as u32);
        assert!(header.starts_with("`\\th1`"), "{header}");
        let right = markdown(source, source.find("\\thr2").unwrap() as u32);
        assert!(right.starts_with("`\\thr2`"), "{right}");
    }

    /// A style the sheet says nothing about prints what there is and no empty
    /// lines for the rest.
    #[test]
    fn an_undocumented_style_prints_only_what_the_sheet_has() {
        // `\zaln-s` is not in the default sheet: the parser derives a rule for
        // it, with no name, no description and no `OccursUnder`.
        let source = "\\id GEN\n\\c 1\n\\p \\zaln-s |x-strong=\"H1\"\\*a\\zaln-e\\*\n";
        let markdown = markdown(source, source.find("\\zaln-s").unwrap() as u32);
        assert_eq!(markdown, "`\\zaln-s`\n\nOccurs under: anywhere");
    }

    /// Nothing under the cursor is no hover, not an empty one.
    #[test]
    fn nothing_under_the_cursor_is_no_hover() {
        let result = usfm::parse(SOURCE);
        assert_eq!(hover(&result.document, 10_000), None);
    }
}
