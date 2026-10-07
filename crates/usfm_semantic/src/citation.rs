//! Scripture references written as text, as in `\xt Mt 5:3-10; Lk 6:20\xt*`
//! (ticket 54).
//!
//! A cross-reference's target is usually only in its text: `link-href` is
//! optional, and what a reader sees is `Mt 5:3-10` in the project's own
//! punctuation and its own book abbreviations. [`parse_citations`] reads such
//! text into [`Citation`]s, each a book, a start and an optional end, with the
//! byte range it was read from; the text between them (separators, words)
//! comes back as [`Piece::Text`] so the caller can rebuild the run with links
//! in it.
//!
//! The punctuation is a [`CitationFormat`], whose fields are Paratext's
//! reference settings (`ChapterVerseSeparator`, `RangeIndicator`, …); the book
//! names are a [`BookNameTable`], filled from `BookNames.xml`. Both come from a
//! project through `usfm_paratext`, or are built by hand. [`xt_citations`]
//! runs the parser over every `\xt` in a document, with the document's own
//! book as the default.
//!
//! The rules, which are Paratext's:
//!
//! * digits are any Unicode decimal digits, so `۵:۳` is 5:3;
//! * `5:3` is chapter 5 verse 3; a bare number after a book name is a whole
//!   chapter (`Ps 23`), except in a one-chapter book, where it is a verse
//!   (`Jude 5` is 1:5);
//! * after the sequence indicator a bare number keeps the chapter and names a
//!   verse if the reference before it named one (`5:3, 7` is 5:3 and 5:7), and
//!   is a chapter otherwise (`Ps 23, 24`); after the chapter-number or
//!   book-sequence separator it is a chapter (`5:3; 6:1`, `Ps 23; 24`);
//! * a range keeps both ends: `5:3-10` is 5:3 to 5:10, `5:3-6:2` crosses
//!   chapters, `3-5` after a book is chapters 3 to 5;
//! * a verse may carry one ASCII letter, its segment (`4a`);
//! * a number with no book name before it and no reference before it is not
//!   read as a reference — outside `\xt` that is just a number — unless it has
//!   a chapter–verse separator (`3:16`) and there is a default book;
//! * and not then either if an unread word stands directly before it:
//!   `1 Chronicles 3:11`, where the table has no such name, would otherwise
//!   be 3:11 of the default book, and nothing tells that word from the
//!   `see` of `see 3:11`. Punctuation before it is fine (`(3:16)`,
//!   `cf. 3:16`);
//! * whitespace may follow the chapter–verse separator (`23: 5-6`);
//! * directional marks (U+200E, U+200F, U+061C) inside a reference are
//!   skipped, since right-to-left projects write them around digits.
//! * a space in a book name matches a no-break space and the reverse;
//! * the words a project declares as extra material
//!   ([`CitationFormat::extra_material`]: `ch.` in `Mt ch. 5`) are passed
//!   over like whitespace where they stand as whole words.
//!
//! The reader does not guess: a wrong reference is worse than none, so
//! whatever is ambiguous stays [`Piece::Text`]. A bare number in an `\xt`
//! (`\xt 5\xt*`) is the plain case — a verse of this chapter as often as a
//! chapter of this book in real projects, with only the word beside it to
//! say which. Where the author settled it with a `link-href`
//! (`\xt 45|MAT 5:45\xt*`), [`xt_citations`] believes the attribute:
//! [`XtCitations::link`].
//!
//! A `C:V` alone in an `\xt` that has no `link-href` is the current book's.
//! That is the convention, and it is not a guess: if the author meant
//! another book and named it outside the `\xt`, the source text is what
//! needs fixing.
//!
//! Nothing here checks that the verse exists: that needs the other books.

use std::ops::Range;

use usfm_ast::text::PlainText;
use usfm_ast::visit::{self, Visit};
use usfm_ast::{Book, BookCode, Char, Document, NodeRef, Span};

/// A place in a book: a chapter, and a verse with its segment when the
/// reference names one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Location {
    pub chapter: usize,
    pub verse: Option<usize>,
    pub segment: Option<char>,
}

/// One reference read from text.
#[derive(Debug, Clone, PartialEq)]
pub struct Citation {
    pub book: BookCode,
    pub start: Location,
    /// The end of a range, `None` for a single place.
    pub end: Option<Location>,
    /// The bytes of the text it was read from, book name included when there
    /// was one.
    pub range: Range<usize>,
}

/// A run of text that is not a reference, or a reference.
#[derive(Debug, Clone, PartialEq)]
pub enum Piece {
    Text(Range<usize>),
    Citation(Citation),
}

/// The punctuation references are written with. Each field may hold several
/// spellings; each is compared with surrounding whitespace ignored.
#[derive(Debug, Clone, PartialEq)]
pub struct CitationFormat {
    /// Between a chapter and its verse: `:` in `5:3`.
    pub chapter_verse: Vec<String>,
    /// Between the ends of a range: `-` in `5:3-10`.
    pub range: Vec<String>,
    /// Between the ends of a range that crosses chapters, when a project
    /// spells it apart from `range`: `—` in `5:3—6:2`.
    pub chapter_range: Vec<String>,
    /// Between references in one chapter: `,` in `5:3, 7`.
    pub sequence: Vec<String>,
    /// Between chapters of one book: `;` in `5:3; 6:1`.
    pub chapter_number: Vec<String>,
    /// Between books: `;` in `Mt 5:3; Lk 6:20`.
    pub book_sequence: Vec<String>,
    /// Words a project writes inside a reference that say nothing the
    /// numbers do not (`ch.` in `Mt ch. 5:3`): Paratext's
    /// `ReferenceExtraMaterial`. Each is passed over like whitespace where it
    /// stands as a whole word. Empty by default.
    pub extra_material: Vec<String>,
}

impl Default for CitationFormat {
    /// Paratext's defaults for a new project.
    fn default() -> Self {
        let one = |s: &str| vec![s.to_string()];
        Self {
            chapter_verse: one(":"),
            range: one("-"),
            chapter_range: vec!["—".to_string(), "-".to_string()],
            sequence: one(","),
            chapter_number: one(";"),
            book_sequence: one(";"),
            extra_material: Vec::new(),
        }
    }
}

impl CitationFormat {
    /// The format of a `link-href` value (`GEN 1:1-3`), which USFM fixes
    /// whatever the project's own punctuation: the defaults.
    pub fn link_href() -> Self {
        Self::default()
    }
}

/// Book names to recognise, each mapped to its book.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BookNameTable {
    /// Longest name first, so `1 Cor` is tried before `1`.
    names: Vec<(String, BookCode)>,
    /// Whether a listed book's three-letter code (`MAT`), in capitals, is a
    /// name too.
    codes: bool,
    /// Whether a code is one in any case (`Mat`), as people write a
    /// `link-href` by hand.
    codes_any_case: bool,
}

impl BookNameTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// A table of the three-letter codes alone (`MAT`), which is what a
    /// `link-href` value names books with.
    pub fn codes() -> Self {
        Self::new().with_codes()
    }

    /// Recognise the listed books' three-letter codes as well as the names.
    pub fn with_codes(mut self) -> Self {
        self.codes = true;
        self
    }

    /// Recognise the codes however they are capitalised: `Mat`, `mat`.
    pub fn with_codes_in_any_case(mut self) -> Self {
        self.codes = true;
        self.codes_any_case = true;
        self
    }

    /// Recognise `name` as `code`. Surrounding whitespace is ignored and an
    /// empty name is not added.
    pub fn add(&mut self, name: &str, code: BookCode) {
        let name = name.trim();
        if name.is_empty() || self.names.iter().any(|(n, _)| n == name) {
            return;
        }
        let at = self
            .names
            .partition_point(|(existing, _)| existing.len() >= name.len());
        self.names.insert(at, (name.to_string(), code));
    }

    /// The longest name that `text` starts with, and the length in bytes of
    /// its spelling there. A space in either is a no-break space in the
    /// other: which of the two a name is typed with shows nowhere, and no
    /// two names differ by it.
    fn match_at(&self, text: &str) -> Option<(BookCode, usize)> {
        let named = self
            .names
            .iter()
            .find_map(|(name, code)| Some((*code, name_len_at(text, name)?)));
        if named.is_some() || !self.codes {
            return named;
        }
        let candidate = text.get(..3)?;
        let upper;
        let candidate = if self.codes_any_case && candidate.is_ascii() {
            upper = candidate.to_ascii_uppercase();
            upper.as_str()
        } else {
            candidate
        };
        if !candidate
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        {
            return None;
        }
        candidate
            .parse::<BookCode>()
            .ok()
            .filter(BookCode::is_listed)
            .map(|code| (code, 3))
    }
}

/// The length of `name` as `text` starts with it, a space and a no-break
/// space taken as the same character.
fn name_len_at(text: &str, name: &str) -> Option<usize> {
    if text.starts_with(name) {
        return Some(name.len());
    }
    let space = |c: char| if c == '\u{00A0}' { ' ' } else { c };
    let mut rest = text.chars();
    let mut len = 0;
    for expected in name.chars() {
        let found = rest.next()?;
        if space(found) != space(expected) {
            return None;
        }
        len += found.len_utf8();
    }
    Some(len)
}

/// Read the references in `text`. `default_book` is the book a reference
/// with no book name refers to (the document's own, for an `\xt`).
pub fn parse_citations(
    text: &str,
    format: &CitationFormat,
    names: &BookNameTable,
    default_book: Option<BookCode>,
) -> Vec<Piece> {
    Scanner {
        text,
        format,
        names,
        book: default_book,
        chapter: None,
        pieces: Vec::new(),
        text_start: 0,
    }
    .run()
}

/// What a bare number means where it stands.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Bare {
    Chapter,
    Verse,
    /// Not a reference at all, unless it has a chapter–verse separator.
    Nothing,
}

struct Scanner<'t> {
    text: &'t str,
    format: &'t CitationFormat,
    names: &'t BookNameTable,
    book: Option<BookCode>,
    chapter: Option<usize>,
    pieces: Vec<Piece>,
    text_start: usize,
}

impl Scanner<'_> {
    fn run(mut self) -> Vec<Piece> {
        let mut i = 0;
        // What a bare number would mean here: set by a reference and by the
        // separators after it, reset by anything else.
        let mut bare = Bare::Nothing;
        // Whether the reference before named a verse, which is what decides
        // a bare number after the sequence indicator.
        let mut last_had_verse = false;

        while i < self.text.len() {
            let at_boundary = !self.text[..i]
                .chars()
                .next_back()
                .is_some_and(char::is_alphanumeric);

            // A book name, then a location.
            if at_boundary
                && let Some((code, len)) = self.names.match_at(&self.text[i..])
                && !self.text[i + len..]
                    .chars()
                    .next()
                    .is_some_and(char::is_alphabetic)
            {
                let mut j = i + len;
                if self.text[j..].starts_with('.') {
                    j += 1;
                }
                j = self.skip_extra(j);
                let bare_here = if is_single_chapter(code) {
                    Bare::Verse
                } else {
                    Bare::Chapter
                };
                let chapter = is_single_chapter(code).then_some(1);
                if let Some((start, end, next)) = self.location(j, bare_here, chapter) {
                    self.push(code, start, end, i..next);
                    last_had_verse = start.verse.is_some();
                    bare = Bare::Verse;
                    i = next;
                    continue;
                }
            }

            // A location with the book from before — unless an unread word
            // stands before it, which may be the name of another book.
            if at_boundary
                && digit_value(self.text[i..].chars().next().unwrap()).is_some()
                && let Some(book) = self.book
                && !(bare == Bare::Nothing && self.follows_a_word(i))
            {
                let meaning = match bare {
                    Bare::Verse if !last_had_verse => Bare::Chapter,
                    other => other,
                };
                let meaning = if is_single_chapter(book) && meaning == Bare::Chapter {
                    Bare::Verse
                } else {
                    meaning
                };
                let chapter = if is_single_chapter(book) {
                    Some(1)
                } else {
                    self.chapter
                };
                if let Some((start, end, next)) = self.location(i, meaning, chapter) {
                    self.push(book, start, end, i..next);
                    last_had_verse = start.verse.is_some();
                    bare = Bare::Verse;
                    i = next;
                    continue;
                }
            }

            // Separators after a reference say what a bare number means next.
            if bare != Bare::Nothing {
                if let Some(len) = starts_with_any(&self.text[i..], &self.format.sequence) {
                    // Kept as it is: a verse after a verse, a chapter after
                    // a chapter.
                    i += len;
                    continue;
                }
                if let Some(len) = starts_with_any(&self.text[i..], &self.format.chapter_number)
                    .or_else(|| starts_with_any(&self.text[i..], &self.format.book_sequence))
                {
                    bare = Bare::Chapter;
                    last_had_verse = false;
                    i += len;
                    continue;
                }
            }

            // Extra material is not a word: it leaves a bare number its
            // meaning, as whitespace does.
            if at_boundary && let Some(len) = self.extra_at(i) {
                i += len;
                continue;
            }

            let c = self.text[i..].chars().next().unwrap();
            if !(c.is_whitespace() || is_directional_mark(c)) {
                bare = Bare::Nothing;
            }
            i += c.len_utf8();
        }
        if self.text_start < self.text.len() {
            self.pieces
                .push(Piece::Text(self.text_start..self.text.len()));
        }
        self.pieces
    }

    fn push(
        &mut self,
        book: BookCode,
        start: Location,
        end: Option<Location>,
        range: Range<usize>,
    ) {
        if self.text_start < range.start {
            self.pieces.push(Piece::Text(self.text_start..range.start));
        }
        self.text_start = range.end;
        self.book = Some(book);
        self.chapter = Some(end.unwrap_or(start).chapter);
        self.pieces.push(Piece::Citation(Citation {
            book,
            start,
            end,
            range,
        }));
    }

    /// A location at `i`: `C:V`, or a bare number meaning `bare`, then an
    /// optional range. `chapter` is the chapter a bare verse is in.
    fn location(
        &self,
        i: usize,
        bare: Bare,
        chapter: Option<usize>,
    ) -> Option<(Location, Option<Location>, usize)> {
        let (first, segment, after_first) = self.number(i)?;
        let (start, mut next) = match self.chapter_verse(after_first) {
            Some((verse, verse_segment, after)) => (
                Location {
                    chapter: first,
                    verse: Some(verse),
                    segment: verse_segment,
                },
                after,
            ),
            None => match bare {
                Bare::Chapter if segment.is_none() => (
                    Location {
                        chapter: first,
                        verse: None,
                        segment: None,
                    },
                    after_first,
                ),
                Bare::Verse => (
                    Location {
                        chapter: chapter?,
                        verse: Some(first),
                        segment,
                    },
                    after_first,
                ),
                _ => return None,
            },
        };

        let mut end = None;
        let separator = self.skip_marks(next);
        let range_len = starts_with_any_exact(&self.text[separator..], &self.format.range)
            .or_else(|| starts_with_any_exact(&self.text[separator..], &self.format.chapter_range));
        if let Some(len) = range_len {
            let j = self.skip_marks(separator + len);
            if let Some((second, second_segment, after_second)) = self.number(j) {
                if let Some((verse, verse_segment, after)) = self.chapter_verse(after_second) {
                    end = Some(Location {
                        chapter: second,
                        verse: Some(verse),
                        segment: verse_segment,
                    });
                    next = after;
                } else if start.verse.is_some() {
                    end = Some(Location {
                        chapter: start.chapter,
                        verse: Some(second),
                        segment: second_segment,
                    });
                    next = after_second;
                } else if second_segment.is_none() {
                    end = Some(Location {
                        chapter: second,
                        verse: None,
                        segment: None,
                    });
                    next = after_second;
                }
            }
        }
        Some((start, end, next))
    }

    /// `:V` at `i`, marks skipped, and whitespace after the separator too
    /// (`23: 5`); none before it.
    fn chapter_verse(&self, i: usize) -> Option<(usize, Option<char>, usize)> {
        let i = self.skip_marks(i);
        let len = starts_with_any_exact(&self.text[i..], &self.format.chapter_verse)?;
        self.number(self.skip_space(i + len))
    }

    /// Whether the last thing before `i` — whitespace, marks and extra
    /// material aside — is a letter or a digit of text nothing has read.
    fn follows_a_word(&self, i: usize) -> bool {
        let skipped = |c: char| c.is_whitespace() || is_directional_mark(c);
        let mut before = self.text[self.text_start..i].trim_end_matches(skipped);
        while let Some(rest) = self.format.extra_material.iter().find_map(|extra| {
            let extra = extra.trim();
            let rest = before.strip_suffix(extra).filter(|_| !extra.is_empty())?;
            (!rest.ends_with(char::is_alphanumeric)).then_some(rest)
        }) {
            before = rest.trim_end_matches(skipped);
        }
        before.ends_with(char::is_alphanumeric)
    }

    /// The length of the extra material at `i`, which the caller knows to be
    /// at the start of a word: one of the format's phrases, with no letter or
    /// digit directly after it.
    fn extra_at(&self, i: usize) -> Option<usize> {
        let text = &self.text[i..];
        self.format
            .extra_material
            .iter()
            .map(|extra| extra.trim())
            .filter(|extra| !extra.is_empty())
            .filter(|extra| text.starts_with(extra))
            .filter(|extra| !text[extra.len()..].starts_with(char::is_alphanumeric))
            .map(str::len)
            .max()
    }

    /// Past whitespace, marks and extra material.
    fn skip_extra(&self, mut i: usize) -> usize {
        loop {
            i = self.skip_space(i);
            match self.extra_at(i) {
                Some(len) => i += len,
                None => return i,
            }
        }
    }

    /// A number at `i` and its segment letter, and where they end. The number
    /// must not run on into a letter other than the segment.
    fn number(&self, i: usize) -> Option<(usize, Option<char>, usize)> {
        let mut value: usize = 0;
        let mut end = i;
        for c in self.text[i..].chars() {
            let Some(digit) = digit_value(c) else { break };
            value = value.checked_mul(10)?.checked_add(digit as usize)?;
            end += c.len_utf8();
        }
        if end == i {
            return None;
        }
        let mut rest = self.text[end..].chars();
        let segment = match (rest.next(), rest.next()) {
            (Some(c), after)
                if c.is_ascii_lowercase() && !after.is_some_and(char::is_alphanumeric) =>
            {
                end += 1;
                Some(c)
            }
            (Some(c), _) if c.is_alphabetic() => return None,
            _ => None,
        };
        Some((value, segment, end))
    }

    fn skip_space(&self, mut i: usize) -> usize {
        for c in self.text[i..].chars() {
            if !(c.is_whitespace() || is_directional_mark(c)) {
                break;
            }
            i += c.len_utf8();
        }
        i
    }

    fn skip_marks(&self, mut i: usize) -> usize {
        for c in self.text[i..].chars() {
            if !is_directional_mark(c) {
                break;
            }
            i += c.len_utf8();
        }
        i
    }
}

/// The length of the first of `spellings` that `text` starts with, spaces
/// around the spelling included on both sides.
fn starts_with_any(text: &str, spellings: &[String]) -> Option<usize> {
    let trimmed = text.trim_start();
    let leading = text.len() - trimmed.len();
    spellings
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .find(|s| trimmed.starts_with(s))
        .map(|s| {
            let after = &trimmed[s.len()..];
            leading + s.len() + (after.len() - after.trim_start().len())
        })
}

/// As [`starts_with_any`], with no space allowed before the spelling (a
/// range or a chapter–verse separator is written tight).
fn starts_with_any_exact(text: &str, spellings: &[String]) -> Option<usize> {
    spellings
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .find(|s| text.starts_with(s))
        .map(str::len)
}

/// The books with one chapter, where a bare number is a verse.
fn is_single_chapter(code: BookCode) -> bool {
    matches!(code.as_str(), "OBA" | "PHM" | "2JN" | "3JN" | "JUD")
}

fn is_directional_mark(c: char) -> bool {
    matches!(c, '\u{200E}' | '\u{200F}' | '\u{061C}')
}

/// The value of a decimal digit of any of the scripts Paratext projects
/// write numbers in.
fn digit_value(c: char) -> Option<u32> {
    // The zero of every block of ten decimal digits in the Unicode ranges
    // Bible translations use; each block is contiguous.
    const ZEROS: [u32; 19] = [
        0x0030, 0x0660, 0x06F0, 0x07C0, 0x0966, 0x09E6, 0x0A66, 0x0AE6, 0x0B66, 0x0BE6, 0x0C66,
        0x0CE6, 0x0D66, 0x0E50, 0x0ED0, 0x0F20, 0x1040, 0x17E0, 0xFF10,
    ];
    let c = c as u32;
    ZEROS
        .iter()
        .find(|zero| (**zero..**zero + 10).contains(&c))
        .map(|zero| c - zero)
}

/// The references in one `\xt`.
#[derive(Debug, Clone, PartialEq)]
pub struct XtCitations {
    /// The `\xt` node's span.
    pub span: Span,
    /// Its text, as [`PlainText`] reads it; the pieces' ranges are into this.
    pub text: String,
    /// What the text reads to. When [`Self::link`] is `Some` this is still
    /// the text's own reading, for a caller that wants to see both; the
    /// link is the one to believe.
    pub pieces: Vec<Piece>,
    /// The `\xt`'s `link-href`, when it has one that is references and
    /// nothing else.
    pub link: Option<LinkHref>,
}

/// A `link-href` value that names Scripture: `MAT 5:45`, `1SA 21:1-15`.
#[derive(Debug, Clone, PartialEq)]
pub struct LinkHref {
    /// The attribute's value; the citations' ranges are into this.
    pub value: String,
    pub citations: Vec<Citation>,
}

impl XtCitations {
    /// The references the `\xt` points at: its `link-href`'s when it has
    /// one, since that is the author saying so, and otherwise the ones read
    /// from its text.
    pub fn citations(&self) -> Vec<&Citation> {
        match &self.link {
            Some(link) => link.citations.iter().collect(),
            None => self
                .pieces
                .iter()
                .filter_map(|piece| match piece {
                    Piece::Citation(citation) => Some(citation),
                    Piece::Text(_) => None,
                })
                .collect(),
        }
    }
}

/// Read a `link-href` value as references: book codes in any case, USFM's
/// own punctuation, direction marks skipped. `None` unless the whole value is
/// references and the separators between them — a URL, a `prj:` link or
/// anything else with a word in it is not one.
pub fn parse_link_href(value: &str) -> Option<Vec<Citation>> {
    let names = BookNameTable::new().with_codes_in_any_case();
    let pieces = parse_citations(value, &CitationFormat::link_href(), &names, None);
    let mut citations = Vec::new();
    for piece in pieces {
        match piece {
            Piece::Citation(citation) => citations.push(citation),
            Piece::Text(range) => {
                let only_separators = value[range]
                    .chars()
                    .all(|c| c.is_whitespace() || is_directional_mark(c) || matches!(c, ',' | ';'));
                if !only_separators {
                    return None;
                }
            }
        }
    }
    (!citations.is_empty()).then_some(citations)
}

/// Read the text of every `\xt` in `document`, in document order, with the
/// book of the `\id` before it as the default.
pub fn xt_citations(
    document: &Document<'_>,
    format: &CitationFormat,
    names: &BookNameTable,
) -> Vec<XtCitations> {
    let mut walk = XtWalk {
        document,
        format,
        names,
        book: None,
        found: Vec::new(),
    };
    walk.visit_document(document);
    walk.found
}

struct XtWalk<'d, 'a> {
    document: &'d Document<'a>,
    format: &'d CitationFormat,
    names: &'d BookNameTable,
    book: Option<BookCode>,
    found: Vec<XtCitations>,
}

impl Visit for XtWalk<'_, '_> {
    fn visit_book(&mut self, book: &Book<'_>) {
        self.book = Some(book.code);
    }

    fn visit_char(&mut self, char: &Char<'_>) {
        let sheet = self.document.style_sheet();
        if sheet.get_rule(char.style.index()).marker == "xt" {
            let text = PlainText::new(sheet).of_node(NodeRef::Char(char));
            let pieces = parse_citations(&text, self.format, self.names, self.book);
            // The default attribute of `\xt` is `link-href`, and its name is
            // empty in the tree.
            let link = char.attributes.as_ref().and_then(|attributes| {
                let href = attributes
                    .pairs
                    .iter()
                    .find(|pair| pair.name.is_empty() || pair.name == "link-href")?;
                Some(LinkHref {
                    citations: parse_link_href(&href.value)?,
                    value: href.value.to_string(),
                })
            });
            self.found.push(XtCitations {
                span: char.span,
                text,
                pieces,
                link,
            });
            return;
        }
        visit::walk_char(self, char);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(s: &str) -> BookCode {
        s.parse().unwrap()
    }

    fn names() -> BookNameTable {
        let mut names = BookNameTable::new();
        names.add("Mt", code("MAT"));
        names.add("Lk", code("LUK"));
        names.add("Ps", code("PSA"));
        names.add("1 Cor", code("1CO"));
        names.add("Jude", code("JUD"));
        names
    }

    fn loc(chapter: usize, verse: Option<usize>) -> Location {
        Location {
            chapter,
            verse,
            segment: None,
        }
    }

    /// The citations alone, as `BOOK start[-end]` strings, for short asserts.
    fn read(text: &str, format: &CitationFormat, default: Option<&str>) -> Vec<String> {
        parse_citations(text, format, &names(), default.map(code))
            .into_iter()
            .filter_map(|piece| match piece {
                Piece::Citation(c) => Some(describe(&c)),
                Piece::Text(_) => None,
            })
            .collect()
    }

    fn describe(c: &Citation) -> String {
        let place = |l: &Location| match l.verse {
            Some(v) => format!(
                "{}:{v}{}",
                l.chapter,
                l.segment.map(String::from).unwrap_or_default()
            ),
            None => l.chapter.to_string(),
        };
        match &c.end {
            Some(end) => format!("{} {}-{}", c.book, place(&c.start), place(end)),
            None => format!("{} {}", c.book, place(&c.start)),
        }
    }

    fn english(text: &str) -> Vec<String> {
        read(text, &CitationFormat::default(), None)
    }

    #[test]
    fn a_single_verse_and_its_range() {
        assert_eq!(english("Mt 5:3"), ["MAT 5:3"]);
        assert_eq!(english("Mt 5:3-10"), ["MAT 5:3-5:10"]);
        assert_eq!(english("Mt 5:3-6:2"), ["MAT 5:3-6:2"]);
        assert_eq!(english("Mt 5:3—6:2"), ["MAT 5:3-6:2"]);
    }

    #[test]
    fn a_whole_chapter_and_a_range_of_chapters() {
        assert_eq!(english("Ps 23"), ["PSA 23"]);
        assert_eq!(english("Ps 23-25"), ["PSA 23-25"]);
    }

    #[test]
    fn a_sequence_keeps_the_chapter_and_the_book() {
        assert_eq!(
            english("Mt 5:3, 7, 9-11"),
            ["MAT 5:3", "MAT 5:7", "MAT 5:9-5:11"]
        );
        assert_eq!(english("Mt 5:3; 6:1"), ["MAT 5:3", "MAT 6:1"]);
        assert_eq!(english("Ps 23, 24"), ["PSA 23", "PSA 24"]);
        assert_eq!(english("Mt 5:3; Lk 6:20"), ["MAT 5:3", "LUK 6:20"]);
    }

    #[test]
    fn a_one_chapter_book_takes_a_bare_number_as_a_verse() {
        assert_eq!(english("Jude 5"), ["JUD 1:5"]);
        assert_eq!(english("Jude 5, 7"), ["JUD 1:5", "JUD 1:7"]);
    }

    #[test]
    fn a_name_with_a_digit_and_a_space_is_matched_whole() {
        assert_eq!(english("1 Cor 13:4"), ["1CO 13:4"]);
    }

    /// Which of the two spaces a name is typed with shows nowhere, so either
    /// matches the other, and the citation covers the spelling in the text.
    #[test]
    fn a_no_break_space_in_a_name_is_a_space() {
        assert_eq!(english("1\u{00A0}Cor 5:3"), ["1CO 5:3"]);
        let mut names = BookNameTable::new();
        names.add("1\u{00A0}Cor", code("1CO"));
        let text = "see 1 Cor 5:3";
        let pieces = parse_citations(text, &CitationFormat::default(), &names, None);
        let [Piece::Text(_), Piece::Citation(citation)] = pieces.as_slice() else {
            panic!("{pieces:?}");
        };
        assert_eq!(describe(citation), "1CO 5:3");
        assert_eq!(&text[citation.range.clone()], "1 Cor 5:3");
    }

    /// A project's extra material is passed over like whitespace, as a whole
    /// word only, and is no reference of its own.
    #[test]
    fn extra_material_is_passed_over() {
        let format = CitationFormat {
            extra_material: vec!["chapter".to_string(), "ch.".to_string()],
            ..CitationFormat::default()
        };
        assert_eq!(read("Mt chapter 5", &format, None), ["MAT 5"]);
        assert_eq!(
            read("Mt ch. 5:3; ch. 6", &format, None),
            ["MAT 5:3", "MAT 6"]
        );
        // Not declared, so a word like any other.
        assert!(english("Mt chapter 5").is_empty());
        // Part of a longer word.
        assert!(read("Mt chapters 5", &format, None).is_empty());
        assert!(read("chapter 5", &format, Some("MAT")).is_empty());
        // It is not the unread word that may be another book's name, and
        // does not hide one either.
        assert_eq!(read("chapter 3:16", &format, Some("MAT")), ["MAT 3:16"]);
        assert!(read("Acts chapter 3:16", &format, Some("MAT")).is_empty());
    }

    #[test]
    fn a_segment_letter_is_kept() {
        assert_eq!(english("Mt 5:3a-4b"), ["MAT 5:3a-5:4b"]);
    }

    #[test]
    fn a_bare_number_is_text_without_a_book() {
        assert_eq!(english("see 5 more"), Vec::<String>::new());
        // With a default book, `3:16` is a reference and a bare `5` is not.
        assert_eq!(
            read("3:16 and 5", &CitationFormat::default(), Some("JHN")),
            ["JHN 3:16"]
        );
    }

    /// A wrong reference is worse than none. With John as the default book,
    /// `1 Chronicles 3:11` — a name the table does not have — was read as
    /// John 3:11: found in a real project whose `BookNames.xml` has no entry
    /// for 1 Chronicles (2026-10-06). Nothing tells that word from `see`, so
    /// neither is read; punctuation before the number is no word.
    #[test]
    fn a_reference_after_an_unread_word_is_not_given_the_default_book() {
        let john = |text| read(text, &CitationFormat::default(), Some("JHN"));
        assert_eq!(john("1 Chronicles 3:11-12"), Vec::<String>::new());
        assert_eq!(john("see 3:11"), Vec::<String>::new());
        assert_eq!(john("Mt 5:3; Chronicles 3:4"), ["MAT 5:3"]);
        assert_eq!(john("3:11"), ["JHN 3:11"]);
        assert_eq!(john("(3:11)"), ["JHN 3:11"]);
        assert_eq!(john("cf. 3:11; 4:2"), ["JHN 3:11", "JHN 4:2"]);
        assert_eq!(john("\u{200F}3:11"), ["JHN 3:11"]);
    }

    /// `23: 5-6`, as one project writes every reference: the verses were
    /// dropped and the chapter read alone.
    #[test]
    fn whitespace_may_follow_the_chapter_verse_separator() {
        assert_eq!(english("Ps 23: 5-6"), ["PSA 23:5-23:6"]);
        assert_eq!(english("Ps 23:\u{200F} 5\u{200F}-6"), ["PSA 23:5-23:6"]);
        // Not before it: `23 :5` is chapter 23 and something else.
        assert_eq!(english("Ps 23 :5"), ["PSA 23"]);
    }

    #[test]
    fn a_link_href_is_references_and_nothing_else() {
        let read = |value| {
            parse_link_href(value).map(|found| found.iter().map(describe).collect::<Vec<_>>())
        };
        assert_eq!(read("MAT 5:45"), Some(vec!["MAT 5:45".to_string()]));
        assert_eq!(
            read("Mat 5\u{200F}:45 "),
            Some(vec!["MAT 5:45".to_string()])
        );
        assert_eq!(
            read("1SA 21\u{200F}:1\u{200F}-15"),
            Some(vec!["1SA 21:1-21:15".to_string()])
        );
        assert_eq!(
            read("GEN 1:1; EXO 2:3"),
            Some(vec!["GEN 1:1".to_string(), "EXO 2:3".to_string()])
        );
        for not_scripture in [
            "",
            "https://example.org/1:2",
            "prj:GEN 1:1",
            "5:45",
            "MAT 5:45 note",
            "Matthew 5:45",
        ] {
            assert_eq!(read(not_scripture), None, "{not_scripture:?}");
        }
    }

    #[test]
    fn words_break_the_sequence() {
        assert_eq!(english("Mt 5:3 and also 7"), ["MAT 5:3"]);
    }

    #[test]
    fn the_text_between_is_kept_as_pieces() {
        let text = "Mt 5:3; Lk 6:20.";
        let pieces = parse_citations(text, &CitationFormat::default(), &names(), None);
        let texts: Vec<&str> = pieces
            .iter()
            .map(|p| match p {
                Piece::Text(r) => &text[r.clone()],
                Piece::Citation(c) => &text[c.range.clone()],
            })
            .collect();
        assert_eq!(texts, ["Mt 5:3", "; ", "Lk 6:20", "."]);
    }

    /// An Urdu project: Extended Arabic-Indic digits, the Arabic comma as the
    /// sequence indicator, the Arabic semicolon between chapters, book names
    /// in Urdu, and right-to-left marks around the numbers. render's own
    /// parser reads none of these digits (its `\d` is ASCII).
    #[test]
    fn an_urdu_reference() {
        let format = CitationFormat {
            sequence: vec!["،".to_string()],
            chapter_number: vec!["؛".to_string()],
            book_sequence: vec!["؛".to_string()],
            ..CitationFormat::default()
        };
        let mut names = BookNameTable::new();
        names.add("متی", code("MAT"));
        names.add("لوقا", code("LUK"));
        let text = "متی \u{200F}۵:۳\u{200F}-۱۰\u{200F}، ۱۲؛ لوقا ۶:۲۰";
        let found: Vec<String> = parse_citations(text, &format, &names, None)
            .iter()
            .filter_map(|p| match p {
                Piece::Citation(c) => Some(describe(c)),
                Piece::Text(_) => None,
            })
            .collect();
        assert_eq!(found, ["MAT 5:3-5:10", "MAT 5:12", "LUK 6:20"]);
    }

    #[test]
    fn a_link_href_reads_with_the_codes() {
        let found = read_with_codes("GEN 1:1-3,5");
        assert_eq!(found, ["GEN 1:1-1:3", "GEN 1:5"]);
    }

    fn read_with_codes(text: &str) -> Vec<String> {
        parse_citations(
            text,
            &CitationFormat::link_href(),
            &BookNameTable::codes(),
            None,
        )
        .iter()
        .filter_map(|p| match p {
            Piece::Citation(c) => Some(describe(c)),
            Piece::Text(_) => None,
        })
        .collect()
    }

    #[test]
    fn locations_are_what_they_say() {
        let pieces = parse_citations("Mt 5", &CitationFormat::default(), &names(), None);
        assert_eq!(
            pieces,
            [Piece::Citation(Citation {
                book: code("MAT"),
                start: loc(5, None),
                end: None,
                range: 0..4,
            })]
        );
        let _ = loc(1, Some(1));
    }

    #[test]
    fn a_number_too_large_is_not_a_reference() {
        assert_eq!(
            english("Mt 99999999999999999999999:1"),
            Vec::<String>::new()
        );
    }
}
