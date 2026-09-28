//! Where an interlinear verse's clusters stand in the verse's text (ticket 56).
//!
//! A cluster's range counts characters of Paratext's own string for the
//! verse, which is not the text a `Document` holds (it carries the verse
//! marker, and Paratext never updates a range when the text is edited). What
//! Paratext itself trusts is the **form**: it matches an analysis to a word by
//! the lexeme's form. So [`anchor`] does the same, following SIL's
//! interlinearizer (`clusterAnchoring.ts`, MIT):
//!
//! - the text is divided into words ([`words`]);
//! - word and parse clusters with the same range are one word's two
//!   analyses and land together; each lands on the first word at or after
//!   the previous one whose text, lower-cased, is the cluster's surface
//!   ([`Cluster::surface`]: a parse's morphemes joined);
//! - a phrase lands on the first run of consecutive words that spells its
//!   form, at or after the previous phrase's first word (so phrases may
//!   overlap, as `zim zam` and `zam zim` do over `zim zam zim`);
//! - where several words or runs match, the range chooses and the anchor is
//!   marked ambiguous: never a place a later cluster of the same form needs
//!   (so `γαρ γαρ` takes one each), and otherwise the one nearest the range,
//!   counted in UTF-16 as Paratext counts. Nearest is by the offset most
//!   uniquely spelled words agree the ranges are shifted by (the verse
//!   marker, in a real project, when the verse is unedited), or, when no
//!   word settles it, by proportion;
//! - a cluster that lands nowhere, or has no place to land, is dropped with
//!   its reason rather than lost.
//!
//! The text to pass is the verse's plain text with notes left out,
//! `usfm_semantic::ReferenceIndex`'s `VerseRef::text()`: glosses are made
//! over the verse's own words, not its footnotes'.
//!
//! Words are compared lower-cased and not Unicode-normalised: a project
//! whose text and lexicon disagree on NFC and NFD will not match.

use std::ops::Range;

use crate::interlinear::{Cluster, ClusterKind, TextRange};

/// Why a cluster has no place in the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropReason {
    /// No word, or run of words, spells the cluster's form.
    FormMismatch,
    /// A second word, or a second parse, over the same range as an earlier
    /// one. Paratext keeps one of each per range, so the file is corrupt
    /// here; the first is kept.
    Duplicate,
    /// A `Lemma`, `Infix` or empty cluster, which has no surface.
    NoSurface,
    /// A lexeme id that is missing or not `Type:Form[:Homograph]`.
    Unparseable,
}

/// A word of the text and the clusters that analyse it.
#[derive(Debug, Clone, PartialEq)]
pub struct AnchoredWord {
    /// The word, an index into [`Anchoring::words`].
    pub word: usize,
    /// The `Word` cluster, an index into the verse's clusters.
    pub gloss: Option<usize>,
    /// The `Parse` cluster (stem and affixes).
    pub parse: Option<usize>,
    /// Several words matched and the range chose between them.
    pub ambiguous: bool,
}

/// A run of words and the phrase cluster that glosses it.
#[derive(Debug, Clone, PartialEq)]
pub struct AnchoredPhrase {
    /// The words, indices into [`Anchoring::words`].
    pub words: Range<usize>,
    pub cluster: usize,
    pub ambiguous: bool,
}

/// One verse's clusters, placed.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Anchoring {
    /// The text's words, as byte ranges of it.
    pub words: Vec<Range<usize>>,
    /// In text order.
    pub anchored_words: Vec<AnchoredWord>,
    /// In the order they were placed (by range).
    pub phrases: Vec<AnchoredPhrase>,
    /// Cluster index and why, in cluster order.
    pub dropped: Vec<(usize, DropReason)>,
}

/// The byte ranges of the words of `text`: runs of letters, digits,
/// combining marks and the zero-width joiners (which Urdu writes inside
/// words). Everything else, punctuation included, separates words.
pub fn words(text: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut start = None;
    for (at, c) in text.char_indices() {
        match (is_word_char(c), start) {
            (true, None) => start = Some(at),
            (false, Some(s)) => {
                out.push(s..at);
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push(s..text.len());
    }
    out
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || is_mark(c) || matches!(c, '\u{200C}' | '\u{200D}')
}

/// The combining marks of the scripts at stake — Latin, Greek, Hebrew
/// (points and cantillation), Arabic and Urdu — and the general combining
/// blocks. Not the whole of Unicode's `M` category.
fn is_mark(c: char) -> bool {
    matches!(c,
        '\u{0300}'..='\u{036F}'
        | '\u{0483}'..='\u{0489}'
        | '\u{0591}'..='\u{05BD}'
        | '\u{05BF}'
        | '\u{05C1}'..='\u{05C2}'
        | '\u{05C4}'..='\u{05C5}'
        | '\u{05C7}'
        | '\u{0610}'..='\u{061A}'
        | '\u{064B}'..='\u{065F}'
        | '\u{0670}'
        | '\u{06D6}'..='\u{06DC}'
        | '\u{06DF}'..='\u{06E4}'
        | '\u{06E7}'..='\u{06E8}'
        | '\u{06EA}'..='\u{06ED}'
        | '\u{08D3}'..='\u{08FF}'
        | '\u{1AB0}'..='\u{1AFF}'
        | '\u{1DC0}'..='\u{1DFF}'
        | '\u{20D0}'..='\u{20FF}'
        | '\u{FE20}'..='\u{FE2F}'
    )
}

fn fold(s: &str) -> String {
    s.to_lowercase()
}

/// Place `clusters` in `text`; see the module documentation.
pub fn anchor(text: &str, clusters: &[Cluster]) -> Anchoring {
    let words = words(text);
    let folded: Vec<String> = words.iter().map(|w| fold(&text[w.clone()])).collect();
    // Where each word starts, in UTF-16 code units: what a range counts.
    let starts: Vec<usize> = words
        .iter()
        .map(|w| text[..w.start].encode_utf16().count())
        .collect();
    let text_len = text.encode_utf16().count().max(1);
    let extent = clusters
        .iter()
        .map(|c| c.range.index + c.range.length)
        .max()
        .unwrap_or(0)
        .max(1);
    let shift = shift(clusters, &folded, &starts);
    // Among several candidates, the one nearest the range: by the offset
    // the text is shifted by when one is known, proportionally otherwise.
    let nearest = |candidates: &[usize], range: TextRange| -> usize {
        let distance = |w: usize| match shift {
            Some(shift) => (starts[w] as f64 + shift as f64 - range.index as f64).abs(),
            None => (starts[w] as f64 / text_len as f64 - range.index as f64 / extent as f64).abs(),
        };
        *candidates
            .iter()
            .min_by(|&&a, &&b| distance(a).total_cmp(&distance(b)))
            .expect("candidates is not empty")
    };

    let mut anchoring = Anchoring::default();
    // Word and parse clusters grouped by range: (range, word, parse).
    let mut groups: Vec<(TextRange, Option<usize>, Option<usize>)> = Vec::new();
    let mut phrases = Vec::new();
    for (index, cluster) in clusters.iter().enumerate() {
        let kind = cluster.kind();
        match kind {
            ClusterKind::Word | ClusterKind::Parse => {
                let group = match groups.iter_mut().position(|g| g.0 == cluster.range) {
                    Some(at) => &mut groups[at],
                    None => {
                        groups.push((cluster.range, None, None));
                        groups.last_mut().expect("just pushed")
                    }
                };
                let slot = if kind == ClusterKind::Word {
                    &mut group.1
                } else {
                    &mut group.2
                };
                if slot.is_some() {
                    anchoring.dropped.push((index, DropReason::Duplicate));
                } else {
                    *slot = Some(index);
                }
            }
            ClusterKind::Phrase => phrases.push(index),
            ClusterKind::Other => anchoring.dropped.push((index, DropReason::NoSurface)),
            ClusterKind::Unparseable => anchoring.dropped.push((index, DropReason::Unparseable)),
        }
    }

    groups.sort_by_key(|g| (g.0.index, g.0.length));
    let surfaces: Vec<String> = groups
        .iter()
        .map(|&(_, gloss, parse)| {
            let first = gloss.or(parse).expect("a group has a cluster");
            fold(&clusters[first].surface().unwrap_or_default())
        })
        .collect();
    let mut cursor = 0;
    for (at, (range, gloss, parse)) in groups.into_iter().enumerate() {
        let expected = &surfaces[at];
        let mut candidates: Vec<usize> = (cursor..words.len())
            .filter(|&w| folded[w] == *expected)
            .collect();
        if candidates.is_empty() {
            for cluster in [gloss, parse].into_iter().flatten() {
                anchoring.dropped.push((cluster, DropReason::FormMismatch));
            }
            continue;
        }
        let ambiguous = candidates.len() > 1;
        reserve(
            &mut candidates,
            surfaces[at + 1..].iter().filter(|s| *s == expected).count(),
        );
        let word = if candidates.len() > 1 {
            nearest(&candidates, range)
        } else {
            candidates[0]
        };
        anchoring.anchored_words.push(AnchoredWord {
            word,
            gloss,
            parse,
            ambiguous,
        });
        cursor = word + 1;
    }

    phrases.sort_by_key(|&p| clusters[p].range.index);
    let forms: Vec<String> = phrases
        .iter()
        .map(|&p| fold(&clusters[p].surface().unwrap_or_default()))
        .collect();
    let mut cursor = 0;
    for (at, &cluster) in phrases.iter().enumerate() {
        let form = &forms[at];
        let parts: Vec<&str> = form.split_whitespace().collect();
        let mut runs: Vec<usize> = if parts.is_empty() {
            Vec::new()
        } else {
            (cursor..(words.len() + 1).saturating_sub(parts.len()))
                .filter(|&s| parts.iter().enumerate().all(|(i, p)| folded[s + i] == *p))
                .collect()
        };
        if runs.is_empty() {
            anchoring.dropped.push((cluster, DropReason::FormMismatch));
            continue;
        }
        let ambiguous = runs.len() > 1;
        reserve(
            &mut runs,
            forms[at + 1..].iter().filter(|f| *f == form).count(),
        );
        let start = if runs.len() > 1 {
            nearest(&runs, clusters[cluster].range)
        } else {
            runs[0]
        };
        anchoring.phrases.push(AnchoredPhrase {
            words: start..start + parts.len(),
            cluster,
            ambiguous,
        });
        cursor = start + 1;
    }

    anchoring.anchored_words.sort_by_key(|w| w.word);
    anchoring.dropped.sort_by_key(|&(cluster, _)| cluster);
    anchoring.words = words;
    anchoring
}

/// The offset between the text and the ranges, when the clusters agree on
/// one: for each word cluster whose form is written once in the text, its
/// range's start less the word's, and the value most of them give. Paratext
/// counts its own verse string, which begins with the verse marker (`\v 7 `),
/// so over an unedited verse this is that marker's length and every range
/// is exact; an edit shifts only the words after it, which the majority
/// outvotes.
fn shift(clusters: &[Cluster], folded: &[String], starts: &[usize]) -> Option<i64> {
    let mut votes: Vec<(i64, usize)> = Vec::new();
    for cluster in clusters {
        if cluster.kind() != ClusterKind::Word {
            continue;
        }
        let surface = fold(&cluster.surface().unwrap_or_default());
        let mut matches = folded.iter().enumerate().filter(|(_, w)| **w == surface);
        let (Some((word, _)), None) = (matches.next(), matches.next()) else {
            continue;
        };
        let delta = cluster.range.index as i64 - starts[word] as i64;
        match votes.iter_mut().find(|(d, _)| *d == delta) {
            Some(vote) => vote.1 += 1,
            None => votes.push((delta, 1)),
        }
    }
    // Most votes; among equals, the smallest offset.
    votes
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
        .map(|(delta, _)| delta)
}

/// Keep, of the matching places, only those that leave one for each later
/// cluster of the same form, so a repeated word (`γαρ γαρ`) is not taken
/// by the first cluster and lost to the second.
fn reserve(candidates: &mut Vec<usize>, later: usize) {
    let keep = candidates.len().saturating_sub(later).max(1);
    candidates.truncate(keep);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spelled(text: &str) -> Vec<&str> {
        words(text).into_iter().map(|w| &text[w]).collect()
    }

    #[test]
    fn words_keep_their_marks_and_joiners() {
        assert_eq!(spelled("Wug blicket, dax."), ["Wug", "blicket", "dax"]);
        // Greek with an accent, Hebrew with points and a maqaf between words,
        // Urdu with a zero-width non-joiner inside a word.
        assert_eq!(spelled("λόγος"), ["λόγος"]);
        assert_eq!(spelled("כָּל־הָאָרֶץ"), ["כָּל", "הָאָרֶץ"]);
        assert_eq!(spelled("کی\u{200C}ا خدا۔"), ["کی\u{200C}ا", "خدا"]);
    }

    fn word(index: usize, form: &str) -> Cluster {
        Cluster {
            range: TextRange {
                index,
                length: form.encode_utf16().count(),
            },
            lexemes: vec![crate::interlinear::ClusterLexeme {
                id: Some(format!("Word:{form}")),
                sense: None,
            }],
            excluded: false,
        }
    }

    fn landed(text: &str, clusters: &[Cluster]) -> Vec<(usize, usize)> {
        let anchoring = anchor(text, clusters);
        assert!(anchoring.dropped.is_empty(), "{:?}", anchoring.dropped);
        let mut out: Vec<_> = anchoring
            .anchored_words
            .iter()
            .map(|w| (w.gloss.unwrap(), w.word))
            .collect();
        out.sort();
        out
    }

    /// 3JN 1:7 of a real Paratext project (Sinaiticus): the ranges count
    /// `\v 7 ` too, and the clusters come in no particular order. The first
    /// `γαρ` had taken the second word, as proportionally nearer, and the
    /// second `γαρ` was dropped.
    #[test]
    fn a_repeated_word_takes_one_each_by_the_markers_offset() {
        let text = "ϋπερ γαρ γαρ του ονοματοϲ";
        let clusters = [
            word(14, "γαρ"),
            word(10, "γαρ"),
            word(22, "ονοματοϲ"),
            word(5, "ϋπερ"),
            word(18, "του"),
        ];
        assert_eq!(
            landed(text, &clusters),
            [(0, 2), (1, 1), (2, 4), (3, 0), (4, 3)]
        );
        // A word written in since the glosses were made shifts the words
        // after it; the ranges still pick their own `γαρ`.
        let edited = "ϋπερ λογοϲ γαρ γαρ του ονοματοϲ";
        assert_eq!(
            landed(edited, &clusters),
            [(0, 3), (1, 2), (2, 5), (3, 0), (4, 4)]
        );
    }
}
