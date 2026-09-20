//! Which node is under the cursor, and which verse the cursor is in.
//!
//! Every request that arrives with a position answers about one node: hover
//! (ticket 31) about the marker under the cursor, and the symbols, completions
//! and code actions of ticket 32 about the node they are asked in. They all
//! start here.
//!
//! One walk answers both questions. [`locate`] descends the tree in document
//! order keeping the innermost node whose span holds the offset, and on the
//! way records the `\id`, `\c` and `\v` it passed, which is the reference the
//! cursor is inside. The alternative for the reference was
//! `usfm_semantic::ReferenceIndex`, which is a second walk of the whole tree
//! plus path arithmetic to decide whether an offset is inside a verse — the
//! index is keyed by [`NodePath`], not by position — for an answer this walk
//! already has in hand.
//!
//! Spans, not paths, decide what is under the cursor, and a synthesized node
//! carries [`SPAN`](usfm::span::SPAN), the empty span at 0: an empty span
//! holds no offset, so a verse end the parser inserted is never what a hover
//! lands on, and a hand-built tree locates nothing at all rather than
//! everything at once.

use usfm::ast::{BookCode, Document, NodePath, NodeRef, NumberList};
use usfm::span::Span;

/// A node with its path from the document root.
///
/// The path is what [`NodeRef::descend`] takes, so it is also the way back to
/// the node's ancestors: `&path[..path.len() - 1]` is its parent. Hover uses
/// that to fall back from a text run to the style that holds it.
#[derive(Debug, Clone)]
pub struct Located<'a> {
    pub node: NodeRef<'a>,
    pub path: NodePath,
}

/// Where in the document a byte offset falls: the innermost node, and the
/// reference (book, chapter, verse) in force there.
#[derive(Debug, Clone)]
pub struct Location<'a> {
    /// The innermost node whose span holds the offset, with its path. `None`
    /// when the offset is in no node at all — in the whitespace between two
    /// blocks, or past the end of the text the tree was parsed from.
    pub node: Option<Located<'a>>,
    /// The book of the `\id` line, if the document has one.
    pub book: Option<BookCode>,
    /// The chapter of the last `\c` at or before the offset.
    pub chapter: Option<usize>,
    /// The verse of the last `\v` at or before the offset, when that `\v`
    /// comes after that `\c`. A `\v` from the chapter before is not the verse
    /// the cursor is in.
    pub verse: Option<NumberList>,
}

impl<'a> Location<'a> {
    /// The reference as USFM writes it in a cross-reference: `GEN 1:1`,
    /// `GEN 1` in a chapter with no verse yet, `1:1` in a document with no
    /// `\id`. `None` when there is not even a chapter to name.
    pub fn reference(&self) -> Option<String> {
        let chapter = self.chapter?;
        let book = self.book.map(|code| format!("{} ", code.as_str()));
        let verse = self
            .verse
            .as_ref()
            .map(|number| format!(":{number}"))
            .unwrap_or_default();
        Some(format!("{}{chapter}{verse}", book.unwrap_or_default()))
    }
}

/// Everything the position-taking requests need about `offset`.
///
/// A caller that wants only the innermost node — ticket 32's symbols, code
/// actions and completion context — takes [`Location::node`] and ignores the
/// rest; nothing here is computed twice.
pub fn locate<'a>(document: &'a Document<'a>, offset: u32) -> Location<'a> {
    let mut location = Location {
        node: None,
        book: None,
        chapter: None,
        verse: None,
    };
    // Where the chapter in force started, so that a `\v` before it — the last
    // verse of the chapter before — is not taken for the verse we are in.
    let mut chapter_at = 0;

    let mut path = Vec::new();
    NodeRef::Document(document).descendants(&mut path, &mut |path, node| {
        let span = span_of(node);
        if contains(span, offset) && is_inner(&location, span) {
            location.node = Some(Located {
                node,
                path: path.to_vec(),
            });
        }
        match node {
            NodeRef::Book(book) if location.book.is_none() => location.book = Some(book.code),
            NodeRef::ChapterStart(chapter) if span.start <= offset => {
                location.chapter = Some(chapter.number);
                location.verse = None;
                chapter_at = span.start;
            }
            NodeRef::VerseStart(verse) if span.start <= offset && span.start >= chapter_at => {
                location.verse = Some(verse.number.clone());
            }
            _ => {}
        }
    });
    location
}

/// A node's source range. Every node has one; this is the one place that has
/// to say so for all seventeen kinds.
pub fn span_of(node: NodeRef<'_>) -> Span {
    match node {
        NodeRef::Document(document) => document.span,
        NodeRef::Book(book) => book.span,
        NodeRef::ChapterStart(chapter) => chapter.span,
        NodeRef::ChapterEnd(chapter) => chapter.span,
        NodeRef::Para(para) => para.span,
        NodeRef::Table(table) => table.span,
        NodeRef::TableRow(row) => row.span,
        NodeRef::TableCell(cell) => cell.span,
        NodeRef::Text(text) => text.span,
        NodeRef::VerseStart(verse) => verse.span,
        NodeRef::VerseEnd(verse) => verse.span,
        NodeRef::Char(char) => char.span,
        NodeRef::Note(note) => note.span,
        NodeRef::Milestone(milestone) => milestone.span,
        NodeRef::OptBreak(brk) => brk.span,
        NodeRef::Sidebar(sidebar) => sidebar.span,
        NodeRef::Periph(periph) => periph.span,
    }
}

/// Whether `span` holds `offset`, counting the start and not the end.
///
/// The end is where the *next* node starts, and a cursor there belongs to that
/// one. An empty span (a synthesized node) holds nothing by this rule, which
/// is what keeps such a node from being located.
fn contains(span: Span, offset: u32) -> bool {
    span.start <= offset && offset < span.end
}

/// Whether a node with `span` is further in than what has been found so far.
///
/// Document order alone would do while every child's span sits inside its
/// parent's, but the parser is allowed to repair, and a node whose span the
/// parent's does not cover must not win on document order alone. Smaller wins,
/// and an equal span goes to the later node, which is the deeper one.
fn is_inner(location: &Location<'_>, span: Span) -> bool {
    match &location.node {
        None => true,
        Some(found) => {
            let best = span_of(found.node);
            span.start >= best.start && span.end <= best.end
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = "\\id GEN Genesis\n\\c 1\n\\p\n\\v 1 In \\nd the\\nd* beginning\n\\v 2 x\n\\c 2\n\\p\n\\v 1 y\n";

    fn at(source: &str, needle: &str) -> u32 {
        source.find(needle).expect("the needle is in the source") as u32
    }

    /// The innermost node alone — the half of [`locate`] these tests are
    /// about, and the one the rest of M6 will reach for.
    fn node_at<'a>(document: &'a Document<'a>, offset: u32) -> Option<Located<'a>> {
        locate(document, offset).node
    }

    #[test]
    fn the_innermost_node_is_the_one_found() {
        let result = usfm::parse(SOURCE);
        let document = &result.document;

        // On the `\id` line: the book.
        let book = node_at(document, 2).expect("a node on the `\\id` line");
        assert!(book.node.is_book());

        // On a paragraph marker: the paragraph, because its text starts after
        // it.
        let para = node_at(document, at(SOURCE, "\\p")).expect("a node on `\\p`");
        assert!(para.node.is_para());

        // On the `\v`: the verse start.
        let verse = node_at(document, at(SOURCE, "\\v 1 In")).expect("a node on `\\v`");
        assert!(verse.node.is_verse_start());

        // In the text after it: the text run, whose parent is the paragraph.
        let text = node_at(document, at(SOURCE, "In ")).expect("a node in the text");
        assert!(text.node.is_text());
        assert!(
            NodeRef::Document(document)
                .descend(&text.path[..text.path.len() - 1])
                .expect("the parent of a located node")
                .is_para()
        );

        // On the character marker: the character style, not the paragraph
        // around it and not the text inside it.
        let char = node_at(document, at(SOURCE, "\\nd the")).expect("a node on `\\nd`");
        assert!(char.node.is_char());
        // And inside it: the text, one level deeper.
        let inside = node_at(document, at(SOURCE, "the\\nd*")).expect("a node inside `\\nd`");
        assert!(inside.node.is_text());
        assert!(inside.path.len() > char.path.len());
    }

    #[test]
    fn an_offset_in_no_node_finds_none() {
        let result = usfm::parse(SOURCE);
        // Past the end of the source: nothing covers it.
        assert!(node_at(&result.document, 10_000).is_none());
        // And a tree with no source behind it locates nothing anywhere.
        let synthesized = usfm::parse("").document;
        assert!(node_at(&synthesized, 0).is_none());
    }

    #[test]
    fn the_reference_is_the_chapter_and_verse_in_force() {
        let result = usfm::parse(SOURCE);
        let document = &result.document;

        // On the `\id` line: no chapter yet, so no reference.
        assert_eq!(locate(document, 2).reference(), None);

        // On `\c 1`, before any verse.
        let chapter = locate(document, at(SOURCE, "\\c 1"));
        assert_eq!(
            chapter.book.map(|code| code.as_str().to_owned()).as_deref(),
            Some("GEN")
        );
        assert_eq!(chapter.reference().as_deref(), Some("GEN 1"));

        // In the text of the first verse, and on its `\v`.
        assert_eq!(
            locate(document, at(SOURCE, "beginning"))
                .reference()
                .as_deref(),
            Some("GEN 1:1"),
        );
        assert_eq!(
            locate(document, at(SOURCE, "\\v 2 x"))
                .reference()
                .as_deref(),
            Some("GEN 1:2"),
        );

        // The next chapter starts over: `\c 2` is not still in verse 2, and
        // the `\v 1` after it is the new chapter's.
        assert_eq!(
            locate(document, at(SOURCE, "\\c 2")).reference().as_deref(),
            Some("GEN 2"),
        );
        assert_eq!(
            locate(document, at(SOURCE, "\\v 1 y"))
                .reference()
                .as_deref(),
            Some("GEN 2:1"),
        );
    }

    #[test]
    fn a_document_without_an_id_line_still_has_a_reference() {
        let source = "\\c 3\n\\p\n\\v 4-5 text\n";
        let result = usfm::parse(source);
        let location = locate(&result.document, at(source, "text"));
        assert_eq!(location.book, None);
        // A verse range is named as it is written.
        assert_eq!(location.reference().as_deref(), Some("3:4-5"));
    }
}
