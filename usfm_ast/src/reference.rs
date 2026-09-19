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

use std::ops::Range;

use crate::text::PlainText;
use crate::{Block, BookCode, ChapterStart, Document, NodeRef, NumberList, VerseStart};

/// The path to a node from the document root: the block index, then the
/// index of each child on the way down. Paths order like document order: a
/// parent sorts before its descendants, a node before its later siblings.
pub type NodePath = Vec<usize>;

pub struct ReferenceIndex<'a> {
    document: &'a Document<'a>,
    book: Option<BookCode>,
    chapters: Vec<ChapterEntry>,
    verses: Vec<VerseEntry>,
}

struct ChapterEntry {
    number: usize,
    /// Block index of the `ChapterStart`.
    start: usize,
    /// Block index of the `ChapterEnd`, or of the next chapter, or the
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
    /// Index `document`. Chapters are read from the top-level blocks; verse
    /// starts are found anywhere, including inside sidebars and periphs,
    /// where the parser never places an end.
    pub fn build(document: &'a Document<'a>) -> Self {
        let mut index = Self {
            document,
            book: None,
            chapters: Vec::new(),
            verses: Vec::new(),
        };
        let blocks = &document.blocks;
        for (position, block) in blocks.iter().enumerate() {
            match block {
                Block::Book(book) => {
                    index.book.get_or_insert(book.code);
                }
                Block::ChapterStart(chapter) => {
                    if let Some(last) = index.chapters.last_mut()
                        && last.end == usize::MAX
                    {
                        last.end = position;
                    }
                    index.chapters.push(ChapterEntry {
                        number: chapter.number,
                        start: position,
                        end: usize::MAX,
                        verses: 0..0,
                    });
                }
                Block::ChapterEnd(chapter) => {
                    if let Some(last) = index.chapters.last_mut()
                        && last.end == usize::MAX
                        && last.number == chapter.number
                    {
                        last.end = position;
                    }
                }
                _ => {}
            }
        }
        if let Some(last) = index.chapters.last_mut()
            && last.end == usize::MAX
        {
            last.end = blocks.len();
        }

        let mut path = Vec::new();
        NodeRef::Document(document).descendants(&mut path, &mut |path, node| match node {
            NodeRef::VerseStart(verse) => {
                let chapter = index.chapter_holding(path[0]);
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

        // Verses are in document order and chapters are top-level, so each
        // chapter's verses are one contiguous run.
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

    fn chapter_holding(&self, block: usize) -> Option<usize> {
        self.chapters
            .iter()
            .position(|chapter| chapter.start < block && block < chapter.end)
    }

    pub fn document(&self) -> &'a Document<'a> {
        self.document
    }

    /// The book from the `\id` line, if any.
    pub fn book(&self) -> Option<BookCode> {
        self.book
    }

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

    /// Every verse in document order, including any before the first
    /// chapter.
    pub fn verses(&self) -> impl ExactSizeIterator<Item = VerseRef<'_, 'a>> {
        (0..self.verses.len()).map(|position| VerseRef {
            index: self,
            position,
        })
    }

    /// The verse of chapter `chapter` whose number covers `verse`: verse 4
    /// is found in `\v 3-5`.
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
        match &self.index.document.blocks[self.entry().start] {
            Block::ChapterStart(chapter) => chapter,
            _ => unreachable!("chapter entries point at chapter starts"),
        }
    }

    /// The blocks between the chapter's start and end milestones.
    pub fn blocks(&self) -> &'a [Block<'a>] {
        let entry = self.entry();
        &self.index.document.blocks[entry.start + 1..entry.end]
    }

    pub fn verses(&self) -> impl ExactSizeIterator<Item = VerseRef<'i, 'a>> {
        let index = self.index;
        self.entry()
            .verses
            .clone()
            .map(move |position| VerseRef { index, position })
    }

    /// The verse whose number covers `number`: 4 is found in `\v 3-5`.
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
    use crate::test_fixtures::sample_document;

    #[test]
    fn chapters_and_verses_are_found() {
        let document = sample_document();
        let index = document.reference_index();
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
        let index = document.reference_index();
        let verse = index.verse(1, 2).unwrap();
        assert_eq!(verse.start().number.to_string(), "2");
        assert_eq!(verse.start_path(), [3, 2]);
        assert_eq!(verse.end_path(), Some(&[3usize, 5][..]));
        assert_eq!(verse.chapter().unwrap().number(), 1);
    }

    #[test]
    fn verse_content_is_the_largest_nodes_between_the_milestones() {
        let document = sample_document();
        let index = document.reference_index();
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
                    .retain(|inline| !matches!(inline, crate::Inline::VerseEnd(_)));
            }
        }
        let index = document.reference_index();
        let first = index.verse(1, 1).unwrap();
        assert!(first.end_path().is_none());
        // Up to `\v 2`: the rest of the first paragraph, the milestone and
        // "and " from the second.
        assert_eq!(first.text(), "In the beginning God created the heavens and");
        // The last verse runs to the end of the document.
        assert_eq!(index.verse(1, 2).unwrap().text(), "the earth Aside Reuben");
    }
}
