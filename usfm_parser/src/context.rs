use std::collections::HashMap;

use crate::ast::{BookCode, Caller, NumberList, StyleId};
use usfm_style::{StyleRule, StyleSheet};

pub struct Context<'a> {
    pub book_code: Option<BookCode>,
    pub chapter_number: Option<usize>,
    pub verse_number: Option<NumberList>,
    pub style_sheet: &'a StyleSheet,
    pub note_counter: usize,
    pub custom_note_counter: usize,
    pub metadata: HashMap<String, String>,
    pub custom_counters: HashMap<&'a str, usize>,
}

impl<'a> Context<'a> {
    pub fn new(style_sheet: &'a StyleSheet) -> Self {
        Self {
            book_code: None,
            chapter_number: None,
            verse_number: None,
            style_sheet,
            note_counter: 0,
            custom_note_counter: 0,
            metadata: HashMap::new(),
            custom_counters: HashMap::new(),
        }
    }

    pub fn from_book(book_code: BookCode, style_sheet: &'a StyleSheet) -> Self {
        Self {
            book_code: Some(book_code),
            chapter_number: None,
            verse_number: None,
            style_sheet,
            note_counter: 0,
            custom_note_counter: 0,
            metadata: HashMap::new(),
            custom_counters: HashMap::new(),
        }
    }

    /// Resolve a node's style against the stylesheet this context was built
    /// from, which must be the one its document owns (hardening plan D3) —
    /// construct with `Context::new(document.style_sheet())`.
    pub fn rule(&self, style: StyleId) -> &'a StyleRule {
        self.style_sheet.get_rule(style.index())
    }

    /// The marker name for a node's style, e.g. `"p"`.
    pub fn marker(&self, style: StyleId) -> &'a str {
        &self.rule(style).marker
    }

    pub fn get_and_increment_note_number(&mut self, caller: &Caller) -> usize {
        match caller {
            Caller::Plus => {
                self.note_counter += 1;
                self.note_counter
            }
            _ => {
                self.custom_note_counter += 1;
                self.custom_note_counter
            }
        }
    }

    pub fn next_counter(&mut self, key: &'a str) -> usize {
        *self
            .custom_counters
            .entry(key)
            .and_modify(|e| {
                *e += 1;
            })
            .or_insert(0)
    }
}
