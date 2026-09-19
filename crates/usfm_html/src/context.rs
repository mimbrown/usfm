//! The state a serializer carries while it walks a document.
//!
//! This lives here, and not in the parser or in a shared crate, because every
//! field is an HTML or SILE concern (ticket 14): the note counters are
//! footnote markers, `custom_counters` is the sequence behind a generated id,
//! and `metadata` carries `\cp`'s replacement chapter number from the `\c`
//! marker to the paragraph that prints it. The USX writer needs none of it and
//! keeps its own private state (ticket 13).

use std::collections::HashMap;

use usfm_ast::{BookCode, Caller, NumberList, StyleId};
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

    /// Resolve a node's style against the stylesheet this context was built
    /// from, which must be the one its document owns (hardening plan D3) —
    /// construct with `Context::new(document.style_sheet())`.
    pub fn rule(&self, style: StyleId) -> &'a StyleRule {
        self.style_sheet.get_rule(style.index())
    }

    /// The number a note's marker carries, and the note's place in its
    /// sequence. `\f + …` numbers run 1, 2, 3 … over the whole document;
    /// every other caller — `\f - …` (no marker shown) and `\f * …` (a custom
    /// one) — shares a second sequence of its own, so a custom-caller note
    /// never consumes a number from the `+` run. Both sequences start at 1 and
    /// neither is reset by a chapter.
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
