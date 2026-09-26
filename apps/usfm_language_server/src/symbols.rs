//! `textDocument/documentSymbol`: the book, its chapters and their verses as
//! an outline (ticket 32).
//!
//! What the editor does with it is the Outline view, the breadcrumb bar and
//! "go to symbol in file" (`Ctrl+Shift+O`), which is how a translator jumps to
//! `\c 12` in a book of forty chapters without scrolling.
//!
//! # Where the numbers come from
//!
//! One walk of the tree ([`found`]), not `usfm_semantic::ReferenceIndex`.
//! The index was the obvious source and is not the right one here: the
//! outline needs the block containers themselves — a sidebar, a `\periph` —
//! which the index does not model, so a walk is needed whatever the chapters
//! come from, and one walk that finds five kinds of node is less than an
//! index plus a walk. (When this was written the index also read chapters
//! from the top-level blocks only, and so lost every chapter of a book with
//! front matter; ticket 38 made it descend into `\periph` divisions.)
//!
//! What the index would have given is the verse-to-chapter mapping, and
//! document order gives the same answer: the chapter a verse is in is the
//! last `\c` before it. Nothing is deduplicated or reordered on the way —
//! a repeated or backwards verse number is a symbol like any other, because
//! the outline is of the file and not of the versification.
//!
//! # The shape of the tree
//!
//! Symbols are built flat, each with the source range it covers, and then
//! nested by containment ([`nest`]): a symbol becomes the child of the
//! innermost symbol whose range covers it whole. That is what puts verses
//! under their chapter and chapters under the book without four special
//! cases, and it puts a sidebar wherever it stands — inside the verse it
//! interrupts, or beside the chapters when it stands outside them all. The
//! protocol requires a child's range to be contained in its parent's, which
//! this rule gives by construction; a symbol that overlaps rather than nests
//! (a `\v` inside a sidebar, whose verse runs past the sidebar's end) becomes
//! a sibling instead of being forced into a parent it does not fit.
//!
//! Ranges:
//!
//! * `selection_range` is the marker itself — `\c 1`, `\v 1-2`, `\id GEN` —
//!   which is what the editor reveals and highlights when the symbol is
//!   picked;
//! * `range` is the content: a chapter runs to the next chapter's start, a
//!   verse to the next verse's start (or, being the last, to the end of the
//!   chapter), and the book to the end of the document. Each is cut short by
//!   the end of the block container it is written in — a sidebar, a
//!   `\periph`, the document — so a verse inside a sidebar ends with the
//!   sidebar and not at the next `\v` outside it.
//!
//! A node with an empty span is left out. That is `usfm::span::SPAN`, what a
//! synthesized node carries (a verse end the parser inserted, a whole tree
//! built by hand), and a symbol at offset 0 with no extent would send the
//! editor to the top of the file.

use tower_lsp_server::ls_types::DocumentSymbol;
use tower_lsp_server::ls_types::SymbolKind;

use usfm::ast::{Document, NodeRef};
use usfm::span::{LineIndex, Span};

use crate::convert;

/// The outline of `document`, as the protocol's nested symbols.
pub fn symbols(document: &Document<'_>, index: &LineIndex) -> Vec<DocumentSymbol> {
    nest(entries(document))
        .into_iter()
        .map(|entry| entry.into_symbol(index))
        .collect()
}

/// One outline entry before it is nested: the protocol's symbol with spans
/// where its ranges will be.
#[derive(Debug, PartialEq)]
struct Entry {
    name: String,
    detail: Option<String>,
    kind: SymbolKind,
    /// The content the symbol covers.
    range: Span,
    /// The marker that opens it.
    selection: Span,
    children: Vec<Entry>,
}

impl Entry {
    fn new(name: String, kind: SymbolKind, range: Span, selection: Span) -> Self {
        Self {
            name,
            detail: None,
            kind,
            range,
            selection,
            children: Vec::new(),
        }
    }

    fn with_detail(mut self, detail: Option<String>) -> Self {
        self.detail = detail.filter(|detail| !detail.is_empty());
        self
    }

    /// Whether `other` fits inside this entry, which is what makes it a child.
    fn covers(&self, other: &Entry) -> bool {
        self.range.start <= other.range.start && other.range.end <= self.range.end
    }

    fn into_symbol(self, index: &LineIndex) -> DocumentSymbol {
        let children: Vec<DocumentSymbol> = self
            .children
            .into_iter()
            .map(|child| child.into_symbol(index))
            .collect();
        #[allow(
            deprecated,
            reason = "`DocumentSymbol::deprecated` is a protocol field the struct still has; \
                      `tags` replaced it and nothing here is deprecated either way"
        )]
        DocumentSymbol {
            name: self.name,
            detail: self.detail,
            kind: self.kind,
            tags: None,
            deprecated: None,
            range: convert::range(index, self.range),
            selection_range: convert::range(index, self.selection),
            children: (!children.is_empty()).then_some(children),
        }
    }
}

/// One symbol as the walk finds it, before its end is known.
///
/// A book, a sidebar and a `\periph` carry their own extent, so `end` is set
/// at once; a chapter and a verse run until something else starts and are
/// finished off by [`entries`].
struct Found {
    entry: Entry,
    kind: Kind,
    /// The end of the block container the node is written in: the document,
    /// or the sidebar or `\periph` around it. Nothing a symbol covers reaches
    /// past it.
    container_end: u32,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Chapter,
    Verse,
    /// A book, a sidebar or a `\periph`: its range is its own span.
    Extent,
}

/// Every symbol the document has, flat, in document order with the widest
/// first where two start together.
fn entries(document: &Document<'_>) -> Vec<Entry> {
    let mut found = found(document);

    // A chapter runs to the next chapter, a verse to the next verse; both
    // stop at the end of the container they are written in, and a verse also
    // at the end of its chapter — the last `\c` before it.
    for kind in [Kind::Chapter, Kind::Verse] {
        let mut chapter_end = u32::MAX;
        let starts: Vec<u32> = found
            .iter()
            .filter(|found| found.kind == kind)
            .map(|found| found.entry.range.start)
            .collect();
        let mut seen = 0;
        for found in found.iter_mut() {
            if found.kind == Kind::Chapter && kind == Kind::Verse {
                chapter_end = found.entry.range.end;
            }
            if found.kind != kind {
                continue;
            }
            seen += 1;
            let next = starts.get(seen).copied().unwrap_or(u32::MAX);
            let end = next
                .min(found.container_end)
                .min(chapter_end)
                .max(found.entry.range.end);
            found.entry.range = Span::new(found.entry.range.start, end);
        }
    }

    let mut entries: Vec<Entry> = found.into_iter().map(|found| found.entry).collect();
    entries.retain(|entry| entry.range.start < entry.range.end);
    // Document order, and where two symbols start at the same offset the wider
    // one first, so that `nest` meets a parent before its children.
    entries.sort_by_key(|entry| (entry.range.start, std::cmp::Reverse(entry.range.end)));
    entries
}

/// The five kinds of node that become a symbol, in document order, each with
/// the block container it is written in.
///
/// The container stack works because [`NodeRef::descendants`] is pre-order
/// and hands over the path: an entry pushed at depth *d* covers every node
/// deeper than *d*, and is popped as soon as one at *d* or above arrives.
fn found(document: &Document<'_>) -> Vec<Found> {
    let mut found = Vec::new();
    // `(depth, end)`. The document is the outermost container and is never
    // popped, every path being at least one long.
    let mut containers = vec![(0usize, document.span.end)];
    let mut path = Vec::new();
    let mut book_seen = false;

    NodeRef::Document(document).descendants(&mut path, &mut |path, node| {
        while containers
            .last()
            .is_some_and(|(depth, _)| *depth >= path.len())
        {
            containers.pop();
        }
        let container_end = containers.last().map_or(document.span.end, |(_, end)| *end);
        let mut push = |entry: Entry, kind: Kind| {
            found.push(Found {
                entry,
                kind,
                container_end,
            })
        };

        match node {
            // Only the first `\id`, and only at the top: a second one starts
            // a second book, which `id-not-first` is already about, and one
            // nested in a sidebar is not the file's book at all.
            NodeRef::Book(book) if path.len() == 1 && !std::mem::replace(&mut book_seen, true) => {
                push(
                    Entry::new(
                        book.code.as_str().to_owned(),
                        SymbolKind::MODULE,
                        Span::new(book.span.start, document.span.end.max(book.span.end)),
                        book.span,
                    )
                    .with_detail(Some(book.description.trim().to_owned())),
                    Kind::Extent,
                )
            }
            NodeRef::ChapterStart(chapter) => push(
                Entry::new(
                    format!("Chapter {}", chapter.number),
                    SymbolKind::NAMESPACE,
                    chapter.span,
                    chapter.span,
                ),
                Kind::Chapter,
            ),
            NodeRef::VerseStart(verse) => push(
                Entry::new(
                    verse.number.to_string(),
                    SymbolKind::KEY,
                    verse.span,
                    verse.span,
                ),
                Kind::Verse,
            ),
            NodeRef::Sidebar(sidebar) => {
                push(
                    Entry::new(
                        "Sidebar".to_owned(),
                        SymbolKind::OBJECT,
                        sidebar.span,
                        marker_span(document.marker(sidebar.style), sidebar.span),
                    )
                    .with_detail(
                        sidebar
                            .category
                            .as_ref()
                            .map(|category| category.content.trim().to_owned()),
                    ),
                    Kind::Extent,
                );
                containers.push((path.len(), sidebar.span.end));
            }
            NodeRef::Periph(periph) => {
                let title = periph
                    .title
                    .as_ref()
                    .map(|title| title.content.trim())
                    .unwrap_or_default();
                let name = match title {
                    "" => "Periph".to_owned(),
                    title => format!("Periph {title}"),
                };
                push(
                    Entry::new(
                        name,
                        SymbolKind::OBJECT,
                        periph.span,
                        marker_span(document.marker(periph.style), periph.span),
                    ),
                    Kind::Extent,
                );
                containers.push((path.len(), periph.span.end));
            }
            _ => {}
        }
    });
    found
}

/// Where a block container's own marker ends: `\esb` is four bytes of the
/// span that runs to `\esbe`.
///
/// The marker is the one the node was built from, so its name and the source
/// agree; the span is clamped to the node's in case the node is synthesized
/// and has no room for it.
fn marker_span(marker: &str, span: Span) -> Span {
    let end = (span.start + 1 + marker.len() as u32).min(span.end);
    Span::new(span.start, end)
}

/// Nest entries by containment: each becomes a child of the innermost entry
/// that covers it, and the rest stay roots.
///
/// `entries` must be in document order, widest first — which is what
/// [`entries`] sorts it into — so a parent is always seen before its children
/// and the stack holds the chain of open ancestors.
fn nest(entries: Vec<Entry>) -> Vec<Entry> {
    let mut roots: Vec<Entry> = Vec::new();
    // The chain of open ancestors, as a path of child indices into `roots`.
    let mut open: Vec<usize> = Vec::new();

    for entry in entries {
        while !open.is_empty() && !at(&roots, &open).covers(&entry) {
            open.pop();
        }
        let siblings = match open.is_empty() {
            true => &mut roots,
            false => &mut at_mut(&mut roots, &open).children,
        };
        siblings.push(entry);
        open.push(siblings.len() - 1);
    }
    roots
}

/// The entry at a path of child indices. `path` is never empty.
fn at<'a>(roots: &'a [Entry], path: &[usize]) -> &'a Entry {
    let (first, rest) = path.split_first().expect("the open path is not empty");
    let mut entry = &roots[*first];
    for index in rest {
        entry = &entry.children[*index];
    }
    entry
}

fn at_mut<'a>(roots: &'a mut [Entry], path: &[usize]) -> &'a mut Entry {
    let (first, rest) = path.split_first().expect("the open path is not empty");
    let mut entry = &mut roots[*first];
    for index in rest {
        entry = &mut entry.children[*index];
    }
    entry
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str =
        "\\id GEN Genesis\n\\c 1\n\\p\n\\v 1 In the beginning\n\\v 2-3 and\n\\c 2\n\\p\n\\v 1 y\n";

    fn outline(source: &str) -> Vec<DocumentSymbol> {
        let result = usfm::parse(source);
        symbols(&result.document, &LineIndex::new(source))
    }

    fn names(symbols: &[DocumentSymbol]) -> Vec<&str> {
        symbols.iter().map(|symbol| symbol.name.as_str()).collect()
    }

    fn children(symbol: &DocumentSymbol) -> &[DocumentSymbol] {
        symbol.children.as_deref().unwrap_or_default()
    }

    /// The shape: the book at the top, chapters under it, verses under those.
    #[test]
    fn the_book_holds_the_chapters_and_a_chapter_holds_its_verses() {
        let outline = outline(SOURCE);
        assert_eq!(names(&outline), ["GEN"]);
        let book = &outline[0];
        assert_eq!(book.kind, SymbolKind::MODULE);
        assert_eq!(book.detail.as_deref(), Some("Genesis"));

        let chapters = children(book);
        assert_eq!(names(chapters), ["Chapter 1", "Chapter 2"]);
        assert_eq!(chapters[0].kind, SymbolKind::NAMESPACE);
        // A verse range names itself as it is written.
        assert_eq!(names(children(&chapters[0])), ["1", "2-3"]);
        assert_eq!(names(children(&chapters[1])), ["1"]);
        assert_eq!(children(&chapters[0])[0].kind, SymbolKind::KEY);
    }

    /// `selection_range` is the marker, `range` the content, and a child's
    /// range is inside its parent's — which the protocol requires.
    #[test]
    fn a_verse_selects_its_marker_and_covers_its_text() {
        let result = usfm::parse(SOURCE);
        let index = LineIndex::new(SOURCE);
        let outline = symbols(&result.document, &index);
        let chapter = &children(&outline[0])[0];
        let verse = &children(chapter)[0];

        // `\v 1` is on line 3 (counting from 0), and the marker is four
        // characters.
        assert_eq!(
            verse.selection_range,
            convert::range(&index, {
                let start = SOURCE.find("\\v 1").unwrap() as u32;
                Span::new(start, start + 4)
            })
        );
        // The content runs to the start of `\v 2-3`.
        assert_eq!(
            verse.range.end,
            convert::position(&index, SOURCE.find("\\v 2-3").unwrap() as u32),
        );
        // Containment, at every level.
        assert!(inside(chapter.range, outline[0].range));
        assert!(inside(verse.range, chapter.range));
        assert!(inside(verse.selection_range, verse.range));

        // The last verse of a chapter stops at the end of the chapter, not at
        // the next chapter's first verse.
        let last = children(chapter).last().expect("chapter 1 has verses");
        assert_eq!(
            last.range.end,
            convert::position(&index, SOURCE.find("\\c 2").unwrap() as u32),
        );
    }

    fn inside(
        inner: tower_lsp_server::ls_types::Range,
        outer: tower_lsp_server::ls_types::Range,
    ) -> bool {
        outer.start <= inner.start && inner.end <= outer.end
    }

    /// Without an `\id` the chapters are the roots: there is nothing to hang
    /// them on and an outline of one made-up entry would be a lie.
    #[test]
    fn a_document_without_an_id_has_the_chapters_at_the_top() {
        let outline = outline("\\c 1\n\\p\n\\v 1 a\n\\c 2\n\\p\n\\v 1 b\n");
        assert_eq!(names(&outline), ["Chapter 1", "Chapter 2"]);
        assert_eq!(names(children(&outline[0])), ["1"]);
    }

    /// A sidebar stands where it is written: inside the verse it interrupts,
    /// named for what it is and detailed with its `\cat` category.
    #[test]
    fn a_sidebar_is_a_symbol_where_it_stands() {
        let source = "\\id GEN Genesis\n\\c 1\n\\p\n\\v 1 a\n\\esb \\cat History\\cat*\n\\p aside\n\\esbe\n\\p more\n";
        let outline = outline(source);
        let chapter = &children(&outline[0])[0];
        let verse = &children(chapter)[0];
        assert_eq!(verse.name, "1");

        let sidebar = children(verse)
            .iter()
            .find(|symbol| symbol.name == "Sidebar")
            .expect("the sidebar is a symbol");
        assert_eq!(sidebar.kind, SymbolKind::OBJECT);
        assert_eq!(sidebar.detail.as_deref(), Some("History"));
        // Its selection range is the `\esb` marker alone, four characters on
        // the fifth line.
        assert_eq!(sidebar.selection_range.start.line, 4);
        assert_eq!(sidebar.selection_range.start.character, 0);
        assert_eq!(sidebar.selection_range.end.character, 4);
    }

    /// A `\periph` division is named for its title — and it holds whatever it
    /// holds. A division runs to the next `\periph` or `\id` (ticket 27), so
    /// a chapter written after one is *inside* it, and the outline says so
    /// rather than tidying the tree into the shape a reader might expect.
    #[test]
    fn a_periph_is_named_for_its_title_and_holds_what_it_holds() {
        let source =
            "\\id GEN Genesis\n\\periph Title Page|id=\"title\"\n\\p intro\n\\c 1\n\\p\n\\v 1 a\n";
        let outline = outline(source);
        assert_eq!(names(&outline), ["GEN"]);
        let periph = &children(&outline[0])[0];
        assert_eq!(periph.name, "Periph Title Page");
        assert_eq!(periph.kind, SymbolKind::OBJECT);
        assert_eq!(names(children(periph)), ["Chapter 1"]);
        // `\periph` is seven characters.
        assert_eq!(periph.selection_range.start.line, 1);
        assert_eq!(periph.selection_range.end.character, 7);
    }

    /// Repeats and out-of-order numbers are shown as they are written: the
    /// outline is of the file, not of the versification.
    #[test]
    fn a_repeated_verse_number_is_a_symbol_of_its_own() {
        let outline = outline("\\id GEN\n\\c 1\n\\p\n\\v 1 a\n\\v 1 b\n\\v 3 c\n");
        let chapter = &children(&outline[0])[0];
        assert_eq!(names(children(chapter)), ["1", "1", "3"]);
    }

    /// An empty file has nothing to point at: no symbols, rather than one of
    /// empty range at offset 0.
    #[test]
    fn an_empty_document_has_no_symbols() {
        assert!(outline("").is_empty());
    }
}
