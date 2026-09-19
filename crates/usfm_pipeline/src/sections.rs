//! Cutting vernacular text into sentence-sized sections.
//!
//! A diglot lines two translations up side by side, and the unit it lines up
//! is not a verse — verse boundaries move between translations — but a run of
//! text ending at punctuation. [`sections`] is that cut over one string, and
//! [`document_sections`] is it over a whole document's vernacular paragraphs.
//!
//! Moved out of `usfm_parser/src/main.rs` by ticket 15.

use std::borrow::Cow;
use std::sync::LazyLock;

use regex::Matches;
use usfm_ast::{Block, Document, Inline};
use usfm_style::StyleSheet;

/// The punctuation a section may end at: Latin, Arabic and the quotation marks
/// and brackets that surround a sentence rather than end one.
static MATCH_PUNCTUATION: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r#"[.?!,۔؟“”"'‘’،:;\(\)]+"#).unwrap());

/// The sections of one text run, as produced by [`sections`].
pub struct IterSections<'a> {
    cursor: usize,
    text: &'a str,
    matches: Matches<'a, 'a>,
}

impl<'a> IterSections<'a> {
    /// True if the punctuation at `cursor` opens a quotation rather than
    /// closing one, in which case the cut goes in front of it: `He said
    /// “Hello.”` is two sections, and the opening quote belongs to the second.
    fn is_opening_quote(&self, cursor: usize) -> bool {
        if !matches!(
            self.text[cursor..].chars().next(),
            Some('"' | '\'' | '”' | '“' | '‘' | '’' | '(')
        ) {
            return false;
        }
        let last_char = self.text[..cursor].chars().next_back();
        match last_char {
            Some(ch) => ch.is_whitespace(),
            None => true,
        }
    }
}

impl<'a> Iterator for IterSections<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        match self.matches.next() {
            Some(m) => {
                let cutoff = if self.is_opening_quote(m.start()) {
                    m.start()
                } else {
                    m.end()
                };
                let result = &self.text[self.cursor..cutoff];
                self.cursor = cutoff;
                Some(result)
            }
            None => {
                if self.cursor < self.text.len() {
                    let result = &self.text[self.cursor..];
                    self.cursor = self.text.len();
                    Some(result)
                } else {
                    None
                }
            }
        }
    }
}

/// Cut `text` at its punctuation. Every byte of `text` is in exactly one
/// section, in order, so the sections concatenate back to the input.
pub fn sections(text: &str) -> IterSections<'_> {
    IterSections {
        cursor: 0,
        text,
        matches: MATCH_PUNCTUATION.find_iter(text),
    }
}

/// The vernacular text of `document`, cut into sections and trimmed.
///
/// Non-vernacular paragraphs (headings, identification lines, anything the
/// stylesheet does not call vernacular) are skipped, and so is everything that
/// is not plain text: a section is the text a reader reads, which is what the
/// prompt format pairs across the two sides of a diglot.
///
/// `styles` is the document's own stylesheet (`document.style_sheet()`), not
/// the sheet handed to the parser: they differ whenever the parser had to
/// derive a style (hardening plan D3).
pub fn document_sections<'a>(document: &'a Document<'a>, styles: &StyleSheet) -> Vec<Cow<'a, str>> {
    let mut result = Vec::new();
    let mut add_if_non_empty = |text: Cow<'a, str>| {
        if !text.is_empty() && !text.chars().all(char::is_whitespace) {
            result.push(match text {
                Cow::Borrowed(text) => Cow::Borrowed(text.trim()),
                Cow::Owned(text) => Cow::Owned(text.trim().to_string()),
            });
        }
    };
    let mut last_text: Option<Cow<'a, str>> = None;
    for block in document.blocks.iter() {
        if let Block::Para(para) = block {
            if styles.get_rule(para.style.index()).is_nonvernacular() {
                continue;
            }
            for inline in para.children.iter() {
                if let Inline::Text(text) = inline {
                    let mut iter = sections(text);
                    if let Some(first_text) = iter.next() {
                        if let Some(last_text) = last_text.as_mut() {
                            last_text.to_mut().push_str(first_text);
                        } else {
                            last_text = Some(Cow::Borrowed(first_text));
                        }
                    }
                    for section in iter {
                        if let Some(last_text) = last_text.as_mut() {
                            add_if_non_empty(std::mem::replace(last_text, Cow::Borrowed(section)));
                        } else {
                            last_text = Some(Cow::Borrowed(section));
                        }
                    }
                }
            }
            if let Some(last_text) = last_text.as_mut() {
                add_if_non_empty(std::mem::take(last_text));
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::parse;

    /// A section ends *after* the punctuation that closes it, and the sections
    /// of a run concatenate back to the run.
    #[test]
    fn a_section_ends_at_its_punctuation() {
        let text = "One. Two! Three";
        assert_eq!(
            sections(text).collect::<Vec<_>>(),
            vec!["One.", " Two!", " Three"]
        );
        assert_eq!(sections(text).collect::<String>(), text);
    }

    /// An opening quotation mark belongs to the section it opens, so the cut
    /// goes in front of it rather than after it.
    #[test]
    fn an_opening_quote_starts_a_section() {
        assert_eq!(
            sections(r#"He said "Hello." Then left."#).collect::<Vec<_>>(),
            vec!["He said ", r#""Hello.""#, " Then left."]
        );
    }

    /// Only vernacular paragraphs are cut up: the chapter label `\cl` is one
    /// of the four paragraph markers the stylesheet does not call vernacular,
    /// and it is not part of the text a diglot lines up.
    #[test]
    fn only_vernacular_paragraphs_produce_sections() {
        let (document, style_sheet) =
            parse("\\id GEN\n\\c 1\n\\cl Chapter One.\n\\p \\v 1 One. Two.");
        assert_eq!(
            document_sections(&document, &style_sheet),
            vec!["One.", "Two."]
        );
    }
}
