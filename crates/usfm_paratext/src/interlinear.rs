//! `Interlinear_{language}_{book}.xml`: Paratext 9's interlinear glosses for
//! one book in one gloss language (ticket 56).
//!
//! For each verse the file lists **clusters**: a character range in the verse
//! and the lexemes chosen for it, each with the id of the lexicon sense that
//! glosses it (`Lexicon.xml`, [`crate::Lexicon`]). The format is documented in
//! SIL's interlinearizer extension (`src/parsers/pt9/pt9-xml.md`, MIT), whose
//! test projects are this crate's fixtures; this reader follows that
//! document's rules: a duplicate verse keeps the last occurrence, a missing
//! `Range` reads as `(0, 0)`, a non-numeric one or a malformed `Excluded`
//! fails the file.
//!
//! A range is **not a place in the tree**. It counts characters of Paratext's
//! own string for the verse, and Paratext does not rewrite ranges when the
//! text changes; it matches an analysis to a word by the lexeme's form. So
//! [`crate::anchor`] places clusters by form and uses the range only as
//! ordering and to choose between repeats.

use std::fmt;

use usfm_ast::BookCode;

/// A lexeme's identity: `Word:logos`, `Stem:exauc`, `Word:a:2` (homograph 2).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LexemeKey {
    pub kind: LexemeType,
    pub form: String,
    /// 1 unless the id says otherwise.
    pub homograph: i32,
}

/// Paratext's lexeme types. Its list is append-only, so a name it may add
/// later survives as `Other`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LexemeType {
    Phrase,
    Word,
    Lemma,
    Stem,
    Prefix,
    Suffix,
    Infix,
    Other(String),
}

impl LexemeType {
    pub fn from_name(name: &str) -> Self {
        match name {
            "Phrase" => Self::Phrase,
            "Word" => Self::Word,
            "Lemma" => Self::Lemma,
            "Stem" => Self::Stem,
            "Prefix" => Self::Prefix,
            "Suffix" => Self::Suffix,
            "Infix" => Self::Infix,
            other => Self::Other(other.to_string()),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Phrase => "Phrase",
            Self::Word => "Word",
            Self::Lemma => "Lemma",
            Self::Stem => "Stem",
            Self::Prefix => "Prefix",
            Self::Suffix => "Suffix",
            Self::Infix => "Infix",
            Self::Other(name) => name,
        }
    }

    /// A morpheme of a word parse: a stem, prefix or suffix. A cluster with
    /// one of these is a parse however many lexemes it has.
    pub fn is_morpheme(&self) -> bool {
        matches!(self, Self::Stem | Self::Prefix | Self::Suffix)
    }
}

impl LexemeKey {
    /// Parse a composed id, `Type:Form[:Homograph]`. The type is a run of
    /// word characters; the form may hold colons, and a trailing `:digits`
    /// is always the homograph, as Paratext reads it.
    pub fn parse(id: &str) -> Option<Self> {
        let (kind, rest) = id.split_once(':')?;
        if kind.is_empty() || !kind.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return None;
        }
        let (form, homograph) = match rest.rsplit_once(':') {
            Some((form, number))
                if !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()) =>
            {
                (form, number.parse().ok()?)
            }
            _ => (rest, 1),
        };
        Some(Self {
            kind: LexemeType::from_name(kind),
            form: form.to_string(),
            homograph,
        })
    }
}

impl fmt::Display for LexemeKey {
    /// The composed id, homograph 1 left out.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.kind.name(), self.form)?;
        if self.homograph != 1 {
            write!(f, ":{}", self.homograph)?;
        }
        Ok(())
    }
}

/// Characters of Paratext's verse string: where a cluster or a punctuation
/// change is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextRange {
    pub index: usize,
    pub length: usize,
}

/// One lexeme chosen for a cluster, as written: the id is kept verbatim so a
/// malformed one is still there to report.
#[derive(Debug, Clone, PartialEq)]
pub struct ClusterLexeme {
    pub id: Option<String>,
    /// The chosen sense in `Lexicon.xml`; `None` when none was chosen.
    pub sense: Option<String>,
}

impl ClusterLexeme {
    pub fn key(&self) -> Option<LexemeKey> {
        LexemeKey::parse(self.id.as_deref()?)
    }
}

/// A word, a word's parse or a phrase, and what glosses it.
#[derive(Debug, Clone, PartialEq)]
pub struct Cluster {
    pub range: TextRange,
    pub lexemes: Vec<ClusterLexeme>,
    /// Left out of the interlinear display.
    pub excluded: bool,
}

/// What a cluster is. Paratext stores no type: it follows from the lexemes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClusterKind {
    /// One `Word` lexeme.
    Word,
    /// Any stem, prefix or suffix: a word divided into morphemes.
    Parse,
    /// One `Phrase` lexeme, spanning several words.
    Phrase,
    /// A `Lemma` or `Infix` cluster (legacy data modern Paratext ignores),
    /// an empty one, or one of several words and phrases.
    Other,
    /// A lexeme with no id, or one that is not `Type:Form[:Homograph]`.
    Unparseable,
}

impl Cluster {
    pub fn kind(&self) -> ClusterKind {
        let Some(keys) = self
            .lexemes
            .iter()
            .map(ClusterLexeme::key)
            .collect::<Option<Vec<_>>>()
        else {
            return ClusterKind::Unparseable;
        };
        if keys.iter().any(|key| key.kind.is_morpheme()) {
            return ClusterKind::Parse;
        }
        match keys.as_slice() {
            [key] if key.kind == LexemeType::Word => ClusterKind::Word,
            [key] if key.kind == LexemeType::Phrase => ClusterKind::Phrase,
            _ => ClusterKind::Other,
        }
    }

    /// The text the cluster stands for: a word's or a phrase's form, or a
    /// parse's morphemes joined (`Stem:wug` + `Suffix:s` is `wugs`). `None`
    /// for a cluster of another kind.
    pub fn surface(&self) -> Option<String> {
        let keys: Vec<LexemeKey> = self.lexemes.iter().filter_map(ClusterLexeme::key).collect();
        match self.kind() {
            ClusterKind::Word | ClusterKind::Phrase => Some(keys[0].form.clone()),
            ClusterKind::Parse => Some(keys.iter().map(|key| key.form.as_str()).collect()),
            ClusterKind::Other | ClusterKind::Unparseable => None,
        }
    }
}

/// A back translation's change of punctuation, kept as read.
#[derive(Debug, Clone, PartialEq)]
pub struct Punctuation {
    pub range: TextRange,
    pub before: Option<String>,
    pub after: Option<String>,
}

/// One verse's clusters.
#[derive(Debug, Clone, PartialEq)]
pub struct InterlinearVerse {
    /// The verse as the file keys it: `PHP 1:1`.
    pub reference: String,
    /// The hash of the verse text when its glosses were approved; `None`
    /// while they are not.
    pub approved_hash: Option<String>,
    pub clusters: Vec<Cluster>,
    pub punctuation: Vec<Punctuation>,
}

impl InterlinearVerse {
    /// The chapter and verse numbers of [`Self::reference`]. For a range or a
    /// segment (`1:3-4`, `1:3a`) the verse is the first number.
    pub fn chapter_verse(&self) -> Option<(usize, usize)> {
        let (_, numbers) = self.reference.trim().split_once(' ')?;
        let (chapter, verse) = numbers.trim().split_once(':')?;
        let digits = |s: &str| -> Option<usize> {
            let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
            s[..end].parse().ok()
        };
        Some((digits(chapter)?, digits(verse)?))
    }

    pub fn is_approved(&self) -> bool {
        self.approved_hash.is_some()
    }
}

/// One `Interlinear_{language}_{book}.xml`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct InterlinearBook {
    /// `GlossLanguage`: a language tag, or a language name in older files.
    pub gloss_language: Option<String>,
    /// `BookId` as written.
    pub book_id: Option<String>,
    /// `ScrTextName`, the project the glosses were made in.
    pub project: Option<String>,
    /// In file order, a repeated reference keeping its last occurrence at
    /// its first position.
    pub verses: Vec<InterlinearVerse>,
}

impl InterlinearBook {
    pub fn from_xml(xml: &str) -> Result<Self, String> {
        let document = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
        let root = document.root_element();
        if !root.has_tag_name("InterlinearData") {
            return Err(format!(
                "expected <InterlinearData>, found <{}>",
                root.tag_name().name()
            ));
        }
        let attribute = |name: &str| root.attribute(name).map(str::to_string);
        let mut book = Self {
            gloss_language: attribute("GlossLanguage"),
            book_id: attribute("BookId"),
            project: attribute("ScrTextName"),
            verses: Vec::new(),
        };
        let items = root
            .children()
            .filter(|n| n.has_tag_name("Verses"))
            .flat_map(|verses| verses.children().filter(|n| n.has_tag_name("item")));
        for item in items {
            let reference = child(item, "string")
                .and_then(|n| n.text())
                .unwrap_or_default()
                .trim()
                .to_string();
            let verse = match child(item, "VerseData") {
                Some(data) => read_verse(reference, data)?,
                None => InterlinearVerse {
                    reference,
                    approved_hash: None,
                    clusters: Vec::new(),
                    punctuation: Vec::new(),
                },
            };
            match book
                .verses
                .iter_mut()
                .find(|v| v.reference == verse.reference)
            {
                Some(earlier) => *earlier = verse,
                None => book.verses.push(verse),
            }
        }
        Ok(book)
    }

    /// The book `BookId` names, if it is a book code.
    pub fn book(&self) -> Option<BookCode> {
        self.book_id.as_deref()?.trim().parse().ok()
    }

    /// The first verse keyed as chapter `chapter`, verse `verse`.
    pub fn verse(&self, chapter: usize, verse: usize) -> Option<&InterlinearVerse> {
        self.verses
            .iter()
            .find(|v| v.chapter_verse() == Some((chapter, verse)))
    }
}

fn read_verse(
    reference: String,
    data: roxmltree::Node<'_, '_>,
) -> Result<InterlinearVerse, String> {
    let mut verse = InterlinearVerse {
        approved_hash: data.attribute("Hash").map(str::to_string),
        reference,
        clusters: Vec::new(),
        punctuation: Vec::new(),
    };
    for node in data.children().filter(roxmltree::Node::is_element) {
        match node.tag_name().name() {
            "Cluster" => verse.clusters.push(Cluster {
                range: read_range(node, &verse.reference)?,
                lexemes: node
                    .children()
                    .filter(|n| n.has_tag_name("Lexeme"))
                    .map(|lexeme| ClusterLexeme {
                        id: lexeme.attribute("Id").map(str::to_string),
                        sense: lexeme
                            .attribute("GlossId")
                            .filter(|s| !s.is_empty())
                            .map(str::to_string),
                    })
                    .collect(),
                excluded: match child(node, "Excluded") {
                    Some(excluded) => {
                        read_bool(excluded.text().unwrap_or_default()).ok_or_else(|| {
                            format!("{}: <Excluded> is not true or false", verse.reference)
                        })?
                    }
                    None => false,
                },
            }),
            "Punctuation" => verse.punctuation.push(Punctuation {
                range: read_range(node, &verse.reference)?,
                before: child(node, "BeforeText").map(|n| n.text().unwrap_or_default().to_string()),
                after: child(node, "AfterText").map(|n| n.text().unwrap_or_default().to_string()),
            }),
            _ => {}
        }
    }
    Ok(verse)
}

fn read_range(node: roxmltree::Node<'_, '_>, reference: &str) -> Result<TextRange, String> {
    let Some(range) = child(node, "Range") else {
        return Ok(TextRange::default());
    };
    let number = |name: &str| -> Result<usize, String> {
        match range.attribute(name) {
            None => Ok(0),
            Some(value) => value
                .trim()
                .parse()
                .map_err(|_| format!("{reference}: Range {name}=\"{value}\" is not a number")),
        }
    };
    Ok(TextRange {
        index: number("Index")?,
        length: number("Length")?,
    })
}

pub(crate) fn child<'a, 'input>(
    node: roxmltree::Node<'a, 'input>,
    name: &str,
) -> Option<roxmltree::Node<'a, 'input>> {
    node.children().find(|n| n.has_tag_name(name))
}

/// .NET's `bool` as XML writes it.
fn read_bool(text: &str) -> Option<bool> {
    match text.trim() {
        t if t.eq_ignore_ascii_case("true") => Some(true),
        t if t.eq_ignore_ascii_case("false") => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexeme_ids_parse_as_paratext_reads_them() {
        let key = LexemeKey::parse("Word:a:2").unwrap();
        assert_eq!(
            (key.kind, key.form.as_str(), key.homograph),
            (LexemeType::Word, "a", 2)
        );
        let key = LexemeKey::parse("Phrase:dax fep").unwrap();
        assert_eq!((key.form.as_str(), key.homograph), ("dax fep", 1));
        // A form may hold a colon; only trailing digits are a homograph.
        assert_eq!(LexemeKey::parse("Word:a:b").unwrap().form, "a:b");
        assert_eq!(LexemeKey::parse("Future:x").unwrap().kind.name(), "Future");
        assert_eq!(LexemeKey::parse("brokenid"), None);
        assert_eq!(LexemeKey::parse(":x"), None);
        assert_eq!(
            LexemeKey::parse("Word:a:2").unwrap().to_string(),
            "Word:a:2"
        );
        assert_eq!(LexemeKey::parse("Word:a:1").unwrap().to_string(), "Word:a");
    }

    #[test]
    fn a_reference_gives_its_chapter_and_first_verse() {
        let verse = |reference: &str| InterlinearVerse {
            reference: reference.to_string(),
            approved_hash: None,
            clusters: Vec::new(),
            punctuation: Vec::new(),
        };
        assert_eq!(verse("PHP 1:1").chapter_verse(), Some((1, 1)));
        assert_eq!(verse("GEN 12:3-4").chapter_verse(), Some((12, 3)));
        assert_eq!(verse("GEN 12:3a").chapter_verse(), Some((12, 3)));
        assert_eq!(verse("GEN").chapter_verse(), None);
    }
}
