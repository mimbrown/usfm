use regex::{self, Replacer};
use regex_automata::{Input, meta::Regex};
use std::borrow::Cow;
use std::sync::{Arc, LazyLock};
use usfm_style::{StyleRule, StyleSheet};

use crate::ast::visit_mut::{VisitMut, walk_char, walk_document, walk_note, walk_para};
use crate::ast::{Char, Document, Note, Para, StyleId, Text};

static MATCH_UNICODE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"\\u([0-9a-fA-F]{4})").unwrap());

#[derive(Debug)]
enum Expansion {
    NeedsCapture(String),
    NoExpansion(String),
}

pub struct TextReplacement {
    replacements: Vec<(Regex, Expansion)>,
    /// The sheet of the document being rewritten, captured at the start of
    /// the walk (plan D3): only vernacular text is replaced.
    style_sheet: Option<Arc<StyleSheet>>,
}

// Hand-written: the stylesheet is not `Debug`, and would be noise anyway.
impl std::fmt::Debug for TextReplacement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextReplacement")
            .field("replacements", &self.replacements)
            .finish_non_exhaustive()
    }
}

impl Default for TextReplacement {
    fn default() -> Self {
        Self::new()
    }
}

impl TextReplacement {
    pub fn new() -> Self {
        Self {
            replacements: Vec::new(),
            style_sheet: None,
        }
    }

    /// Rewrite every vernacular text run in `document`.
    pub fn apply_to(&mut self, document: &mut Document<'_>) {
        self.visit_document(document);
    }

    fn rule(&self, style: StyleId) -> &StyleRule {
        self.style_sheet
            .as_ref()
            .expect("the stylesheet is captured in visit_document")
            .get_rule(style.index())
    }

    fn parse_quote_delimited(text: &str) -> Option<(String, &str)> {
        let delimiter = match text.as_bytes().first() {
            Some(b'"') => '"',
            Some(b'\'') => '\'',
            _ => return None,
        };

        let text = &text[1..];
        let mut trimmed_text = text;
        let mut escape_next = false;
        while let Some(ch) = trimmed_text.chars().next() {
            if escape_next {
                escape_next = false;
                trimmed_text = &trimmed_text[ch.len_utf8()..];
                continue;
            }
            if ch == delimiter {
                let result = text[..text.len() - trimmed_text.len()].to_string();
                trimmed_text = &trimmed_text[ch.len_utf8()..];
                return Some((result, trimmed_text));
            }
            trimmed_text = &trimmed_text[ch.len_utf8()..];
            if ch == '\\' {
                escape_next = true;
            }
        }
        None
    }

    /// Parse a replacement rule from a line of text
    /// Format: "search" > "replace"
    /// The search pattern can be a regex pattern
    pub fn parse_rule(line: &str) -> Option<(Regex, String)> {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            return None;
        }

        let (matcher, line) = Self::parse_quote_delimited(line)?;
        let line = line.trim_start();
        let line = line.strip_prefix('>')?;
        let line = line.trim_start();
        let (replacement, _) = Self::parse_quote_delimited(line)?;
        let replacement = MATCH_UNICODE
            .replace_all(&replacement, |cap: &regex::Captures<'_>| {
                let code = cap.get(1).unwrap();
                let code = u32::from_str_radix(code.as_str(), 16).unwrap();
                char::from_u32(code).unwrap().to_string()
            })
            .to_string();
        Some((Regex::new(&matcher).ok()?, replacement))
    }

    pub fn apply(&self, text: &str) -> String {
        let mut replaced = String::with_capacity(text.len());
        let mut index = 0;
        'loop_text: while index < text.len() {
            for (regex, expansion) in self.replacements.iter() {
                let input = Input::new(text)
                    .span(index..text.len())
                    .anchored(regex_automata::Anchored::Yes);
                match expansion {
                    // If we know that the replacement doesn't have any capture expansions,
                    // then we can use the fast path. The fast path can make a tremendous
                    // difference:
                    //
                    //   1) We use `find_iter` instead of `captures_iter`. Not asking for
                    //      captures generally makes the regex engines faster.
                    //   2) We don't need to look up all of the capture groups and do
                    //      replacements inside the replacement string. We just push it
                    //      at each match and be done with it.
                    Expansion::NoExpansion(rep) => {
                        if let Some(found) = regex.search(&input) {
                            replaced.push_str(rep);
                            index = found.end();
                            continue 'loop_text;
                        }
                    }
                    Expansion::NeedsCapture(rep) => {
                        // The slower path, which we use if the replacement may need access to
                        // capture groups.
                        let mut caps = regex.create_captures();
                        regex.search_captures(&input, &mut caps);
                        if caps.is_match() {
                            // unwrap on 0 is OK because captures only reports matches
                            let m = caps.get_group(0).unwrap();
                            // TODO: Prepare the replacement string for capture groups
                            caps.interpolate_string_into(text, rep, &mut replaced);
                            // rep.clone().replace_append(&caps, &mut replaced);
                            index = m.end;
                            continue 'loop_text;
                        }
                    }
                }
            }
            // At this point, we failed to match any and need to advance a char
            // Not `unsafe`, so not a `// SAFETY:` note: the `unwrap` cannot fire
            // because the loop above already established `index < text.len()`.
            let unmatched_char = &text[index..].chars().next().unwrap();
            replaced.push(*unmatched_char);
            index += unmatched_char.len_utf8();
        }
        replaced
    }

    /// Load replacement rules from a string containing multiple lines
    pub fn from_rules(rules: &str) -> Self {
        let mut text_replacements = Self::new();

        for line in rules.lines() {
            if let Some((search, mut replacement)) = Self::parse_rule(line) {
                text_replacements.replacements.push((
                    search,
                    match replacement.no_expansion() {
                        Some(replacement) => Expansion::NoExpansion(replacement.to_string()),
                        None => Expansion::NeedsCapture(replacement.to_string()),
                    },
                ));
            }
        }

        text_replacements
    }
}

impl VisitMut for TextReplacement {
    fn visit_document(&mut self, document: &mut Document<'_>) {
        self.style_sheet = Some(Arc::clone(document.style_sheet()));
        walk_document(self, document);
    }

    fn visit_para(&mut self, para: &mut Para<'_>) {
        if self.rule(para.style).is_vernacular() {
            walk_para(self, para)
        }
    }

    fn visit_char(&mut self, char: &mut Char<'_>) {
        if self.rule(char.style).is_vernacular() {
            walk_char(self, char)
        }
    }

    fn visit_note(&mut self, note: &mut Note<'_>) {
        if self.rule(note.style).is_vernacular() {
            walk_note(self, note)
        }
    }

    fn visit_text(&mut self, text: &mut Text<'_>) {
        // A replacement rewrites the content; the span still points at the
        // source the text was read from, so it is left alone.
        text.content = Cow::Owned(self.apply(&text.content));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_quote_delimited() {
        assert_eq!(
            TextReplacement::parse_quote_delimited("\"Hello\""),
            Some(("Hello".to_string(), ""))
        );
        assert_eq!(
            TextReplacement::parse_quote_delimited("'Hello' >"),
            Some(("Hello".to_string(), " >"))
        );
        assert_eq!(TextReplacement::parse_quote_delimited("'Hello\" >"), None);
        assert_eq!(
            TextReplacement::parse_quote_delimited("'Hello \\'a' >"),
            Some(("Hello \\'a".to_string(), " >"))
        );
    }

    #[test]
    fn test_quote_replacements() {
        let rules = r#"
# CONVERT STANDARD ASCII QUOTES TO ENGLISH (CURLY) QUOTES
 "``"                  >    '\u201c'          # use double open  curly quotes
 "`"                   >    '\u2018'          # use single open  curly quotes
 '"'                   >    '\u201d'          # use double close curly quotes
 "''"                  >    '\u201d'          # use double close curly quotes
 "'"                   >    '\u2019'          # use double close curly quotes
"#;

        let replacement = TextReplacement::from_rules(rules);

        // Test some example replacements
        assert_eq!(replacement.apply("``Hello''"), "\u{201c}Hello\u{201d}");

        assert_eq!(replacement.apply("`Hello'"), "\u{2018}Hello\u{2019}");
    }

    #[test]
    fn test_regex_replacements() {
        let rules = r#"
# Example regex replacements
 "\s+"             >    " "              # collapse multiple spaces
 "\b(\d+)\s*st\b"  >    "$1"             # remove "st" suffix from numbers
"#;

        let replacement = TextReplacement::from_rules(rules);

        // Test space collapsing
        assert_eq!(replacement.apply("Hello   World"), "Hello World");

        // Test number suffix removal
        assert_eq!(replacement.apply("in 1st place"), "in 1 place");
    }

    #[test]
    fn test_complex_patterns() {
        let rules = r#"
# Test patterns with special characters
 ">"                >    "&gt;"           # HTML encode greater than
 "\""               >    "&quot;"          # HTML encode quotes
"#;

        let replacement = TextReplacement::from_rules(rules);

        // Test HTML encoding
        assert_eq!(replacement.apply("x > y"), "x &gt; y");

        // Test quote encoding
        assert_eq!(replacement.apply("\"Hello\""), "&quot;Hello&quot;");
    }
}
