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
//! over the whole `\id` line, not one for the code; a [`Char`] runs from its
//! opening marker to wherever it closed — the node's span is what is
//! reported: the source text is not available here, and narrowing by
//! arithmetic (`span.start + 4`) would assume a single space after the marker
//! and point at whitespace whenever the input has two. Where the tree does
//! carry the narrower offset the narrower one is used, which is why
//! [`usfm_ast::Attributes`] keeps the `|` and each [`usfm_ast::Attribute`] its
//! name: an attribute diagnostic points at the attribute, exactly as it did
//! when the parser reported it mid-parse.

use usfm_ast::visit::{Visit, walk_document, walk_note, walk_para, walk_table_cell};
use usfm_ast::{
    Attributes, Book, Char, Document, Milestone, Note, Para, Periph, StyleId, TableCell,
    default_attribute_name, is_valid_attribute_name,
};
use usfm_diagnostics::{Code, Diagnostic};
use usfm_span::Span;
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
/// many there are. Each one is a private method named after the [`Code`] it
/// reports, or after the rule where one rule chooses between codes
/// (`check_placement`, `check_attributes`), so the mapping from the recovery
/// table to the code that implements it stays easy to follow.
///
/// [`Scope`] is the only state besides the diagnostics: what the walk is
/// inside, which is what the parser read off its stack of open markers.
struct Analyzer<'a> {
    /// The sheet the document's `StyleId`s resolve against — the parser's
    /// extended with anything it had to derive (hardening plan D3), never
    /// `DEFAULT_STYLESHEET`.
    style_sheet: &'a StyleSheet,
    /// What the walk is inside, innermost last; see [`Scope`].
    scopes: Vec<Scope>,
    diagnostics: Vec<Diagnostic>,
}

/// One level of the containment the placement check reads.
///
/// Only the four kinds of container that change the answer are on the stack.
/// Blocks that hold blocks (a sidebar, a periph) are not: whatever they
/// contain is in a paragraph of its own, which is the parent that counts.
#[derive(Clone, Copy, PartialEq)]
enum Scope {
    /// A paragraph: the parent of the notes and of the outermost character
    /// styles it holds.
    Para(StyleId),
    /// A character style. Its content is governed by `NEST` rather than by
    /// `OccursUnder`, so a character style inside one is not checked.
    Char,
    /// A note: the parent of the character styles inside it, and transparent
    /// to a note, whose parent is the paragraph either way.
    Note(StyleId),
    /// A table cell, whose content is not placement-checked at all: the cell
    /// markers are not in any `OccursUnder` list.
    Cell,
}

impl<'a> Analyzer<'a> {
    fn new(style_sheet: &'a StyleSheet) -> Self {
        Self {
            style_sheet,
            scopes: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn emit(&mut self, code: Code, span: Span, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic::new(code, span, message));
    }

    fn in_scope(&mut self, scope: Scope, f: impl FnOnce(&mut Self)) {
        self.scopes.push(scope);
        f(self);
        self.scopes.pop();
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

    /// The parent a character style or note is placed under, or `None` where
    /// nothing is checked.
    ///
    /// The three rules the parser applied to its open-marker stack, read off
    /// the tree instead (ticket 20):
    ///
    /// * inside a table cell nothing is checked. The cell markers occur in no
    ///   `OccursUnder` list, so every style in a table would be reported;
    /// * a note's parent is its paragraph, whatever character styles or notes
    ///   stand between. `\f` inside `\wj` is still `\f` under `\p`;
    /// * a character style inside another character style is not checked
    ///   here at all — `NEST` decides that, and the parser reports
    ///   `character-style-nested-without-plus` for it. Inside a note it is
    ///   the note; otherwise the paragraph.
    fn placement_parent(&self, is_note: bool) -> Option<StyleId> {
        if self.scopes.contains(&Scope::Cell) {
            return None;
        }
        if is_note {
            return self.scopes.iter().rev().find_map(|scope| match scope {
                Scope::Para(style) => Some(*style),
                _ => None,
            });
        }
        match self.scopes.last() {
            Some(Scope::Char) => None,
            Some(Scope::Note(style) | Scope::Para(style)) => Some(*style),
            Some(Scope::Cell) | None => None,
        }
    }

    /// `marker-not-allowed-here` / `marker-not-listed-here`: a character style
    /// or note whose stylesheet entry does not list the marker it sits under.
    ///
    /// The two codes are one rule with two severities. A marker whose whole
    /// `OccursUnder` list is note styles (`\xq`, `\fr`, `\xo` …) exists only
    /// inside a note, so anywhere else is an error and Paratext marks it
    /// `status="invalid"`. Every other list is advisory — Paratext accepts
    /// `\f` under `\cl`, which `\f` does not list — so it only informs.
    ///
    /// The span is the node's, which runs from the opening marker to wherever
    /// the style closed; the parser reported the opening marker alone. Both
    /// start at the same offset (see the note on spans above).
    fn check_placement(&mut self, style: StyleId, span: Span, is_note: bool) {
        let Some(parent) = self.placement_parent(is_note) else {
            return;
        };
        let sheet = self.style_sheet;
        let rule = sheet.get_rule(style.index());
        if rule.occurs_under.is_empty() {
            return;
        }
        let parent_name = &sheet.get_rule(parent.index()).marker;
        if rule.occurs_under.contains(parent_name) {
            return;
        }
        let note_only = rule.occurs_under.iter().all(|allowed| {
            sheet
                .get_rule_by_marker(allowed)
                .is_some_and(|allowed| allowed.is_note())
        });
        let (code, message) = if note_only {
            (
                Code::MarkerNotAllowedHere,
                format!("`\\{}` cannot occur under `\\{parent_name}`", rule.marker),
            )
        } else {
            (
                Code::MarkerNotListedHere,
                format!(
                    "`\\{}` is not listed as occurring under `\\{parent_name}`",
                    rule.marker
                ),
            )
        };
        self.emit(code, span, message);
    }

    /// The attribute list of a `\w`, a milestone or a `\periph` line against
    /// the marker that carries it. Nothing is repaired: the list is in the
    /// tree exactly as written whatever is reported here, which is what puts
    /// all six codes on this side of the split. The ones that decide what the
    /// list *is* — an unquoted value, a missing one, a line break inside the
    /// list — stay with the parser, which had to choose.
    ///
    /// `attributes` is `Some` iff the source had a `|`, so an empty list is
    /// `|` with nothing after it and not a marker written without one.
    fn check_attributes(&mut self, style: StyleId, attributes: &Attributes<'_>) {
        let sheet = self.style_sheet;
        let rule = sheet.get_rule(style.index());
        if attributes.pairs.is_empty() {
            // A milestone is nothing but its attributes, so `\ts-s |\*` says
            // the same as `\ts-s\*` and nothing was lost; unfoldingWord's
            // aligned texts write their translation sections that way. On a
            // character style the `|` announces a value that is missing.
            let code = if rule.is_milestone() {
                Code::EmptyMilestoneAttributeList
            } else {
                Code::EmptyAttributeList
            };
            self.emit(code, attributes.pipe, "`|` is not followed by any attribute");
            return;
        }
        for (index, pair) in attributes.pairs.iter().enumerate() {
            // Both of these keep the pair in the tree; it is the USX and HTML
            // writers that drop it, having no way to write it.
            if !pair.name.is_empty() && !is_valid_attribute_name(&pair.name) {
                self.emit(
                    Code::MalformedAttributeName,
                    pair.span,
                    format!(
                        "`{}` is not an attribute name; it is kept in the tree but \
                         cannot be written to USX",
                        pair.name
                    ),
                );
            }
            if attributes.pairs[..index]
                .iter()
                .any(|earlier| earlier.name == pair.name)
            {
                let message = if pair.name.is_empty() {
                    "the default attribute is given more than once".to_string()
                } else {
                    format!("`{}` is given more than once", pair.name)
                };
                self.emit(Code::DuplicateAttribute, pair.span, message);
            }
        }
        if !attributes.pairs.iter().any(|pair| pair.name.is_empty()) {
            return;
        }
        // `rule` is borrowed from the sheet, not from `self`, so it survives
        // the `emit` calls without a clone.
        let marker = &rule.marker;
        if default_attribute_name(marker).is_none() {
            self.emit(
                Code::NoDefaultAttribute,
                attributes.pipe,
                format!("`\\{marker}` has no default attribute; the value needs a name"),
            );
        }
        if attributes.pairs.len() > 1 {
            self.emit(
                Code::DefaultAttributeWithOthers,
                attributes.pipe,
                "a bare value must be the only attribute; give it a name",
            );
        }
    }
}

impl Visit for Analyzer<'_> {
    fn visit_document(&mut self, document: &Document<'_>) {
        walk_document(self, document);
    }

    fn visit_book(&mut self, book: &Book<'_>) {
        self.check_unlisted_book_code(book);
    }

    fn visit_para(&mut self, para: &Para<'_>) {
        self.in_scope(Scope::Para(para.style), |analyzer| walk_para(analyzer, para));
    }

    fn visit_table_cell(&mut self, cell: &TableCell<'_>) {
        self.in_scope(Scope::Cell, |analyzer| walk_table_cell(analyzer, cell));
    }

    fn visit_char(&mut self, char: &Char<'_>) {
        self.check_placement(char.style, char.span, false);
        if let Some(attributes) = &char.attributes {
            self.check_attributes(char.style, attributes);
        }
        // Not `walk_char`: it visits the attribute list after the children,
        // and this pass has just read that list itself.
        self.in_scope(Scope::Char, |analyzer| {
            for inline in &char.children {
                analyzer.visit_inline(inline);
            }
        });
    }

    fn visit_note(&mut self, note: &Note<'_>) {
        self.check_placement(note.style, note.span, true);
        self.in_scope(Scope::Note(note.style), |analyzer| walk_note(analyzer, note));
    }

    fn visit_milestone(&mut self, milestone: &Milestone<'_>) {
        if let Some(attributes) = &milestone.attributes {
            self.check_attributes(milestone.style, attributes);
        }
    }

    fn visit_periph(&mut self, periph: &Periph<'_>) {
        if let Some(attributes) = &periph.attributes {
            self.check_attributes(periph.style, attributes);
        }
        for block in &periph.blocks {
            self.visit_block(block);
        }
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
