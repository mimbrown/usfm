//! USFM lexer.
//!
//! Produces a flat token stream over the source text. Markers are recognised
//! here rather than in the parser, because `\name`, `\+name`, `\name*`, `\*`
//! and the escapes `\\` / `\|` are lexically unambiguous. Everything else is
//! a coarse classification (words, whitespace runs, and the attribute
//! punctuation `| = "`) that the parser reassembles into text by span.
//!
//! Invariant: the spans of all tokens (excluding the final `Eof`) tile the
//! source exactly, in order, with no gaps or overlaps.

use std::mem;

use source::Source;
use span::{SPAN, Span};

use crate::UniquePromise;

pub mod source;
/// Re-exported from `usfm_ast`: spans are part of the AST's public surface
/// (hardening plan D2), so the AST crate owns the type.
pub use usfm_ast::span;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Kind {
    /// A marker: `\name`, `\+name`, `\name*`, or `\+name*`.
    /// The span covers the whole marker including the backslash, the
    /// optional `+`, and the optional trailing `*`.
    Marker {
        nested: bool,
        closing: bool,
    },
    /// `\*` — closes a milestone.
    MilestoneEnd,
    /// `\\`, `\|`, or `\"` — an escaped literal. The literal is the second
    /// byte of the span.
    Escape,
    /// A `\` (optionally followed by `+`) not followed by a marker name.
    Backslash,
    /// A run of characters that are not whitespace and not one of `\ * | = "`.
    Word,
    /// A run of Unicode whitespace. `has_newline` is set if the run contains
    /// `\n` or `\r`.
    Whitespace {
        has_newline: bool,
    },
    /// A `*` not attached to a marker.
    Star,
    /// `//`: an optional line break. Ends a word wherever it appears.
    OptBreak,
    /// `|`
    Pipe,
    /// `=`
    Equal,
    /// `"`
    DoubleQuote,
    Eof,
}

impl Kind {
    /// Kinds that contribute to running text content.
    #[inline]
    pub fn is_text(self) -> bool {
        matches!(
            self,
            Kind::Word | Kind::Whitespace { .. } | Kind::Star | Kind::Equal | Kind::DoubleQuote
        )
    }

    #[inline]
    pub fn is_whitespace(self) -> bool {
        matches!(self, Kind::Whitespace { .. })
    }

    #[inline]
    pub fn is_marker(self) -> bool {
        matches!(self, Kind::Marker { .. })
    }

    #[inline]
    pub fn is_opening_marker(self) -> bool {
        matches!(self, Kind::Marker { closing: false, .. })
    }

    #[inline]
    pub fn is_closing_marker(self) -> bool {
        matches!(self, Kind::Marker { closing: true, .. })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub kind: Kind,
    pub span: Span,
}

impl Token {
    pub fn take(&mut self) -> Token {
        mem::take(self)
    }

    /// For a `Marker` token, the byte range of the name within the source
    /// (excluding `\`, `+`, and `*`). For other kinds, the whole span.
    pub fn name_span(&self) -> Span {
        match self.kind {
            Kind::Marker { nested, closing } => {
                let start = self.span.start + 1 + u32::from(nested);
                let end = self.span.end - u32::from(closing);
                Span::new(start, end)
            }
            _ => self.span,
        }
    }
}

impl Default for Token {
    fn default() -> Self {
        Self {
            kind: Kind::Eof,
            span: SPAN,
        }
    }
}

/// Bytes allowed in a marker name. USFM marker names are ASCII letters and
/// digits, plus `-` for milestone suffixes (`qt-s`) and `_` for custom markers.
#[inline]
fn is_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'
}

#[inline]
fn is_word_byte_terminator(byte: u8) -> bool {
    matches!(byte, b'\\' | b'*' | b'|' | b'=' | b'"')
}

/// Consume marker-name bytes. Returns how many were consumed.
fn consume_name(source: &mut Source) -> usize {
    let mut count = 0;
    while let Some(byte) = source.peek_byte() {
        if !is_name_byte(byte) {
            break;
        }
        // `is_name_byte` only accepts ASCII, so advancing one byte keeps the
        // cursor on a UTF-8 boundary.
        source.advance_if_ascii_eq(byte);
        count += 1;
    }
    count
}

/// Lex the token following a `\` that has already been consumed.
fn lex_after_backslash(source: &mut Source) -> Kind {
    match source.peek_byte() {
        Some(b'\\') | Some(b'|') | Some(b'"') => {
            source.next_char();
            Kind::Escape
        }
        Some(b'*') => {
            source.next_char();
            Kind::MilestoneEnd
        }
        Some(b'+') => {
            source.next_char();
            if consume_name(source) == 0 {
                return Kind::Backslash;
            }
            let closing = source.advance_if_ascii_eq(b'*');
            Kind::Marker {
                nested: true,
                closing,
            }
        }
        Some(byte) if is_name_byte(byte) => {
            consume_name(source);
            let closing = source.advance_if_ascii_eq(b'*');
            Kind::Marker {
                nested: false,
                closing,
            }
        }
        _ => Kind::Backslash,
    }
}

fn read_token(source: &mut Source) -> Token {
    let start = source.offset();
    let Some(byte) = source.peek_byte() else {
        return Token {
            kind: Kind::Eof,
            span: Span::new(start, start),
        };
    };
    let kind = match byte {
        b'|' => {
            source.next_char();
            Kind::Pipe
        }
        b'=' => {
            source.next_char();
            Kind::Equal
        }
        b'"' => {
            source.next_char();
            Kind::DoubleQuote
        }
        b'*' => {
            source.next_char();
            Kind::Star
        }
        b'\\' => {
            source.next_char();
            lex_after_backslash(source)
        }
        b'/' if source.peek_2_bytes() == Some(*b"//") => {
            source.next_char();
            source.next_char();
            Kind::OptBreak
        }
        _ => {
            // Not at EOF: a byte was peeked above.
            let first = source.next_char().unwrap();
            // Only ASCII whitespace is USFM whitespace. A no-break space
            // (U+00A0), an ideographic space (U+3000) and the like are
            // content: they must survive normalisation and trimming intact.
            if first.is_ascii_whitespace() {
                let mut has_newline = matches!(first, '\n' | '\r');
                while let Some(c) = source.peek_char() {
                    if !c.is_ascii_whitespace() {
                        break;
                    }
                    has_newline |= matches!(c, '\n' | '\r');
                    source.next_char();
                }
                Kind::Whitespace { has_newline }
            } else {
                while let Some(c) = source.peek_char() {
                    if c.is_ascii_whitespace()
                        || (c.is_ascii() && is_word_byte_terminator(c as u8))
                        || source.peek_2_bytes() == Some(*b"//")
                    {
                        break;
                    }
                    source.next_char();
                }
                Kind::Word
            }
        }
    };
    Token {
        kind,
        span: Span::new(start, source.offset()),
    }
}

pub struct Lexer<'a> {
    source: Source<'a>,
    pub(crate) token: Token,
}

/// A saved lexer position. See `Lexer::checkpoint`.
#[derive(Clone, Copy)]
pub(crate) struct LexerCheckpoint<'a> {
    position: source::SourcePosition<'a>,
    token: Token,
}

impl<'a> Lexer<'a> {
    pub fn new(source_text: &'a str, unique: UniquePromise) -> Self {
        let mut source = Source::new(source_text, unique);
        let token = read_token(&mut source);
        Self { source, token }
    }

    pub fn cur_offset(&self) -> u32 {
        self.source.offset()
    }

    /// Advance to the next token.
    pub fn advance(&mut self) {
        self.token = read_token(&mut self.source);
    }

    /// Look at the token after the current one without consuming anything.
    pub fn peek(&mut self) -> Token {
        let pos = self.source.position();
        let token = read_token(&mut self.source);
        self.source.set_position(pos);
        token
    }

    /// Capture the current lexer state so it can be restored with `rewind`.
    pub(crate) fn checkpoint(&self) -> LexerCheckpoint<'a> {
        LexerCheckpoint {
            position: self.source.position(),
            token: self.token,
        }
    }

    /// Restore state captured by `checkpoint`.
    pub(crate) fn rewind(&mut self, checkpoint: LexerCheckpoint<'a>) {
        self.source.set_position(checkpoint.position);
        self.token = checkpoint.token;
    }

    pub fn has_remaining(&self) -> bool {
        self.token.kind != Kind::Eof
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        if self.token.kind == Kind::Eof {
            return None;
        }
        let token = self.token.take();
        self.advance();
        Some(token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex(source: &str) -> Vec<(Kind, &str)> {
        Lexer::new(source, UniquePromise::new_for_tests_and_benchmarks())
            .map(|token| (token.kind, &source[token.span]))
            .collect()
    }

    fn kinds(source: &str) -> Vec<Kind> {
        lex(source).into_iter().map(|(kind, _)| kind).collect()
    }

    const OPEN: Kind = Kind::Marker {
        nested: false,
        closing: false,
    };
    const CLOSE: Kind = Kind::Marker {
        nested: false,
        closing: true,
    };
    const WS: Kind = Kind::Whitespace { has_newline: false };
    const NL: Kind = Kind::Whitespace { has_newline: true };

    #[test]
    fn punctuation() {
        assert_eq!(
            kinds(r#"|="*"#),
            vec![Kind::Pipe, Kind::Equal, Kind::DoubleQuote, Kind::Star]
        );
    }

    #[test]
    fn markers() {
        assert_eq!(
            lex(r"\p text \em*"),
            vec![
                (OPEN, r"\p"),
                (WS, " "),
                (Kind::Word, "text"),
                (WS, " "),
                (CLOSE, r"\em*"),
            ]
        );
    }

    #[test]
    fn nested_markers() {
        assert_eq!(
            lex(r"\+nd Lord\+nd*"),
            vec![
                (
                    Kind::Marker {
                        nested: true,
                        closing: false
                    },
                    r"\+nd"
                ),
                (WS, " "),
                (Kind::Word, "Lord"),
                (
                    Kind::Marker {
                        nested: true,
                        closing: true
                    },
                    r"\+nd*"
                ),
            ]
        );
    }

    #[test]
    fn marker_name_span() {
        let source = r"\+nd*";
        let token = Lexer::new(source, UniquePromise::new_for_tests_and_benchmarks())
            .next()
            .unwrap();
        assert_eq!(&source[token.name_span()], "nd");
    }

    #[test]
    fn milestone_and_escapes() {
        assert_eq!(
            lex(r#"\qt-s |who=x\* a\\b\|c\"d"#),
            vec![
                (OPEN, r"\qt-s"),
                (WS, " "),
                (Kind::Pipe, "|"),
                (Kind::Word, "who"),
                (Kind::Equal, "="),
                (Kind::Word, "x"),
                (Kind::MilestoneEnd, r"\*"),
                (WS, " "),
                (Kind::Word, "a"),
                (Kind::Escape, r"\\"),
                (Kind::Word, "b"),
                (Kind::Escape, r"\|"),
                (Kind::Word, "c"),
                (Kind::Escape, r#"\""#),
                (Kind::Word, "d"),
            ]
        );
    }

    #[test]
    fn lone_backslash() {
        assert_eq!(
            lex("a \\ b \\+ c \\"),
            vec![
                (Kind::Word, "a"),
                (WS, " "),
                (Kind::Backslash, "\\"),
                (WS, " "),
                (Kind::Word, "b"),
                (WS, " "),
                (Kind::Backslash, "\\+"),
                (WS, " "),
                (Kind::Word, "c"),
                (WS, " "),
                (Kind::Backslash, "\\"),
            ]
        );
    }

    #[test]
    fn marker_name_stops_at_non_name_bytes() {
        // Punctuation and non-ASCII are not part of a marker name.
        assert_eq!(
            lex("\\p.x \\pé"),
            vec![
                (OPEN, r"\p"),
                (Kind::Word, ".x"),
                (WS, " "),
                (OPEN, r"\p"),
                (Kind::Word, "é"),
            ]
        );
    }

    #[test]
    fn whitespace_newline_flag() {
        assert_eq!(
            kinds("a \t b\nc\r\n d"),
            vec![Kind::Word, WS, Kind::Word, NL, Kind::Word, NL, Kind::Word]
        );
    }

    #[test]
    fn non_ascii_spaces_are_word_characters() {
        // NBSP and ideographic space are content, not separators.
        assert_eq!(kinds("a\u{a0}b \u{3000}c"), vec![Kind::Word, WS, Kind::Word]);
        assert_eq!(lex("a\u{a0}b")[0].1, "a\u{a0}b");
    }

    #[test]
    fn peek_does_not_consume() {
        let source = r"\p a";
        let mut lexer = Lexer::new(source, UniquePromise::new_for_tests_and_benchmarks());
        assert_eq!(lexer.token.kind, OPEN);
        assert_eq!(lexer.peek().kind, WS);
        assert_eq!(lexer.peek().kind, WS);
        assert_eq!(lexer.token.kind, OPEN);
        lexer.advance();
        assert_eq!(lexer.token.kind, WS);
        assert_eq!(lexer.peek().kind, Kind::Word);
    }

    #[test]
    fn tokens_tile_source() {
        let sources = [
            "",
            r"\id GEN Some book",
            "\\c 1\n\\p \\v 1 In the \\nd beginning\\nd* \\w x|lemma=\"y\"\\w*\n",
            "text with é and 漢字 and \u{200F} rtl marks\\*",
            "\\\\ \\| \\ \\+ *|=\"",
        ];
        for source in sources {
            let tokens: Vec<Token> =
                Lexer::new(source, UniquePromise::new_for_tests_and_benchmarks()).collect();
            let mut offset = 0u32;
            for token in &tokens {
                assert_eq!(token.span.start, offset, "gap or overlap in {source:?}");
                assert!(
                    token.span.end > token.span.start,
                    "empty token in {source:?}"
                );
                offset = token.span.end;
            }
            assert_eq!(
                offset as usize,
                source.len(),
                "did not reach end of {source:?}"
            );
        }
    }
}
