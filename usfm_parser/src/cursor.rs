//! Token navigation helpers for `ParserImpl`.

use crate::lexer::Kind;
use crate::lexer::span::Span;
use crate::parser::ParserImpl;

impl<'a> ParserImpl<'a> {
    /// Current token kind.
    #[inline]
    pub(crate) fn cur_kind(&self) -> Kind {
        self.lexer.token.kind
    }

    /// Current token span.
    #[inline]
    pub(crate) fn cur_span(&self) -> Span {
        self.lexer.token.span
    }

    /// Source text covered by the current token.
    pub(crate) fn cur_src(&self) -> &'a str {
        self.src(self.lexer.token.span)
    }

    /// Source text covered by `span`.
    ///
    /// The one `unsafe` left in the workspace. Ticket 05 replaced the other 26
    /// after measuring that they cost nothing; this one is on the parser's
    /// hottest path — every word, every marker name, every attribute — and it
    /// is the only one the benchmark defends. Making it safe costs **2.3–3.5%
    /// on `parse/whole-corpus` and 3.8–4.8% on `parse/plain`**, whether it is
    /// written `&self.source_text[a..b]` or
    /// `self.source_text.get(a..b).unwrap_or("")`: both pay two
    /// `is_char_boundary` checks per call. Eight interleaved rounds of the first
    /// pair, three of the second; see `docs/benchmarks.md`. That is over the 2%
    /// the ADR asks for, so it stays.
    ///
    /// What pays for it is the `debug_assert` below, not a comment: it is on in
    /// every test build, so the whole unit and integration suite — and
    /// `scripts/miri.sh` on top of it — checks the invariant on every call.
    /// Miri would catch a read past the end of the source on its own, but not a
    /// slice that splits a UTF-8 character, which is why the boundaries are
    /// asserted here rather than left to Miri.
    pub(crate) fn src(&self, span: Span) -> &'a str {
        let (start, end) = (span.start as usize, span.end as usize);
        debug_assert!(
            start <= end
                && end <= self.source_text.len()
                && self.source_text.is_char_boundary(start)
                && self.source_text.is_char_boundary(end),
            "span {span:?} is not a character-aligned range of the source"
        );
        // SAFETY: `span` is a token span the lexer produced, or a sub-range of
        // one cut at bytes the lexer has checked are ASCII (`Token::name_span`
        // trims `\`, `+` and `*`). The lexer's `Source` only ever moves its
        // cursor by `char::len_utf8` or over a byte it has checked is ASCII, so
        // both ends are within `source_text` and on UTF-8 character boundaries.
        unsafe { self.source_text.get_unchecked(start..end) }
    }

    /// For a `Marker` token, the marker name without `\`, `+`, or `*`.
    /// For any other token, the whole token text.
    pub(crate) fn cur_marker_name(&self) -> &'a str {
        self.src(self.lexer.token.name_span())
    }

    /// Checks if the current token has kind `kind`.
    #[inline]
    pub(crate) fn at(&self, kind: Kind) -> bool {
        self.cur_kind() == kind
    }

    /// End offset of the last token consumed, which is where a node that runs
    /// to "wherever parsing stopped" should end. Using `cur_span().start`
    /// instead would stretch the node over trailing whitespace.
    #[inline]
    pub(crate) fn prev_token_end(&self) -> u32 {
        self.prev_token_end
    }

    /// Move to the next token.
    ///
    /// Every other method here funnels through this one, so it is the single
    /// place `prev_token_end` has to be maintained.
    #[inline]
    fn advance(&mut self) {
        self.prev_token_end = self.lexer.token.span.end;
        self.lexer.advance();
    }

    /// Advance and return true if at `kind`, otherwise return false.
    #[inline]
    pub(crate) fn eat(&mut self, kind: Kind) -> bool {
        if self.at(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Advance past any whitespace token. Returns true if one was consumed.
    #[inline]
    pub(crate) fn eat_whitespace(&mut self) -> bool {
        if self.cur_kind().is_whitespace() {
            self.advance();
            true
        } else {
            false
        }
    }

    /// If the current token is a `Word`, return it and advance.
    pub(crate) fn eat_word(&mut self) -> Option<&'a str> {
        if self.at(Kind::Word) {
            let word = self.cur_src();
            self.advance();
            Some(word)
        } else {
            None
        }
    }

    /// Advance past any token.
    #[inline]
    pub(crate) fn bump_any(&mut self) {
        self.advance();
    }

    /// Consume the current token and return its span.
    #[inline]
    pub(crate) fn bump_span(&mut self) -> Span {
        let span = self.cur_span();
        self.advance();
        span
    }

    /// True if the current token is an opening marker named `name`.
    pub(crate) fn at_opening_marker(&self, name: &str) -> bool {
        self.cur_kind().is_opening_marker() && self.cur_marker_name() == name
    }

    /// If the current token is a closing marker named `name`, consume it.
    pub(crate) fn eat_closing_marker(&mut self, name: &str) -> bool {
        if self.cur_kind().is_closing_marker() && self.cur_marker_name() == name {
            self.advance();
            true
        } else {
            false
        }
    }
}
