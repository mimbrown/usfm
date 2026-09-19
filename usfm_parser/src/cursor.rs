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
    pub(crate) fn src(&self, span: Span) -> &'a str {
        // SAFETY: the lexer guarantees token spans are in bounds and on
        // UTF-8 character boundaries, and every span passed here is a token
        // span or a sub-range of one at ASCII boundaries.
        unsafe {
            self.source_text
                .get_unchecked(span.start as usize..span.end as usize)
        }
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
