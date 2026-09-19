use core::fmt;
use std::fmt::Display;

use crate::{parse_error::ParseError, string_parser::{NoMatch, ParseStr, StringParser}};

#[derive(Debug, PartialEq, Clone)]
pub struct NumberRange {
    pub start: usize,
    pub start_modifier: Option<char>,
    pub end: usize,
    pub end_modifier: Option<char>,
    pub guard_rtl: bool,
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
        if !self.is_collapsed() {
            if self.guard_rtl {
                write!(f, "\u{200F}")?;
            }
            write!(f, "-")?;
            write!(f, "{}", self.end)?;
            if let Some(end_modifier) = self.end_modifier {
                write!(f, "{}", end_modifier)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct NumberList {
    ranges: Vec<NumberRange>,
    guard_rtl: bool,
}

impl ParseStr for NumberList {
    type Err = ParseError;

    fn consume_parser(parser: &mut StringParser) -> Result<Self, NoMatch> {
        let mut ranges = Vec::new();
        let mut guard_rtl_list = false;
        loop {
            let start = parser.expect_usize()?;
            let start_modifier = parser.maybe_expect_char(char::is_alphabetic);
            let has_guard = parser.eat_char('\u{200F}');
            let mut end = start;
            let mut end_modifier = None;
            let guard_rtl = if parser.eat_char('-') {
                end = parser.expect_usize()?;
                end_modifier = parser.maybe_expect_char(char::is_alphabetic);
                let parent_has_guard = parser.eat_char('\u{200F}');
                guard_rtl_list = guard_rtl_list || parent_has_guard;
                has_guard
            } else {
                guard_rtl_list = guard_rtl_list || has_guard;
                false
            };
            ranges.push(NumberRange { start, start_modifier, end, end_modifier, guard_rtl });
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

        Ok(Self { ranges, guard_rtl: guard_rtl_list })
    }

    fn create_err() -> Self::Err {
        ParseError::MalformedVerseNumber
    }
}

impl Display for NumberList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.ranges[0])?;
        if self.ranges.len() > 1 {
            let separator = if self.guard_rtl {
                "\u{200F},"
            } else {
                ","
            };
            for range in &self.ranges[1..] {
                write!(f, "{}{}", separator, range)?;
            }
        }
        Ok(())
    }
}

impl NumberList {
    pub fn collapsed(number: usize) -> Self {
        Self {
            ranges: vec![NumberRange { start: number, start_modifier: None, end: number, end_modifier: None, guard_rtl: false }],
            guard_rtl: false,
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
            ranges: vec![NumberRange { start: 1, start_modifier: None, end: 1, end_modifier: None, guard_rtl: false }],
            guard_rtl: false,
        });
    }

    #[test]
    fn test_verse_number_range() {
        let verse_number = NumberList::parse_str("1-3").unwrap();
        assert!(!verse_number.is_collapsed());
        assert_eq!(verse_number, NumberList {
            ranges: vec![NumberRange { start: 1, start_modifier: None, end: 3, end_modifier: None, guard_rtl: false }],
            guard_rtl: false,
        });
    }

    #[test]
    fn test_verse_number_range_with_modifier() {
        let verse_number = NumberList::parse_str("1a-3b").unwrap();
        assert!(!verse_number.is_collapsed());
        assert_eq!(verse_number, NumberList {
            ranges: vec![NumberRange { start: 1, start_modifier: Some('a'), end: 3, end_modifier: Some('b'), guard_rtl: false }],
            guard_rtl: false,
        });
    }

    #[test]
    fn test_verse_number_range_with_guard_rtl() {
        let verse_number = NumberList::parse_str("1\u{200F}-3").unwrap();
        assert!(!verse_number.is_collapsed());
        assert_eq!(verse_number, NumberList {
            ranges: vec![NumberRange { start: 1, start_modifier: None, end: 3, end_modifier: None, guard_rtl: true }],
            guard_rtl: false,
        });
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
}
