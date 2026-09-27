//! Which glossary entry a `\w` word refers to (ticket 55).
//!
//! A glossary book (`GLO`) defines each term with a keyword, `\k grace\k*`,
//! and the text marks a word that has an entry with `\w`: `\w grace\w*`, or
//! `\w favour|grace\w*` when the word as written is not the term (the default
//! attribute of `\w` is `lemma`). A [`GlossaryIndex`] is built from the
//! documents that hold the entries — usually the glossary book alone, which is
//! its own document — and answers, for a `\w` anywhere, which `\k` defines it.
//!
//! Terms are compared after [`normalise`]: the `\k`'s or the `\w`'s plain text
//! (or the lemma), whitespace collapsed, lower-cased. The first entry for a
//! term wins, as `ReferenceIndex` answers with the first match; a term defined
//! twice is in [`GlossaryIndex::duplicates`] rather than a diagnostic, because
//! the check would need the glossary and the text in one document.
//!
//! Like [`crate::ReferenceIndex`], this is derived from the tree and never
//! changes it; mutate a document and build the index again.

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use usfm_ast::text::PlainText;
use usfm_ast::visit::{self, Visit};
use usfm_ast::{Book, BookCode, ChapterStart, Char, Document, NodeRef, Span};

/// One `\k` keyword: a glossary term and where it is defined.
#[derive(Debug, Clone, PartialEq)]
pub struct GlossaryEntry {
    /// The keyword as written, whitespace collapsed.
    pub term: String,
    /// The index, into the documents the index was built from, of the one
    /// that holds the entry.
    pub document: usize,
    /// The book of the `\id` before the entry.
    pub book: Option<BookCode>,
    /// The chapter the entry is in, if there was a `\c` before it.
    pub chapter: Option<usize>,
    /// The `\k` node's span, in its document's source.
    pub span: Span,
}

/// A `\w` word and the entry it refers to, if there is one.
#[derive(Debug, Clone, PartialEq)]
pub struct GlossaryWord {
    /// The term looked up: the lemma if the word has one, otherwise its
    /// text, whitespace collapsed.
    pub term: String,
    /// The `\w` node's span.
    pub span: Span,
    /// The entry's index in [`GlossaryIndex::entries`], or `None` when no
    /// `\k` defines the term.
    pub entry: Option<usize>,
}

/// The glossary terms of one or more documents.
#[derive(Debug, Clone, Default)]
pub struct GlossaryIndex {
    entries: Vec<GlossaryEntry>,
    by_term: HashMap<String, usize>,
    duplicates: Vec<usize>,
}

impl GlossaryIndex {
    /// Index every `\k` in `documents`, in order.
    pub fn new(documents: &[&Document<'_>]) -> Self {
        let mut index = Self::default();
        for (number, document) in documents.iter().enumerate() {
            let mut walk = Walk::new(document, Looking::Keywords);
            walk.visit_document(document);
            for found in walk.found {
                let at = index.entries.len();
                let key = normalise(&found.term);
                if key.is_empty() {
                    continue;
                }
                match index.by_term.entry(key) {
                    Entry::Vacant(vacant) => {
                        vacant.insert(at);
                    }
                    Entry::Occupied(_) => index.duplicates.push(at),
                }
                index.entries.push(GlossaryEntry {
                    term: collapse(&found.term),
                    document: number,
                    book: found.book,
                    chapter: found.chapter,
                    span: found.span,
                });
            }
        }
        index
    }

    /// Every entry, in document order, repeats included.
    pub fn entries(&self) -> &[GlossaryEntry] {
        &self.entries
    }

    /// The entry that defines `term`, compared after [`normalise`].
    pub fn get(&self, term: &str) -> Option<&GlossaryEntry> {
        self.by_term
            .get(&normalise(term))
            .map(|&at| &self.entries[at])
    }

    /// The entries whose term an earlier entry already defines.
    pub fn duplicates(&self) -> impl Iterator<Item = &GlossaryEntry> {
        self.duplicates.iter().map(|&at| &self.entries[at])
    }

    /// Every `\w` in `document`, in document order, with the entry it refers
    /// to.
    pub fn words(&self, document: &Document<'_>) -> Vec<GlossaryWord> {
        let mut walk = Walk::new(document, Looking::Words);
        walk.visit_document(document);
        walk.found
            .into_iter()
            .map(|found| {
                let entry = self.by_term.get(&normalise(&found.term)).copied();
                GlossaryWord {
                    term: collapse(&found.term),
                    span: found.span,
                    entry,
                }
            })
            .collect()
    }

    /// The `\w` words in `document` that no entry defines.
    pub fn unresolved(&self, document: &Document<'_>) -> Vec<GlossaryWord> {
        self.words(document)
            .into_iter()
            .filter(|word| word.entry.is_none())
            .collect()
    }
}

/// A term as the index compares it: whitespace collapsed to one space and
/// trimmed, and lower-cased, so `Grace` in a heading-case glossary matches
/// `\w grace\w*` in the text.
pub fn normalise(term: &str) -> String {
    collapse(term).to_lowercase()
}

fn collapse(term: &str) -> String {
    term.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[derive(Clone, Copy, PartialEq)]
enum Looking {
    Keywords,
    Words,
}

struct Found {
    term: String,
    book: Option<BookCode>,
    chapter: Option<usize>,
    span: Span,
}

struct Walk<'d, 'a> {
    document: &'d Document<'a>,
    looking: Looking,
    book: Option<BookCode>,
    chapter: Option<usize>,
    found: Vec<Found>,
}

impl<'d, 'a> Walk<'d, 'a> {
    fn new(document: &'d Document<'a>, looking: Looking) -> Self {
        Self {
            document,
            looking,
            book: None,
            chapter: None,
            found: Vec::new(),
        }
    }
}

impl Visit for Walk<'_, '_> {
    fn visit_book(&mut self, book: &Book<'_>) {
        self.book = Some(book.code);
        self.chapter = None;
    }

    fn visit_chapter_start(&mut self, chapter: &ChapterStart<'_>) {
        self.chapter = Some(chapter.number);
    }

    fn visit_char(&mut self, char: &Char<'_>) {
        let sheet = self.document.style_sheet();
        let marker = sheet.get_rule(char.style.index()).marker.as_str();
        let wanted = match self.looking {
            Looking::Keywords => "k",
            Looking::Words => "w",
        };
        if marker != wanted {
            visit::walk_char(self, char);
            return;
        }
        // A `\w`'s lemma, written as `lemma="…"` or as the default attribute
        // (which the tree keeps under an empty name), is the term it refers
        // to; otherwise its text is.
        let lemma = (self.looking == Looking::Words)
            .then(|| {
                char.attributes.as_ref().and_then(|attributes| {
                    attributes
                        .pairs
                        .iter()
                        .find(|pair| pair.name.is_empty() || pair.name == "lemma")
                        .map(|pair| pair.value.to_string())
                })
            })
            .flatten();
        let term = lemma
            .filter(|lemma| !lemma.trim().is_empty())
            .unwrap_or_else(|| PlainText::new(sheet).of_node(NodeRef::Char(char)));
        self.found.push(Found {
            term,
            book: self.book,
            chapter: self.chapter,
            span: char.span,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalising_collapses_whitespace_and_case() {
        assert_eq!(normalise("  Holy \n Spirit "), "holy spirit");
        assert_eq!(normalise("فضل"), "فضل");
    }
}
