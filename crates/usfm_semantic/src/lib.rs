//! Semantic checks over a parsed USFM document.
//!
//! The split with the parser is a split of jobs, not of subject matter
//! (`.scratch/oxc-layout/spec.md`, M4):
//!
//! * the **parser** repairs. When the input is not USFM it has to put
//!   *something* in the tree — drop a marker, close a style, open an implicit
//!   `\p` — and it reports that repair so the caller can see the tree is a
//!   guess. A [`Code`] the parser owns always answers "what does the tree hold
//!   instead?".
//! * this crate **reports**. A check here reads the finished tree and the
//!   stylesheet it resolves against, and says something about a document that
//!   parsed exactly as written: a marker in a place its `OccursUnder` does not
//!   list, a table column out of order, a verse number with a leading zero, a
//!   book code USX accepts but USFM does not list. Nothing here changes the
//!   tree, and none of these codes has a "recovery" to describe.
//!
//! The practical test for where a check belongs: if deleting it would change
//! the tree, it is the parser's; if deleting it would only make the diagnostics
//! shorter, it is this crate's.
//!
//! [`analyze`] is the whole API. Callers usually reach it through
//! `usfm::parse`, which merges these diagnostics with the parser's; a caller
//! that has a `Document` from somewhere else — an editor holding a cached
//! tree, a test — can call it directly.
//!
//! ```
//! // The parser alone reports nothing about `\id ZZZ`; the check is here.
//! let result = usfm::parser::parser::Parser::new("\\id ZZZ\n\\c 1\n\\p \\v 1 a\n")
//!     .parse(&usfm::DEFAULT_STYLESHEET);
//! assert!(result.diagnostics.is_empty());
//!
//! let diagnostics = usfm_semantic::analyze(&result.document);
//! assert_eq!(diagnostics[0].code, usfm::Code::UnlistedBookCode);
//! ```
//!
//! **Spans.** A check reports the span of the node it read. Where the node's
//! span is wider than the thing being reported — a [`Book`] carries one span
//! over the whole `\id` line, not one for the code — the node's span is what
//! is reported: the source text is not available here, and narrowing by
//! arithmetic (`span.start + 4`) would assume a single space after the marker
//! and point at whitespace whenever the input has two.

use usfm_ast::visit::{Visit, walk_document};
use usfm_ast::{Book, Document};
use usfm_diagnostics::{Code, Diagnostic};
use usfm_style::StyleSheet;

/// Run every semantic check over `document`.
///
/// The returned diagnostics are sorted by `span.start`, stably: two checks
/// that report at the same offset keep the order they were emitted in, which
/// is the order of the walk.
pub fn analyze(document: &Document) -> Vec<Diagnostic> {
    let mut analyzer = Analyzer::new(document.style_sheet());
    analyzer.visit_document(document);
    let mut diagnostics = analyzer.diagnostics;
    diagnostics.sort_by_key(|diagnostic| diagnostic.span.start);
    diagnostics
}

/// One walk of the tree, running every check.
///
/// The checks share a visitor rather than each taking their own pass: a check
/// is a few lines in a `visit_*` method, and the tree is walked once however
/// many there are. Each one is a private method named after its [`Code`], so
/// the mapping from the recovery table to the code that implements it stays
/// one to one.
struct Analyzer<'a> {
    /// The sheet the document's `StyleId`s resolve against — the parser's
    /// extended with anything it had to derive (hardening plan D3), never
    /// `DEFAULT_STYLESHEET`.
    #[expect(
        dead_code,
        reason = "the placement and attribute checks of ticket 20 are what read it; \
                  holding it from the start is what fixes *which* sheet they read"
    )]
    style_sheet: &'a StyleSheet,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Analyzer<'a> {
    fn new(style_sheet: &'a StyleSheet) -> Self {
        Self {
            style_sheet,
            diagnostics: Vec::new(),
        }
    }

    /// `unlisted-book-code`: a code that is well formed — `book@code` in
    /// `usx.rnc` allows `[A-Z][A-Z0-9]{2}` and two numeric shapes beyond the
    /// book list — but is not one of the books [`usfm_ast::BookCode`] names.
    ///
    /// Nothing is repaired: the parser keeps the book as `BookCode::Other` and
    /// every writer emits it verbatim (`<book code="TST">`). It is reported
    /// because a typo in a real code (`ZZZ` for `ZEC`) has exactly this shape,
    /// which is a judgement about the document and so belongs here rather than
    /// in the parser.
    ///
    /// The span is the whole `\id` line: see the note on spans in the module
    /// documentation.
    fn check_unlisted_book_code(&mut self, book: &Book<'_>) {
        if book.code.is_listed() {
            return;
        }
        self.diagnostics.push(Diagnostic::new(
            Code::UnlistedBookCode,
            book.span,
            format!(
                "`{}` is not one of the book codes USFM lists; kept as written",
                book.code
            ),
        ));
    }
}

impl Visit for Analyzer<'_> {
    fn visit_document(&mut self, document: &Document<'_>) {
        walk_document(self, document);
    }

    fn visit_book(&mut self, book: &Book<'_>) {
        self.check_unlisted_book_code(book);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;
    use std::str::FromStr;
    use usfm_ast::{Block, BookCode};
    use usfm_span::Span;

    /// A one-block document holding `code`, built by hand: this crate does not
    /// depend on the parser, and the integration tests in `tests/checks.rs`
    /// cover the real path through the facade.
    fn document_with_book(code: &str, span: Span) -> Document<'static> {
        Document::without_styles(vec![Block::Book(Book {
            code: BookCode::from_str(code).expect("a well-formed book code"),
            description: Cow::Borrowed(""),
            span,
        })])
    }

    #[test]
    fn a_listed_book_code_reports_nothing() {
        let document = document_with_book("GEN", Span::new(0, 7));
        assert!(analyze(&document).is_empty());
    }

    #[test]
    fn an_unlisted_book_code_is_reported_at_the_id_line() {
        let document = document_with_book("ZZZ", Span::new(0, 7));
        let diagnostics = analyze(&document);
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, Code::UnlistedBookCode);
        assert_eq!(diagnostics[0].span, Span::new(0, 7));
        assert!(
            diagnostics[0].message.contains("ZZZ"),
            "{}",
            diagnostics[0].message
        );
    }

    #[test]
    fn diagnostics_come_back_sorted_by_span_start() {
        let document = Document::without_styles(vec![
            Block::Book(Book {
                code: BookCode::from_str("ZZZ").unwrap(),
                description: Cow::Borrowed(""),
                span: Span::new(40, 47),
            }),
            Block::Book(Book {
                code: BookCode::from_str("YYY").unwrap(),
                description: Cow::Borrowed(""),
                span: Span::new(0, 7),
            }),
        ]);
        let starts: Vec<u32> = analyze(&document)
            .iter()
            .map(|diagnostic| diagnostic.span.start)
            .collect();
        assert_eq!(starts, vec![0, 40]);
    }
}
