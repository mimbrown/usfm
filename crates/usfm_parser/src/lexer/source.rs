use std::marker::PhantomData;

use crate::{MAX_LEN, UniquePromise};

/// `Source` holds the source text for the lexer, and provides APIs to read it.
///
/// It provides a cursor which allows consuming source text either as `char`s, or as bytes.
/// It replaces `std::str::Chars` iterator which performed the same function previously,
/// but was less flexible as only allowed consuming source char by char.
///
/// Consuming source text byte-by-byte is often more performant than char-by-char.
///
/// `Source` provides:
///
/// * API for consuming source char-by-char (`Source::next_char`, `Source::peek_char`).
/// * API for peeking the next source byte (`Source::peek_byte`) or two
///   (`Source::peek_2_bytes`).
/// * API for advancing over a known ASCII byte (`Source::advance_if_ascii_eq`).
/// * API for rewinding to a previous position in source
///   (`Source::position`, `Source::set_position`).
///
/// # Composition of `Source`
///
/// * `text` is the whole source text.
/// * `offset` is the cursor: a byte offset into `text`.
///
/// # Invariants of `Source`
///
/// 1. `offset` <= `text.len()`.
///    i.e. cursor always within bounds of source text `&str`, or 1 byte after the last byte
///    of source text (positioned on EOF).
/// 2. `offset` is always on a UTF-8 character boundary, or EOF.
///    i.e. pointing at the *1st* byte of a UTF-8 character.
///
/// Both invariants are enforced by the compiler rather than by this type: every read goes
/// through a slice index or `slice::get`, so a violation is a panic, not undefined behaviour.
/// `Source` still upholds them so that no index ever panics: `Source::next_char` advances by
/// `char::len_utf8`, and `Source::advance_if_ascii_eq` only advances over a byte it has
/// checked is ASCII.
///
/// This type was ported from oxc's lexer, which holds three raw pointers and reads through
/// them. Ticket 05 replaced the pointers with an offset after measuring: the offset version
/// lexes the whole corpus as fast as the pointer version did (153.7 MiB/s against
/// 152.4–153.5), so the 21 `unsafe` blocks and `unsafe fn`s this file used to hold did not
/// earn their place. `docs/benchmarks.md` has the runs, and the warning that goes with them:
/// a `lex` number measured on a whole binary carries ±5–8% of codegen-unit placement, so
/// compare this file's cost across an edit only with several binaries interleaved.
pub(super) struct Source<'a> {
    /// The whole source string. Never altered after initialization.
    text: &'a str,
    /// Current position in the source string, as a byte offset into `text`.
    offset: usize,
}

impl<'a> Source<'a> {
    /// Create `Source` from `&str`.
    ///
    /// `UniquePromise` is no longer load-bearing for soundness — it was what guaranteed
    /// a `SourcePosition` could only come from the one `Source` on this thread, back when
    /// a `SourcePosition` was a raw pointer. It is kept because it is the parser's public
    /// entry-token for building a `Lexer` (see `crate::UniquePromise`).
    #[expect(unused_variables, clippy::needless_pass_by_value)]
    pub(super) fn new(mut source_text: &'a str, unique: UniquePromise) -> Self {
        // If source text exceeds size limit, substitute a short source text which will fail to parse.
        // `Parser::parse` will convert error to `diagnostics::overlong_source()`.
        if source_text.len() > MAX_LEN {
            source_text = "\0";
        }

        // A UTF-8 byte-order mark is not content: Paratext and Windows
        // editors write one at the head of a book, and read as text it would
        // stand in front of the `\id`. Starting after it keeps every span a
        // byte offset into the text as given.
        let offset = if source_text.starts_with('\u{feff}') {
            '\u{feff}'.len_utf8()
        } else {
            0
        };
        Self {
            text: source_text,
            offset,
        }
    }

    /// Get remaining source text as `&str`.
    #[inline]
    pub(super) fn remaining(&self) -> &'a str {
        // Invariants of `Source`: `offset` is in bounds and on a UTF-8 character boundary,
        // so this index cannot panic.
        &self.text[self.offset..]
    }

    /// Get current position.
    ///
    /// The `SourcePosition` returned is guaranteed to be within bounds of the `&str` that
    /// `Source` was created from, and on a UTF-8 character boundary, so can be used by the
    /// caller to later move the current position of this `Source` using `Source::set_position`.
    ///
    /// `SourcePosition` lives as long as the source text `&str` that `Source` was created from.
    #[inline]
    pub(super) fn position(&self) -> SourcePosition<'a> {
        SourcePosition {
            offset: self.offset,
            _marker: PhantomData,
        }
    }

    /// Move current position.
    ///
    /// `pos` must have come from this `Source`. If it came from a longer one, the
    /// `debug_assert`s below catch it in tests and the next read panics in release —
    /// neither is undefined behaviour.
    #[inline]
    pub(super) fn set_position(&mut self, pos: SourcePosition<'a>) {
        debug_assert!(pos.offset <= self.text.len());
        debug_assert!(self.text.is_char_boundary(pos.offset));
        self.offset = pos.offset;
    }

    /// Advance `Source`'s cursor by one byte if it is equal to the given ASCII value.
    ///
    /// `ascii_byte` must be ASCII: advancing one byte past a non-ASCII byte would leave the
    /// cursor inside a UTF-8 character, and the next `remaining()` would panic. The
    /// `debug_assert` is what holds callers to it.
    #[inline]
    pub(super) fn advance_if_ascii_eq(&mut self, ascii_byte: u8) -> bool {
        debug_assert!(ascii_byte.is_ascii());
        let matched = self.peek_byte() == Some(ascii_byte);
        if matched {
            self.offset += 1;
        }
        matched
    }

    /// Get current position in source, relative to start of source.
    #[expect(clippy::cast_possible_truncation)]
    #[inline]
    pub(crate) fn offset(&self) -> u32 {
        // Cannot overflow `u32` because of the `MAX_LEN` check in `Source::new`.
        self.offset as u32
    }

    /// Get next char of source, and advance position to after it.
    #[inline]
    pub(super) fn next_char(&mut self) -> Option<char> {
        // Check not at EOF and handle ASCII bytes
        let byte = self.peek_byte()?;
        if byte.is_ascii() {
            // Current byte is ASCII, so the incremented offset is on a UTF-8 character
            // boundary, and is at most `text.len()`.
            self.offset += 1;
            return Some(byte as char);
        }

        // Multi-byte Unicode character. `remaining()` starts on a character boundary, so
        // `chars().next()` yields the whole character and `len_utf8` lands on the next one.
        let c = self.remaining().chars().next()?;
        self.offset += c.len_utf8();
        Some(c)
    }

    /// Peek next char of source, without consuming it.
    #[inline]
    pub(super) fn peek_char(&self) -> Option<char> {
        // Check not at EOF and handle ASCII bytes
        let byte = self.peek_byte()?;
        if byte.is_ascii() {
            return Some(byte as char);
        }

        // Multi-byte Unicode character.
        self.remaining().chars().next()
    }

    /// Peek next byte of source without consuming it.
    #[inline]
    pub(super) fn peek_byte(&self) -> Option<u8> {
        self.text.as_bytes().get(self.offset).copied()
    }

    /// Peek next two bytes of source without consuming them.
    #[inline]
    pub(super) fn peek_2_bytes(&self) -> Option<[u8; 2]> {
        let bytes = self.text.as_bytes();
        // `offset` is always <= `bytes.len()`, so `offset + 2` cannot wrap.
        if self.offset + 2 <= bytes.len() {
            Some([bytes[self.offset], bytes[self.offset + 1]])
        } else {
            None
        }
    }
}

/// A position in the `Source` that created it, as a byte offset.
///
/// A `SourcePosition` is always on a UTF-8 character boundary and within bounds of the
/// `Source` that created it (or one byte past its end, at EOF), because the only way to make
/// one is `Source::position`.
#[derive(Debug, Clone, Copy)]
pub struct SourcePosition<'a> {
    offset: usize,
    _marker: PhantomData<&'a str>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(text: &str) -> Source<'_> {
        Source::new(text, UniquePromise::new_for_tests_and_benchmarks())
    }

    #[test]
    fn walks_multi_byte_characters_whole() {
        // Every `next_char` must land on the next character boundary; a cursor left
        // inside a character would make `remaining()` panic (and, before ticket 05,
        // would have been undefined behaviour).
        let text = "a\u{a0}é漢\u{1f600}z";
        let mut source = source(text);
        let mut seen = String::new();
        while let Some(c) = source.next_char() {
            seen.push(c);
            // `remaining()` re-slices `text` at the cursor, which panics off a boundary.
            assert!(text.ends_with(source.remaining()));
        }
        assert_eq!(seen, text);
        assert!(source.remaining().is_empty());
        assert_eq!(source.offset() as usize, text.len());
    }

    #[test]
    fn peek_does_not_move_and_matches_next() {
        let text = "\\p é//x";
        let mut source = source(text);
        while let Some(peeked) = source.peek_char() {
            let offset = source.offset();
            assert_eq!(source.peek_char(), Some(peeked));
            assert_eq!(source.offset(), offset);
            assert_eq!(source.next_char(), Some(peeked));
        }
        assert_eq!(source.peek_byte(), None);
        assert_eq!(source.peek_2_bytes(), None);
    }

    #[test]
    fn peek_2_bytes_stops_one_byte_short_of_the_end() {
        let mut source = source("ab");
        assert_eq!(source.peek_2_bytes(), Some(*b"ab"));
        source.next_char();
        // One byte left: reading two would run past the end.
        assert_eq!(source.peek_2_bytes(), None);
        assert_eq!(source.peek_byte(), Some(b'b'));
    }

    #[test]
    fn advance_if_ascii_eq_only_moves_on_a_match() {
        let mut source = source("*x");
        assert!(!source.advance_if_ascii_eq(b'x'));
        assert_eq!(source.offset(), 0);
        assert!(source.advance_if_ascii_eq(b'*'));
        assert_eq!(source.offset(), 1);
        // At EOF it must not move either.
        source.next_char();
        assert!(source.remaining().is_empty());
        assert!(!source.advance_if_ascii_eq(b'x'));
        assert_eq!(source.offset(), 2);
    }

    #[test]
    fn positions_round_trip_across_multi_byte_text() {
        let text = "é漢x";
        let mut source = source(text);
        let start = source.position();
        source.next_char();
        let after_first = source.position();
        source.next_char();
        source.next_char();
        assert!(source.remaining().is_empty());
        source.set_position(start);
        assert_eq!(source.remaining(), text);
        source.set_position(after_first);
        assert_eq!(source.remaining(), "漢x");
        assert_eq!(source.offset(), 2);
    }

    #[test]
    fn the_substitute_for_an_overlong_source_reads_as_one_character() {
        // `Source::new` swaps a source past `MAX_LEN` for `"\0"`, which the parser
        // rejects. `MAX_LEN` is too large to allocate in a test, so what is checked
        // here is the substitute text itself: the cursor must walk it like any other.
        let mut source = source("\0");
        assert_eq!(source.remaining(), "\0");
        assert_eq!(source.peek_byte(), Some(0));
        assert_eq!(source.next_char(), Some('\0'));
        assert!(source.remaining().is_empty());
    }
}
