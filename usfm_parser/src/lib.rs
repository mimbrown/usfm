/// The default stylesheet, generated from `usfm.sty` by `build.rs` into
/// `OUT_DIR`. Nothing generated is checked in.
mod generated {
    include!(concat!(env!("OUT_DIR"), "/default_stylesheet.rs"));
}

pub use generated::*;

pub use usfm_ast as ast;
pub mod cursor;

/// Parse diagnostics, which live in [`usfm_diagnostics`].
///
/// Re-exported under the name the module had when it was
/// `usfm_parser/src/diagnostics.rs`, so `usfm_parser::diagnostics::Code` and
/// the parser's own `crate::diagnostics::…` paths keep resolving (ticket 12).
pub mod diagnostics {
    pub use usfm_diagnostics::*;
}

pub mod parser;
pub use diagnostics::{Code, Diagnostic, ParseResult, Severity};

/// The state an HTML serializer carries, which lives in
/// [`usfm_html`].
///
/// Re-exported under the name the module had when it was
/// `usfm_parser/src/context.rs`, so `usfm_parser::context::Context` keeps
/// resolving (ticket 14). Ticket 17 deletes this re-export along with the
/// parser's binary; new code should name [`usfm_html`] directly.
pub mod context {
    pub use usfm_html::context::*;
}

pub mod lexer;

/// The generic serializer trait an output format other than HTML would be
/// written against, which lives in [`usfm_html`].
///
/// Re-exported under the name the module had when it was
/// `usfm_parser/src/serialize.rs`; ticket 17 deletes it, as for [`context`].
pub mod serialize {
    pub use usfm_html::serialize::*;
}

/// HTML output, which lives in [`usfm_html`].
///
/// Re-exported under the name the module had when it was
/// `usfm_parser/src/serialize_html.rs`; ticket 17 deletes it, as for
/// [`context`].
pub mod serialize_html {
    pub use usfm_html::serialize_html::*;
}

pub use serialize_html::{SerializeHtml, ToHtml, serialize_html, to_html_string};

// The span invariants, shared by `tests/spans.rs` and `tasks/fuzz` so the two
// cannot drift apart. Behind a feature, so the default build is unchanged.
#[cfg(feature = "testing")]
pub mod span_check;

pub mod style;
pub mod text_replacements;

/// USX output, which lives in [`usfm_usx`].
///
/// Re-exported under the name the module had when it was
/// `usfm_parser/src/usx.rs`, so `usfm_parser::usx::to_usx_string` and the
/// CLI's own paths keep resolving (ticket 13). Ticket 17 deletes this
/// re-export along with the parser's binary; new code should name
/// [`usfm_usx`] directly.
pub mod usx {
    pub use usfm_usx::usx::*;
}

pub use usfm_ast::{fold, visit, visit_mut};

/// The XML tree and its writer, which live in [`usfm_usx`].
///
/// Re-exported under the name the module had when it was
/// `usfm_parser/src/xml_document.rs`; ticket 17 deletes it, as for [`usx`].
pub mod xml_document {
    pub use usfm_usx::xml_document::*;
}

mod parser_parse {
    use std::sync::Arc;

    use usfm_style::StyleSheet;

    use crate::{
        diagnostics::ParseResult,
        parser::{Parser, ParserImpl},
    };

    /// `UniquePromise` is a way to use the type system to enforce the invariant that only
    /// a single `ParserImpl`, `Lexer` and `lexer::Source` can exist at any time on a thread.
    ///
    /// `ParserImpl::new`, `Lexer::new` and `lexer::Source::new` all require a `UniquePromise`
    /// to be provided to them. `UniquePromise::new` is not visible outside this module, so only
    /// `Parser::parse` can create one, and it only calls `ParserImpl::new` once.
    /// This enforces the invariant throughout the entire parser.
    ///
    /// It used to be load-bearing for soundness: a `SourcePosition` was a raw pointer, and
    /// `Source::set_position` was safe only because the promise guaranteed the position had
    /// come from the one `Source` on this thread. Ticket 05 made `SourcePosition` an offset,
    /// so the worst a foreign position can now do is panic. What is left is a structural
    /// guarantee: one parse at a time, no accidental second lexer over the same text.
    ///
    /// `UniquePromise` is a zero-sized type and has no runtime cost. It's purely for the type-checker.
    ///
    /// `UniquePromise::new_for_tests_and_benchmarks` is a backdoor for tests/benchmarks, so they can
    /// create a `ParserImpl` or `Lexer`, and manipulate it directly, for testing/benchmarking purposes.
    pub struct UniquePromise(());

    impl UniquePromise {
        #[inline]
        fn new() -> Self {
            Self(())
        }

        /// Backdoor for tests/benchmarks to create a `UniquePromise` (see above).
        /// This function must NOT be exposed outside of tests and benchmarks,
        /// as it allows circumventing the one-parse-at-a-time invariant.
        #[cfg(any(test, feature = "benchmarking"))]
        pub fn new_for_tests_and_benchmarks() -> Self {
            Self(())
        }
    }

    impl<'a> Parser<'a> {
        /// Parse the source text. Never fails: malformed input is repaired
        /// and every repair is reported in `ParseResult::diagnostics`.
        /// Use `ParseResult::strict` to reject repaired input.
        pub fn parse(self, style_sheet: &Arc<StyleSheet>) -> ParseResult<'a> {
            self.parse_with_options(style_sheet, true)
        }

        /// Parse with configurable chapter/verse end milestone insertion.
        ///
        /// The returned document owns the stylesheet its styles resolve
        /// against (hardening plan D3): it is `style_sheet`, extended with any
        /// style the parser had to derive, so the caller never needs to know
        /// which sheet an index belongs to.
        pub fn parse_with_options(
            self,
            style_sheet: &Arc<StyleSheet>,
            insert_end_milestones: bool,
        ) -> ParseResult<'a> {
            let unique = UniquePromise::new();
            let mut parser = ParserImpl::new(self.source_text, style_sheet, unique);
            parser.should_insert_end_milestones(insert_end_milestones);
            parser.parse()
        }
    }
}
#[cfg(not(feature = "benchmarking"))]
use parser_parse::UniquePromise;
// Re-exported under `benchmarking` only, so `tasks/benchmark` can name the type
// `Lexer::new` needs and construct a `Lexer`. The default build keeps the
// private `use` above and exposes nothing.
#[cfg(feature = "benchmarking")]
pub use parser_parse::UniquePromise;

/// Maximum length of source which can be parsed (in bytes).
/// ~4 GiB on 64-bit systems, ~2 GiB on 32-bit systems.
// Length is constrained by 2 factors:
// 1. `Span`'s `start` and `end` are `u32`s, which limits length to `u32::MAX` bytes.
// 2. Rust's allocator APIs limit allocations to `isize::MAX`.
// https://doc.rust-lang.org/std/alloc/struct.Layout.html#method.from_size_align
pub(crate) const MAX_LEN: usize = if std::mem::size_of::<usize>() >= 8 {
    // 64-bit systems
    u32::MAX as usize
} else {
    // 32-bit or 16-bit systems
    isize::MAX as usize
};

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use crate::{
        ast::*, generated::DEFAULT_STYLESHEET, lexer::span::Span, parser::Parser,
        usx::to_usx_string,
    };

    /// The `StyleId` of a marker in the default stylesheet.
    fn style_of(marker: &str) -> StyleId {
        StyleId::new(*DEFAULT_STYLESHEET.get_marker_index(marker).unwrap() as u32)
    }

    /// The span of the first occurrence of `needle` in `input`.
    ///
    /// Spans are written as the source they should cover rather than as
    /// offsets, so an expected span says what it means and cannot drift out of
    /// step with the input string above it.
    fn span_of(input: &str, needle: &str) -> Span {
        let start = input
            .find(needle)
            .unwrap_or_else(|| panic!("{needle:?} is not in {input:?}")) as u32;
        Span::new(start, start + needle.len() as u32)
    }

    /// Parse a fragment, allowing only the structural diagnostics a
    /// fragment without `\id`/`\c` necessarily produces.
    fn parse_clean(input: &str) -> Document<'_> {
        use crate::diagnostics::Code;
        let result = Parser::new(input).parse(&DEFAULT_STYLESHEET);
        let unexpected: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| {
                !matches!(
                    d.code,
                    Code::MissingId | Code::VerseOutsideChapter | Code::VerseTextBeforeChapter
                )
            })
            .collect();
        assert!(
            unexpected.is_empty(),
            "unexpected diagnostics: {unexpected:?}"
        );
        result.document
    }

    #[test]
    fn parse_simple() {
        let input = r#"\p hello"#;
        let document = parse_clean(input);
        assert_eq!(
            document.blocks,
            vec![Block::Para(Para {
                style: style_of("p"),
                children: vec![Inline::Text(Text::new(
                    Cow::Borrowed("hello"),
                    span_of(input, "hello")
                ))],
                span: span_of(input, r#"\p hello"#),
            }),]
        );
    }

    #[test]
    fn parse_str() {
        let input = r"\p hello, \em world!\em*";
        let document = parse_clean(input);
        assert_eq!(
            document.blocks,
            vec![Block::Para(Para {
                style: style_of("p"),
                children: vec![
                    Inline::Text(Text::new(
                        Cow::Borrowed("hello, "),
                        span_of(input, "hello, ")
                    )),
                    Inline::Char(Char {
                        style: style_of("em"),
                        children: vec![Inline::Text(Text::new(
                            Cow::Borrowed("world!"),
                            span_of(input, "world!")
                        ))],
                        attributes: None,
                        span: span_of(input, r"\em world!\em*"),
                    }),
                ],
                span: span_of(input, input),
            }),]
        );
    }

    #[test]
    fn parse_str_with_escape() {
        let input = r#"\p hello\\, \em world!\em*"#;
        let document = parse_clean(input);
        assert_eq!(
            document.blocks,
            vec![Block::Para(Para {
                style: style_of("p"),
                children: vec![
                    // One text node: the escape is merged with the runs either
                    // side of it, and the span covers all three.
                    Inline::Text(Text::new(
                        Cow::Borrowed("hello\\, "),
                        span_of(input, r#"hello\\, "#)
                    )),
                    Inline::Char(Char {
                        style: style_of("em"),
                        children: vec![Inline::Text(Text::new(
                            Cow::Borrowed("world!"),
                            span_of(input, "world!")
                        ))],
                        attributes: None,
                        span: span_of(input, r#"\em world!\em*"#),
                    }),
                ],
                span: span_of(input, input),
            }),]
        );
    }

    #[test]
    fn parse_implicit_close() {
        let input = r#"
\p hello, \em world!

\p Next paragraph
"#;
        let result = Parser::new(input).parse(&DEFAULT_STYLESHEET);
        assert_eq!(
            result.document.blocks,
            vec![
                Block::Para(Para {
                    style: style_of("p"),
                    children: vec![
                        Inline::Text(Text::new(
                            Cow::Borrowed("hello, "),
                            span_of(input, "hello, ")
                        )),
                        Inline::Char(Char {
                            style: style_of("em"),
                            // The run was read up to the blank line, so its
                            // span covers the newlines the content dropped.
                            children: vec![Inline::Text(Text::new(
                                Cow::Borrowed("world!"),
                                span_of(input, "world!\n\n")
                            ))],
                            attributes: None,
                            span: span_of(input, "\\em world!\n\n"),
                        }),
                    ],
                    span: span_of(input, "\\p hello, \\em world!\n\n"),
                }),
                Block::Para(Para {
                    style: style_of("p"),
                    children: vec![Inline::Text(Text::new(
                        Cow::Borrowed("Next paragraph"),
                        span_of(input, "Next paragraph\n")
                    ))],
                    span: span_of(input, "\\p Next paragraph\n"),
                }),
            ]
        );
        let codes: Vec<_> = result.diagnostics.iter().map(|d| d.code).collect();
        assert_eq!(
            codes,
            vec![
                crate::diagnostics::Code::MissingId,
                crate::diagnostics::Code::CharacterStyleNotClosed
            ]
        );
    }

    #[test]
    fn test_usx() {
        let document = parse_clean(
            r#"
        \id GEN Some description.
        \c 1
        \ca 2-3\ca*
        \cp A
        \p \v 1 This is a \em test\em*.
        \s Header
        \p \v 2 This is another. \v 3 More content. \v 4 And more.\f + \fr 1.4: \ft Some manuscripts do not have \fq And more. \f*
        \tr \th1 Tribe \th2 Leader \thr3 Number
        \tr \tc1 Reuben \tc2 Elizur son of Shedeur \tcr3 46,500
        \tr \tc1 \v 5 Simeon \tc2 Shelumiel son of Zurishaddai \tcr3 59,300
        \tr \tc1 Gad \tc2 Eliasaph son of Deuel \tcr3 45,650
        \tr \tcr1-2 Total: \tcr3 151,450
        \p
        And another paragraph.
        "#,
        );
        let serialized = to_usx_string(&document);
        println!("{}", serialized);
    }
}
