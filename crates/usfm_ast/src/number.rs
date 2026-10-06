use core::fmt;
use std::fmt::Display;

use crate::{parse_error::ParseError, string_parser::{NoMatch, ParseStr, StringParser}};

/// Whether `c` is a direction mark a number may carry: U+200F RIGHT-TO-LEFT
/// MARK or U+200E LEFT-TO-RIGHT MARK. A right-to-left project writes one
/// after a number to keep the punctuation beside it in reading order
/// (`1\u{200F}-3`, which Paratext writes for a verse bridge, and
/// `4\u{200F}`). They are invisible but they are what the author wrote, so
/// the number keeps each one where it stood.
pub fn is_direction_mark(c: char) -> bool {
    matches!(c, '\u{200E}' | '\u{200F}')
}

#[derive(Debug, PartialEq, Clone)]
pub struct NumberRange {
    pub start: usize,
    pub start_modifier: Option<char>,
    pub end: usize,
    pub end_modifier: Option<char>,
    /// The direction mark between the start and the `-`: `1\u{200F}-3`.
    pub guard: Option<char>,
    /// The direction mark after the range, before the `,` or the end of the
    /// number: `4\u{200F}`, `1-3\u{200F},5`.
    pub trailing_guard: Option<char>,
}

impl NumberRange {
    pub fn is_collapsed(&self) -> bool {
        self.start == self.end
    }
}

impl Display for NumberRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.start)?;
        if let Some(start_modifier) = self.start_modifier {
            write!(f, "{}", start_modifier)?;
        }
        // A range whose ends are the same number is written as that number,
        // since `4-4` and `4` are the same verse — but only when there is
        // nothing else in the range to write. An `end_modifier` has nowhere
        // else to go: `4-4t` written as `4` loses the `t`, and reads back as a
        // different verse; and neither has a `guard`, which written without
        // its `-` reads back as the `trailing_guard`. (`is_collapsed` is left
        // alone: `4-4t` *is* one verse, and `usfm_semantic` asks that
        // question, not this one.)
        if !self.is_collapsed() || self.end_modifier.is_some() || self.guard.is_some() {
            if let Some(guard) = self.guard {
                write!(f, "{}", guard)?;
            }
            write!(f, "-")?;
            write!(f, "{}", self.end)?;
            if let Some(end_modifier) = self.end_modifier {
                write!(f, "{}", end_modifier)?;
            }
        }
        if let Some(trailing_guard) = self.trailing_guard {
            write!(f, "{}", trailing_guard)?;
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct NumberList {
    ranges: Vec<NumberRange>,
}

impl ParseStr for NumberList {
    type Err = ParseError;

    fn consume_parser(parser: &mut StringParser) -> Result<Self, NoMatch> {
        let mut ranges = Vec::new();
        loop {
            let start = parser.expect_usize()?;
            let start_modifier = parser.maybe_expect_char(char::is_alphabetic);
            // Whose mark this is depends on what follows it: the range's
            // `guard` before a `-`, the `trailing_guard` otherwise.
            let mark = parser.maybe_expect_char(is_direction_mark);
            let range = if parser.eat_char('-') {
                let end = parser.expect_usize()?;
                let end_modifier = parser.maybe_expect_char(char::is_alphabetic);
                let trailing_guard = parser.maybe_expect_char(is_direction_mark);
                NumberRange { start, start_modifier, end, end_modifier, guard: mark, trailing_guard }
            } else {
                NumberRange { start, start_modifier, end: start, end_modifier: None, guard: None, trailing_guard: mark }
            };
            ranges.push(range);
            if parser.eat_char(',') {
                continue;
            } else {
                if parser.has_remaining() {
                    return Err(NoMatch);
                } else {
                    break;
                }
            }
        }

        if ranges.is_empty() {
            return Err(NoMatch);
        }

        Ok(Self { ranges })
    }

    fn create_err() -> Self::Err {
        ParseError::MalformedVerseNumber
    }
}

impl Display for NumberList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.ranges[0])?;
        for range in &self.ranges[1..] {
            write!(f, ",{}", range)?;
        }
        Ok(())
    }
}

impl NumberList {
    pub fn collapsed(number: usize) -> Self {
        Self {
            ranges: vec![NumberRange { start: number, start_modifier: None, end: number, end_modifier: None, guard: None, trailing_guard: None }],
        }
    }

    pub fn first_range(&self) -> &NumberRange {
        &self.ranges[0]
    }

    pub fn last_range(&self) -> &NumberRange {
        &self.ranges[self.ranges.len() - 1]
    }

    pub fn start(&self) -> usize {
        self.first_range().start
    }

    pub fn end(&self) -> usize {
        self.last_range().end
    }

    pub fn is_collapsed(&self) -> bool {
        self.ranges.len() == 1 && self.ranges[0].is_collapsed()
    }

    /// The ranges in source order: `1,3-5` is two.
    pub fn ranges(&self) -> &[NumberRange] {
        &self.ranges
    }

    /// Whether `number` falls in any of the ranges: `3-5` contains 4.
    /// Modifiers are ignored, so `1a-1b` contains 1.
    pub fn contains(&self, number: usize) -> bool {
        self.ranges
            .iter()
            .any(|range| range.start <= number && number <= range.end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verse_number_simple() {
        let verse_number = NumberList::parse_str("1").unwrap();
        assert!(verse_number.is_collapsed());
        assert_eq!(verse_number, NumberList {
            ranges: vec![NumberRange { start: 1, start_modifier: None, end: 1, end_modifier: None, guard: None, trailing_guard: None }],
        });
    }

    #[test]
    fn test_verse_number_range() {
        let verse_number = NumberList::parse_str("1-3").unwrap();
        assert!(!verse_number.is_collapsed());
        assert_eq!(verse_number, NumberList {
            ranges: vec![NumberRange { start: 1, start_modifier: None, end: 3, end_modifier: None, guard: None, trailing_guard: None }],
        });
    }

    #[test]
    fn test_verse_number_range_with_modifier() {
        let verse_number = NumberList::parse_str("1a-3b").unwrap();
        assert!(!verse_number.is_collapsed());
        assert_eq!(verse_number, NumberList {
            ranges: vec![NumberRange { start: 1, start_modifier: Some('a'), end: 3, end_modifier: Some('b'), guard: None, trailing_guard: None }],
        });
    }

    #[test]
    fn test_verse_number_range_with_guard_rtl() {
        let verse_number = NumberList::parse_str("1\u{200F}-3").unwrap();
        assert!(!verse_number.is_collapsed());
        assert_eq!(verse_number, NumberList {
            ranges: vec![NumberRange { start: 1, start_modifier: None, end: 3, end_modifier: None, guard: Some('\u{200F}'), trailing_guard: None }],
        });
    }

    /// A range whose ends are the same number is written as that number —
    /// unless the end carries a modifier of its own, which is then the only
    /// place it can be written. `\v 4-4t` is real (the round-trip fuzz target
    /// found it, ticket 27) and used to come back as `\v 4`.
    #[test]
    fn a_collapsed_range_keeps_an_end_modifier() {
        for number in ["4", "4a", "4-4t", "4a-4b", "4-5"] {
            assert_eq!(
                NumberList::parse_str(number).unwrap().to_string(),
                number,
                "{number}"
            );
        }
    }

    #[test]
    fn test_verse_number_display() {
        let verse_number = NumberList::parse_str("1,3\u{200F}-5,7").unwrap();
        assert_eq!(verse_number.to_string(), "1,3\u{200F}-5,7");
    }

    #[test]
    fn test_verse_number_display_guarded() {
        let verse_number = NumberList::parse_str("1\u{200F},3\u{200F}-5\u{200F},7").unwrap();
        assert_eq!(verse_number.to_string(), "1\u{200F},3\u{200F}-5\u{200F},7");
    }

    #[test]
    fn test_collapsed_verse_number() {
        let verse_number = NumberList::collapsed(1);
        assert_eq!(verse_number.to_string(), "1");
    }

    #[test]
    fn test_not_collapsed_verse_number() {
        let verse_number = NumberList::parse_str("1,3-5").unwrap();
        assert!(!verse_number.is_collapsed());
    }

    /// A direction mark stays where it was written, whichever of the two it
    /// is. `4\u{200F}` is real — fifteen verses of the Paratext projects the
    /// toolchain was run over on 2026-10-06 — and came back as `4`: the mark
    /// was remembered only as "write one before each comma", and a single
    /// number has no comma.
    #[test]
    fn a_direction_mark_is_written_where_it_was_read() {
        for mark in ['\u{200F}', '\u{200E}'] {
            for number in ["4M", "4aM", "1M-3", "1-3M", "1M-3M", "4M-4", "1,3M", "1M,3", "1M,3M-5M,7M"] {
                let number = number.replace('M', &mark.to_string());
                assert_eq!(
                    NumberList::parse_str(&number).unwrap().to_string(),
                    number,
                    "{number:?}"
                );
            }
        }
    }

    #[test]
    fn a_trailing_direction_mark_is_the_ranges_own() {
        let number = NumberList::parse_str("4\u{200F}").unwrap();
        assert!(number.is_collapsed());
        assert_eq!(number.first_range().guard, None);
        assert_eq!(number.first_range().trailing_guard, Some('\u{200F}'));
        assert_ne!(number, NumberList::parse_str("4").unwrap());
    }

    /// One mark in each place, and nowhere else: not before the number, not
    /// twice, and no other invisible character.
    #[test]
    fn a_direction_mark_elsewhere_is_malformed() {
        for number in ["\u{200F}4", "4\u{200F}\u{200F}", "4\u{200D}", "1-\u{200F}3"] {
            assert!(NumberList::parse_str(number).is_err(), "{number:?}");
        }
    }
}
