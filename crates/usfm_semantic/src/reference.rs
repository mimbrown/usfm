//! Chapter and verse extents, derived from the milestones (plan D4).
//!
//! Chapters and verses stay flat in the tree because verses cross paragraph
//! boundaries; a pipeline that wants "the content of Genesis 1:2" asks a
//! [`ReferenceIndex`] rather than walking for milestones itself. The index
//! borrows the document, so it can never be stale: mutate the tree, build
//! it again.
//!
//! Positions are [`NodePath`]s from the document root. A verse's content is
//! everything in document order between its start and end milestones;
//! [`VerseRef::nodes`] yields it as the largest nodes that fit, so a
//! paragraph wholly inside the verse comes as one node and a paragraph the
//! verse starts or ends in comes as its inlines.
//!
//! The index is derived from the tree, not part of it, so it lives in this
//! crate rather than in `usfm_ast` (ADR 0001; ticket 22). The types it walks
//! with — [`NodePath`], [`NodeRef`] — are tree types and stayed there.

use std::ops::Range;

use usfm_ast::text::PlainText;
use usfm_ast::{
    Block, BookCode, ChapterStart, Document, NodePath, NodeRef, NumberList, VerseStart,
};

/// The chapters and verses of a document, in document order.
///
/// Built with [`ReferenceIndex::new`]; the index borrows the document, so it
/// can never be stale.
pub struct ReferenceIndex<'a> {
    document: &'a Document<'a>,
    book: Option<BookCode>,
    chapters: Vec<ChapterEntry>,
    verses: Vec<VerseEntry>,
}

struct ChapterEntry {
    number: usize,
    /// Index of the `Block::Periph` the chapter is written in, or `None`
    /// for a chapter among the document's own blocks.
    periph: Option<usize>,
    /// Index of the `ChapterStart` in its container's blocks.
    start: usize,
    /// Index of the `ChapterEnd`, or of the next chapter, or the container's
    /// block count: the chapter's blocks are `start + 1 .. end`.
    end: usize,
    /// Indices into `verses`.
    verses: Range<usize>,
}

struct VerseEntry {
    /// Index into `chapters`, or `None` before the first chapter.
    chapter: Option<usize>,
    number: NumberList,
    start: NodePath,
    /// Path of the `VerseEnd`, when the document has one.
    end: Option<NodePath>,
}

impl<'a> ReferenceIndex<'a> {
    /// Index `document`. Chapters are read from the top-level blocks and
    /// from the blocks of each `\periph` division, because a division runs
    /// to the next `\periph` or `\id` and so holds every `\c` written after
    /// it — and `usx.rnc`'s `PeripheralContent` lists `Chapter`, so those are
    /// the book's chapters, not something beside it (ticket 38). A sidebar
    /// is not descended into: `usx.rnc` allows no chapter there. Verse starts
    /// are found anywhere, including inside sidebars and periphs, where the
    /// parser never places an end.
    ///
    /// Nothing is deduplicated or reordered: the tree is reported as it
    /// stands, which is what lets a check over the index say that a verse
    /// number repeats or runs backwards.
    pub fn new(document: &'a Document<'a>) -> Self {
        let mut index = Self {
            document,
            book: None,
            chapters: Vec::new(),
            verses: Vec::new(),
        };
        index.read_chapters(&document.blocks, None);

        let mut path = Vec::new();
        NodeRef::Document(document).descendants(&mut path, &mut |path, node| match node {
            NodeRef::VerseStart(verse) => {
                let chapter = index.chapter_holding(path);
                index.verses.push(VerseEntry {
                    chapter,
                    number: verse.number.clone(),
                    start: path.to_vec(),
                    end: None,
                });
            }
            NodeRef::VerseEnd(end) => {
                if let Some(entry) = index
                    .verses
                    .iter_mut()
                    .rev()
                    .find(|entry| entry.end.is_none() && entry.number == end.number)
                {
                    entry.end = Some(path.to_vec());
                }
            }
            _ => {}
        });

        // Verses and chapters are both in document order, and a chapter's
        // blocks are one run of one container, so each chapter's verses are
        // one contiguous run.
        for (position, verse) in index.verses.iter().enumerate() {
            if let Some(chapter) = verse.chapter {
                let range = &mut index.chapters[chapter].verses;
                // Called as a path, because on a `&mut Range` the method call
                // would be ambiguous with unstable `ExactSizeIterator::is_empty`.
                if Range::is_empty(range) {
                    *range = position..position + 1;
                } else {
                    range.end = position + 1;
                }
            }
        }
        index
    }

    /// Record the chapters among `blocks`, the document's own
    /// (`periph == None`) or a `\periph` division's, descending into each
    /// division as it is met so that chapters stay in document order.
    fn read_chapters(&mut self, blocks: &'a [Block<'a>], periph: Option<usize>) {
        let first = self.chapters.len();
        for (position, block) in blocks.iter().enumerate() {
            match block {
                Block::Book(book) => {
                    self.book.get_or_insert(book.code);
                }
                Block::ChapterStart(chapter) => {
                    self.close_open_chapter(first, position);
                    self.chapters.push(ChapterEntry {
                        number: chapter.number,
                        periph,
                        start: position,
                        end: usize::MAX,
                        verses: 0..0,
                    });
                }
                Block::ChapterEnd(chapter) => {
                    if let Some(last) = self.chapters[first..].last_mut()
                        && last.end == usize::MAX
                        && last.number == chapter.number
                    {
                        last.end = position;
                    }
                }
                Block::Periph(division) if periph.is_none() => {
                    // A division ends the chapter it is written in: what
                    // follows it is the division's, not the chapter's.
                    self.close_open_chapter(first, position);
                    self.read_chapters(&division.blocks, Some(position));
                }
                _ => {}
            }
        }
        self.close_open_chapter(first, blocks.len());
    }

    /// End the last chapter of this container (those from `first` on) at
    /// `position`, if nothing has ended it yet.
    fn close_open_chapter(&mut self, first: usize, position: usize) {
        if let Some(last) = self.chapters[first..].last_mut()
            && last.end == usize::MAX
        {
            last.end = position;
        }
    }

    /// The chapter whose blocks hold the node at `path`.
    fn chapter_holding(&self, path: &[usize]) -> Option<usize> {
        self.chapters.iter().position(|chapter| {
            let block = match chapter.periph {
                None => path[0],
                Some(periph) if path[0] == periph && path.len() > 1 => path[1],
                Some(_) => return false,
            };
            chapter.start < block && block < chapter.end
        })
    }

    fn container(&self, chapter: &ChapterEntry) -> &'a [Block<'a>] {
        match chapter.periph {
            None => &self.document.blocks,
            Some(periph) => match &self.document.blocks[periph] {
                Block::Periph(division) => &division.blocks,
                _ => unreachable!("chapter entries point at periph divisions"),
            },
        }
    }

    pub fn document(&self) -> &'a Document<'a> {
        self.document
    }

    /// The book from the `\id` line, if any.
    pub fn book(&self) -> Option<BookCode> {
        self.book
    }

    /// Every chapter in document order, one per `\c` — including a number
    /// that repeats or goes backwards, which is exactly what the order checks
    /// read.
    pub fn chapters(&self) -> impl ExactSizeIterator<Item = ChapterRef<'_, 'a>> {
        (0..self.chapters.len()).map(|position| ChapterRef {
            index: self,
            position,
        })
    }

    /// The first chapter numbered `number`.
    pub fn chapter(&self, number: usize) -> Option<ChapterRef<'_, 'a>> {
        self.chapters().find(|chapter| chapter.number() == number)
    }

    /// Every verse in document order, one per `\v`, including any before the
    /// first chapter (whose [`VerseRef::chapter`] is `None`, and which no
    /// [`ChapterRef::verses`] yields) and any whose number repeats.
    pub fn verses(&self) -> impl ExactSizeIterator<Item = VerseRef<'_, 'a>> {
        (0..self.verses.len()).map(|position| VerseRef {
            index: self,
            position,
        })
    }

    /// The verse of chapter `chapter` whose number covers `verse`: verse 4
    /// is found in `\v 3-5`. The *first* one, where the number repeats;
    /// [`ReferenceIndex::verses`] has them all.
    pub fn verse(&self, chapter: usize, verse: usize) -> Option<VerseRef<'_, 'a>> {
        self.chapter(chapter)?.verse(verse)
    }
}

/// One chapter of an indexed document.
#[derive(Clone, Copy)]
pub struct ChapterRef<'i, 'a> {
    index: &'i ReferenceIndex<'a>,
    position: usize,
}

impl<'i, 'a> ChapterRef<'i, 'a> {
    fn entry(&self) -> &'i ChapterEntry {
        &self.index.chapters[self.position]
    }

    pub fn number(&self) -> usize {
        self.entry().number
    }

    /// The `\c` node.
    pub fn start(&self) -> &'a ChapterStart<'a> {
        let entry = self.entry();
        match &self.index.container(entry)[entry.start] {
            Block::ChapterStart(chapter) => chapter,
            _ => unreachable!("chapter entries point at chapter starts"),
        }
    }

    /// The blocks between the chapter's start and end milestones.
    pub fn blocks(&self) -> &'a [Block<'a>] {
        let entry = self.entry();
        &self.index.container(entry)[entry.start + 1..entry.end]
    }

    /// The chapter's verses in document order, one per `\v` between its start
    /// and end milestones, repeats and all.
    pub fn verses(&self) -> impl ExactSizeIterator<Item = VerseRef<'i, 'a>> {
        let index = self.index;
        self.entry()
            .verses
            .clone()
            .map(move |position| VerseRef { index, position })
    }

    /// The first verse whose number covers `number`: 4 is found in `\v 3-5`.
    pub fn verse(&self, number: usize) -> Option<VerseRef<'i, 'a>> {
        self.verses().find(|verse| verse.number().contains(number))
    }
}

/// One verse of an indexed document.
#[derive(Clone, Copy)]
pub struct VerseRef<'i, 'a> {
    index: &'i ReferenceIndex<'a>,
    position: usize,
}

impl<'i, 'a> VerseRef<'i, 'a> {
    fn entry(&self) -> &'i VerseEntry {
        &self.index.verses[self.position]
    }

    pub fn number(&self) -> &'i NumberList {
        &self.entry().number
    }

    pub fn chapter(&self) -> Option<ChapterRef<'i, 'a>> {
        self.entry().chapter.map(|position| ChapterRef {
            index: self.index,
            position,
        })
    }

    /// The `\v` node.
    pub fn start(&self) -> &'a VerseStart<'a> {
        NodeRef::Document(self.index.document)
            .descend(&self.entry().start)
            .and_then(|node| node.as_verse_start())
            .expect("verse entries point at verse starts")
    }

    pub fn start_path(&self) -> &'i [usize] {
        &self.entry().start
    }

    /// The path of the verse's end milestone, or `None` when the document
    /// has none for it (ends switched off at parse time, or a `\v` inside
    /// a sidebar).
    pub fn end_path(&self) -> Option<&'i [usize]> {
        self.entry().end.as_deref()
    }

    /// Where the verse's content stops: its end milestone, or, without
    /// one, the next verse start, or the end of the document.
    fn bound(&self) -> Option<&'i [usize]> {
        self.end_path().or_else(|| {
            self.index
                .verses
                .get(self.position + 1)
                .map(|next| next.start.as_slice())
        })
    }

    /// The content between the start and end milestones, in document
    /// order, as the largest nodes that fit: a block wholly inside the verse
    /// is one node, and a container the verse starts or ends inside (the
    /// paragraph holding the `\v`, a character style holding the end) is
    /// entered and contributes its children in range instead. Each node
    /// comes with its path, so a consumer can tell which block it is in or
    /// find the container it was lifted out of.
    pub fn nodes(&self) -> Vec<(NodePath, NodeRef<'a>)> {
        let start = self.start_path();
        let bound = self.bound();
        let mut out = Vec::new();
        let mut path = Vec::new();
        collect(
            NodeRef::Document(self.index.document),
            &mut path,
            start,
            bound,
            &mut out,
        );
        out
    }

    /// The verse's plain text with notes stripped and blocks joined by a
    /// space; see [`PlainText`] for other choices, via
    /// [`VerseRef::text_with`].
    pub fn text(&self) -> String {
        self.text_with(&mut PlainText::new(self.index.document.style_sheet()))
    }

    pub fn text_with(&self, extractor: &mut PlainText<'_>) -> String {
        extractor.of_located(self.nodes())
    }
}

/// The maximal nodes strictly between `start` and `bound`, in order.
fn collect<'a>(
    node: NodeRef<'a>,
    path: &mut NodePath,
    start: &[usize],
    bound: Option<&[usize]>,
    out: &mut Vec<(NodePath, NodeRef<'a>)>,
) {
    for index in 0..node.num_children() {
        path.push(index);
        let child = node.child(index).expect("index is in range");
        let on_start = start.starts_with(path);
        let on_bound = bound.is_some_and(|bound| bound.starts_with(path));
        if on_start || on_bound {
            // A milestone, or a container on the way to one: never yielded
            // whole, but its children in range are.
            collect(child, path, start, bound, out);
        } else if path[..] > *start && bound.is_none_or(|bound| path[..] < *bound) {
            out.push((path.clone(), child));
        }
        path.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usfm_ast::test_fixtures::sample_document;

    /// Parse through the facade, for the cases that are about what the parser
    /// puts in the tree (verse order) rather than about walking a tree of a
    /// known shape. A dev-dependency cycle, as in `tests/checks.rs`.
    fn parse(source: &str) -> Document<'_> {
        usfm::parse(source).document
    }

    #[test]
    fn chapters_and_verses_are_found() {
        let document = sample_document();
        let index = ReferenceIndex::new(&document);
        assert_eq!(index.book(), Some(BookCode::Gen));
        assert_eq!(index.chapters().len(), 1);
        let chapter = index.chapter(1).unwrap();
        assert_eq!(chapter.start().number, 1);
        // Two paragraphs, the sidebar and the table; not the milestones.
        assert_eq!(chapter.blocks().len(), 4);
        let numbers: Vec<String> = chapter.verses().map(|v| v.number().to_string()).collect();
        assert_eq!(numbers, ["1", "2"]);
        assert!(index.chapter(2).is_none());
        assert!(index.verse(1, 3).is_none());
    }

    #[test]
    fn a_verse_knows_its_milestones() {
        let document = sample_document();
        let index = ReferenceIndex::new(&document);
        let verse = index.verse(1, 2).unwrap();
        assert_eq!(verse.start().number.to_string(), "2");
        assert_eq!(verse.start_path(), [3, 2]);
        assert_eq!(verse.end_path(), Some(&[3usize, 5][..]));
        assert_eq!(verse.chapter().unwrap().number(), 1);
    }

    #[test]
    fn verse_content_is_the_largest_nodes_between_the_milestones() {
        let document = sample_document();
        let index = ReferenceIndex::new(&document);
        let verse = index.verse(1, 1).unwrap();
        let kinds: Vec<(NodePath, &str)> = verse
            .nodes()
            .iter()
            .map(|(path, node)| {
                let kind = match node {
                    NodeRef::Text(text) => text.content.as_ref(),
                    NodeRef::Char(_) => "<char>",
                    NodeRef::Note(_) => "<note>",
                    _ => "<other>",
                };
                (path.clone(), kind)
            })
            .collect();
        assert_eq!(
            kinds,
            [
                (vec![2, 1], "In the beginning "),
                (vec![2, 2], "<char>"),
                (vec![2, 3], " "),
                (vec![2, 4], "<char>"),
                (vec![2, 5], " the heavens"),
                (vec![2, 6], "<note>"),
            ]
        );
        assert_eq!(verse.text(), "In the beginning God created the heavens");
        assert_eq!(index.verse(1, 2).unwrap().text(), "the earth");
    }

    #[test]
    fn a_verse_without_an_end_runs_to_the_next_start() {
        let mut document = sample_document();
        // Strip the ends, as `parse_with_options(_, false)` would.
        for block in &mut document.blocks {
            if let Block::Para(para) = block {
                para.children
                    .retain(|inline| !matches!(inline, usfm_ast::Inline::VerseEnd(_)));
            }
        }
        let index = ReferenceIndex::new(&document);
        let first = index.verse(1, 1).unwrap();
        assert!(first.end_path().is_none());
        // Up to `\v 2`: the rest of the first paragraph, the milestone and
        // "and " from the second.
        assert_eq!(first.text(), "In the beginning God created the heavens and");
        // The last verse runs to the end of the document.
        assert_eq!(index.verse(1, 2).unwrap().text(), "the earth Aside Reuben");
    }

    /// The index reports the tree, not a tidied version of it: a chapter's
    /// `verses()` is one entry per `\v` in document order, so a number written
    /// twice is there twice and a number that goes backwards stays where it
    /// was written. That is what the order checks (ticket 23) read; `verse()`
    /// keeps answering with the first match, because a lookup wants *the*
    /// verse.
    #[test]
    fn verses_keep_document_order_when_a_number_repeats() {
        let document = parse("\\id MAT\n\\c 1\n\\p \\v 5 e \\v 6 f \\v 6 g \\v 4 h\n");
        let index = ReferenceIndex::new(&document);
        let chapter = index.chapter(1).unwrap();
        let numbers: Vec<String> = chapter.verses().map(|v| v.number().to_string()).collect();
        assert_eq!(numbers, ["5", "6", "6", "4"]);
        // Every verse is a distinct `\v`, in the order they were written.
        let starts: Vec<u32> = chapter.verses().map(|v| v.start().span.start).collect();
        assert!(starts.windows(2).all(|w| w[0] < w[1]), "{starts:?}");
        // The lookup takes the first of the two sixes.
        assert_eq!(chapter.verse(6).unwrap().text(), "f");
        // `verses()` on the index is the same run, since every verse is in a
        // chapter here.
        assert_eq!(index.verses().len(), 4);
    }

    /// A `\v` before the first `\c` belongs to no chapter: it is in
    /// `index.verses()` with `chapter() == None` and in no `ChapterRef`'s
    /// verses. Nothing is dropped, so a check can see it.
    #[test]
    fn a_verse_before_the_first_chapter_belongs_to_no_chapter() {
        let document = parse("\\id MAT\n\\p \\v 1 a\n\\c 1\n\\p \\v 2 b\n");
        let index = ReferenceIndex::new(&document);
        let numbers: Vec<String> = index.verses().map(|v| v.number().to_string()).collect();
        assert_eq!(numbers, ["1", "2"]);
        assert!(index.verses().next().unwrap().chapter().is_none());
        let in_chapter: Vec<String> = index
            .chapter(1)
            .unwrap()
            .verses()
            .map(|v| v.number().to_string())
            .collect();
        assert_eq!(in_chapter, ["2"]);
    }

    /// Chapters likewise: one entry per `\c`, in document order.
    #[test]
    fn chapters_keep_document_order_when_a_number_repeats() {
        let document =
            parse("\\id MAT\n\\c 1\n\\p \\v 1 a\n\\c 1\n\\p \\v 1 b\n\\c 3\n\\p \\v 1 c\n");
        let index = ReferenceIndex::new(&document);
        let numbers: Vec<usize> = index.chapters().map(|c| c.number()).collect();
        assert_eq!(numbers, [1, 1, 3]);
        let starts: Vec<u32> = index.chapters().map(|c| c.start().span.start).collect();
        assert!(starts.windows(2).all(|w| w[0] < w[1]), "{starts:?}");
        // The lookup takes the first chapter 1, whose verse is `a`.
        assert_eq!(index.verse(1, 1).unwrap().text(), "a");
    }

    /// A `\periph` division runs to the next `\periph` or `\id`, so in a
    /// front-matter book every `\c` after it is inside `Block::Periph`.
    /// `usx.rnc`'s `PeripheralContent` lists `Chapter`, so they are the
    /// book's chapters and the index reads them (ticket 38).
    #[test]
    fn chapters_inside_a_periph_division_are_indexed() {
        let document = parse(concat!(
            "\\id FRT\n\\periph Title|id=\"title\"\n",
            "\\c 1\n\\p \\v 1 a \\v 2 b\n",
            "\\c 2\n\\p \\v 1 c\n",
        ));
        assert!(matches!(document.blocks[1], Block::Periph(_)));
        let index = ReferenceIndex::new(&document);
        assert_eq!(index.book(), Some(BookCode::Frt));
        let numbers: Vec<usize> = index.chapters().map(|c| c.number()).collect();
        assert_eq!(numbers, [1, 2]);
        let first = index.chapter(1).unwrap();
        assert_eq!(first.start().number, 1);
        // The paragraph after `\c 1`, inside the division.
        assert_eq!(first.blocks().len(), 1);
        let verses: Vec<String> = first.verses().map(|v| v.number().to_string()).collect();
        assert_eq!(verses, ["1", "2"]);
        assert_eq!(index.verse(1, 2).unwrap().text(), "b");
        assert_eq!(index.verse(2, 1).unwrap().text(), "c");
        assert_eq!(index.verse(2, 1).unwrap().chapter().unwrap().number(), 2);
    }

    /// A division written after a chapter ends that chapter: what follows
    /// `\periph` is the division's, and a `\c` inside it is a new chapter.
    #[test]
    fn a_periph_division_ends_the_chapter_before_it() {
        let document = parse(concat!(
            "\\id GEN\n\\c 1\n\\p \\v 1 a\n",
            "\\periph Map|id=\"maps\"\n\\p \\v 2 b\n",
            "\\c 2\n\\p \\v 1 c\n",
        ));
        let index = ReferenceIndex::new(&document);
        let numbers: Vec<usize> = index.chapters().map(|c| c.number()).collect();
        assert_eq!(numbers, [1, 2]);
        let in_first: Vec<String> = index
            .chapter(1)
            .unwrap()
            .verses()
            .map(|v| v.number().to_string())
            .collect();
        assert_eq!(in_first, ["1"]);
        // `\v 2` stands in the division before its first `\c`: in no chapter.
        let orphan = index.verses().nth(1).unwrap();
        assert_eq!(orphan.number().to_string(), "2");
        assert!(orphan.chapter().is_none());
        assert_eq!(index.verse(2, 1).unwrap().text(), "c");
    }
}
