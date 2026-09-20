//! `textDocument/codeAction`: the quick fixes for the repairs the toolchain
//! reports (ticket 32).
//!
//! One rule per [`Code`], and only where the edit is *obvious* — where the
//! author's intention is not in doubt and the fix is what the parser already
//! did to the tree, written back into the file. The fix for a dropped marker
//! is to drop it in the text too; the fix for a style left open is the closing
//! marker the parser supplied; the fix for a note with no caller is the `+`
//! the parser assumed.
//!
//! Five codes have one (the list ticket 32 names):
//!
//! | code | the edit |
//! |---|---|
//! | `unknown-marker` | delete the marker, and the space after it |
//! | `character-style-not-closed` | insert the closing marker where the style ended |
//! | `missing-note-caller` | insert the `+` the parser assumed |
//! | `attribute-value-not-quoted` | put the value in double quotes |
//! | `empty-milestone-attribute-list` | delete the `\|`, and the space before it |
//!
//! Every other code is left alone, and deliberately: `missing-id` would have
//! to invent a book code, `verse-out-of-order` a renumbering of the chapter,
//! `marker-not-allowed-here` a decision about which of the two markers the
//! author meant. Those are edits for a person, not for a lightbulb.
//!
//! # Where the edit comes from
//!
//! The diagnostic's span and the source, with the tree consulted for the one
//! thing a span cannot say. Each fix is computed here and nowhere else, and
//! each has a test that applies it and parses the result: **the code must be
//! gone and no new code may appear**, which is the only definition of "fixed"
//! that does not need a human to read the output.
//!
//! The one that needs the tree is `character-style-not-closed`, whose span is
//! the *opening* marker alone. The closer goes at the end of the style's
//! content — the end of its last child, with the whitespace the container's
//! own end swept into it trimmed off, because the node's span runs to
//! wherever the style was closed and that is sometimes past an outer closing
//! marker (`\em a \+nd b\em*`, where `\+nd`'s span ends after `\em*`; the
//! closer belongs before it).

use usfm::Code;
use usfm::ast::{Document, NodeRef};
use usfm::diagnostics::Diagnostic;
use usfm::span::Span;

use crate::locate::{locate, span_of};

/// One quick fix: what to call it, and the edits that make it.
#[derive(Debug, PartialEq)]
pub struct Fix {
    pub title: String,
    /// The edits, each a span of the source replaced by a text. They never
    /// overlap, and the protocol takes them in any order.
    pub edits: Vec<Edit>,
}

#[derive(Debug, PartialEq)]
pub struct Edit {
    pub span: Span,
    pub text: String,
}

impl Edit {
    fn replace(span: Span, text: impl Into<String>) -> Self {
        Self {
            span,
            text: text.into(),
        }
    }

    fn insert(at: u32, text: impl Into<String>) -> Self {
        Self::replace(Span::empty(at), text)
    }

    fn delete(span: Span) -> Self {
        Self::replace(span, "")
    }
}

/// The quick fix for `diagnostic`, or `None` where there is no obvious one.
///
/// `source` is the text the document was parsed from; the spans of the
/// diagnostic and of the tree both index into it.
pub fn fix(document: &Document<'_>, source: &str, diagnostic: &Diagnostic) -> Option<Fix> {
    let span = diagnostic.span;
    if span.end as usize > source.len() {
        // A diagnostic about a synthesized node, or about a document the text
        // has moved on from: there is nothing to edit.
        return None;
    }
    match diagnostic.code {
        Code::UnknownMarker => delete_marker(source, span),
        Code::CharacterStyleNotClosed => close_style(document, source, span),
        Code::MissingNoteCaller => add_caller(source, span),
        Code::AttributeValueNotQuoted => quote_value(source, span),
        Code::EmptyMilestoneAttributeList => delete_pipe(source, span),
        _ => None,
    }
}

/// `unknown-marker`: take the marker out, as the parser did.
///
/// The whitespace after it goes too, so the text on either side joins into
/// the one run the tree already holds — `a \foo b` parses to `a b`, and the
/// fix writes exactly that. Only spaces and tabs: a line break is a
/// paragraph's business, not this marker's.
fn delete_marker(source: &str, span: Span) -> Option<Fix> {
    let marker = source.get(span.start as usize..span.end as usize)?;
    if !marker.starts_with('\\') {
        return None;
    }
    let end = span.end + horizontal_whitespace_after(source, span.end);
    Some(Fix {
        title: format!("Delete `{marker}`"),
        edits: vec![Edit::delete(Span::new(span.start, end))],
    })
}

/// `character-style-not-closed`: write the closing marker the parser assumed.
///
/// The closer is spelled from the source's own opening marker — `\+nd` closes
/// with `\+nd*`, not with `\nd*` — so the fix never has to decide how the
/// style was nested.
fn close_style(document: &Document<'_>, source: &str, span: Span) -> Option<Fix> {
    let marker = source.get(span.start as usize..span.end as usize)?;
    if !marker.starts_with('\\') {
        return None;
    }
    let node = locate(document, span.start).node?.node;
    let char = match node {
        NodeRef::Char(char) if char.span.start == span.start => char,
        _ => return None,
    };

    // The end of the style's content. The node's own span runs to wherever
    // the style was closed, which may be past a closing marker that belongs
    // to something else; its last child ends where the text does.
    let content = char
        .children
        .last()
        .map(|child| span_of(NodeRef::from_inline(child)).end)
        .unwrap_or(span.end)
        .max(span.end);
    let at = trim_end(source, span.end, content);
    Some(Fix {
        title: format!("Close `{marker}` with `{marker}*`"),
        edits: vec![Edit::insert(at, format!("{marker}*"))],
    })
}

/// `missing-note-caller`: write the `+` the parser assumed.
///
/// `+` is the caller USFM's own examples use and the one the parser puts in
/// the tree, so this is the repair written down rather than a choice of ours.
fn add_caller(source: &str, span: Span) -> Option<Fix> {
    let marker = source.get(span.start as usize..span.end as usize)?;
    if !marker.starts_with('\\') {
        return None;
    }
    // The caller is a word of its own: a space before it always, and one
    // after it unless the source already has whitespace there.
    let followed_by_space = source[span.end as usize..]
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_whitespace());
    let text = if followed_by_space { " +" } else { " + " };
    Some(Fix {
        title: format!("Add the `+` caller to `{marker}`"),
        edits: vec![Edit::insert(span.end, text)],
    })
}

/// `attribute-value-not-quoted`: put the value in double quotes.
///
/// A value that already holds a `"` is left alone: quoting it would change
/// where the value ends, and USFM has no escape to write one with.
fn quote_value(source: &str, span: Span) -> Option<Fix> {
    let value = source.get(span.start as usize..span.end as usize)?;
    if value.contains('"') {
        return None;
    }
    Some(Fix {
        title: format!("Put `{value}` in quotes"),
        edits: vec![Edit::replace(span, format!("\"{value}\""))],
    })
}

/// `empty-milestone-attribute-list`: take the `|` out.
///
/// `\ts-s |\*` and `\ts-s\*` are the same milestone — there is nothing after
/// the `|` to lose — so the fix is the shorter spelling. The space before the
/// `|` goes with it, because that is how USFM 3 spells the list (`\qt-s
/// |who="God"\*`) and leaving it would put a space before the `\*`.
fn delete_pipe(source: &str, span: Span) -> Option<Fix> {
    if source.get(span.start as usize..span.end as usize)? != "|" {
        return None;
    }
    let start = span.start - horizontal_whitespace_before(source, span.start);
    Some(Fix {
        title: "Delete the empty attribute list".to_owned(),
        edits: vec![Edit::delete(Span::new(start, span.end))],
    })
}

/// How many bytes of spaces and tabs follow `at`.
fn horizontal_whitespace_after(source: &str, at: u32) -> u32 {
    source[at as usize..]
        .bytes()
        .take_while(|byte| matches!(byte, b' ' | b'\t'))
        .count() as u32
}

/// How many bytes of spaces and tabs come before `at`.
fn horizontal_whitespace_before(source: &str, at: u32) -> u32 {
    source[..at as usize]
        .bytes()
        .rev()
        .take_while(|byte| matches!(byte, b' ' | b'\t'))
        .count() as u32
}

/// `end` moved back over any ASCII whitespace, but never before `floor`.
///
/// A `Text` run's span covers the line break that ended it, so the end of a
/// style's content is where its last non-blank byte is — which is where a
/// closing marker has to go if it is not to land on the line after.
fn trim_end(source: &str, floor: u32, end: u32) -> u32 {
    let mut end = end.max(floor);
    while end > floor && source.as_bytes()[end as usize - 1].is_ascii_whitespace() {
        end -= 1;
    }
    end
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `source` with `fix` applied, edits taken from the back so the earlier
    /// spans still index what they did.
    fn apply(source: &str, fix: &Fix) -> String {
        let mut edits: Vec<&Edit> = fix.edits.iter().collect();
        edits.sort_by_key(|edit| std::cmp::Reverse(edit.span.start));
        let mut text = source.to_owned();
        for edit in edits {
            text.replace_range(edit.span.start as usize..edit.span.end as usize, &edit.text);
        }
        text
    }

    fn codes(source: &str) -> Vec<Code> {
        usfm::parse(source)
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect()
    }

    /// Fix the first `code` in `source` and hand back what the file becomes.
    ///
    /// The assertion every fix has to pass: the re-parse reports that code
    /// one time less — none at all where the input had one, which is every
    /// case here but the document with two unknown markers, where fixing one
    /// leaves the other for its own action — and reports nothing the first
    /// parse did not.
    fn fixed(code: Code, source: &str) -> String {
        let result = usfm::parse(source);
        let diagnostic = result
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == code)
            .unwrap_or_else(|| panic!("{code} in {source:?}, got {:?}", codes(source)));
        let fix = fix(&result.document, source, diagnostic)
            .unwrap_or_else(|| panic!("a fix for {code} in {source:?}"));

        let fixed = apply(source, &fix);
        let before = codes(source);
        let after = codes(&fixed);
        let count = |codes: &[Code]| codes.iter().filter(|each| **each == code).count();
        assert_eq!(
            count(&after),
            count(&before) - 1,
            "{code} survived the fix: {source:?} -> {fixed:?} ({after:?})",
        );
        let gained: Vec<Code> = after
            .iter()
            .copied()
            .filter(|code| !before.contains(code))
            .collect();
        assert!(
            gained.is_empty(),
            "the fix for {code} gained {gained:?}: {source:?} -> {fixed:?}",
        );
        fixed
    }

    #[test]
    fn an_unknown_marker_is_deleted_with_the_space_after_it() {
        assert_eq!(
            fixed(
                Code::UnknownMarker,
                "\\id GEN\n\\c 1\n\\p \\v 1 a \\foo b\n"
            ),
            "\\id GEN\n\\c 1\n\\p \\v 1 a b\n",
        );
        // A closing marker of the same unknown style is one too.
        assert_eq!(
            fixed(
                Code::UnknownMarker,
                "\\id GEN\n\\c 1\n\\p \\v 1 a \\foo custom\\foo* b\n",
            ),
            "\\id GEN\n\\c 1\n\\p \\v 1 a custom\\foo* b\n",
        );
        // On a line of its own, with nothing after it.
        assert_eq!(
            fixed(Code::UnknownMarker, "\\id GEN\n\\c 1\n\\qqq\n\\p \\v 1 a\n"),
            "\\id GEN\n\\c 1\n\n\\p \\v 1 a\n",
        );
    }

    #[test]
    fn an_open_character_style_gets_its_closer() {
        // Closed by the next paragraph: the closer goes after the text, not
        // after the line break the text run's span swept up.
        assert_eq!(
            fixed(
                Code::CharacterStyleNotClosed,
                "\\id GEN\n\\c 1\n\\p \\v 1 a \\em b\n\\p \\v 2 c\n",
            ),
            "\\id GEN\n\\c 1\n\\p \\v 1 a \\em b\\em*\n\\p \\v 2 c\n",
        );
        // Closed by the end of the file.
        assert_eq!(
            fixed(
                Code::CharacterStyleNotClosed,
                "\\id GEN\n\\c 1\n\\p \\v 1 a \\em b",
            ),
            "\\id GEN\n\\c 1\n\\p \\v 1 a \\em b\\em*",
        );
        // Closed by the next cell.
        assert_eq!(
            fixed(
                Code::CharacterStyleNotClosed,
                "\\id GEN\n\\c 1\n\\tr \\tc1 \\em a \\tc2 b\n",
            ),
            "\\id GEN\n\\c 1\n\\tr \\tc1 \\em a\\em* \\tc2 b\n",
        );
        // Closed by an outer closing marker: the nested style's own closer
        // goes *before* it, and it is spelled `\+nd*` as the source spelled
        // the opener.
        assert_eq!(
            fixed(
                Code::CharacterStyleNotClosed,
                "\\id GEN\n\\c 1\n\\p \\v 1 \\em a \\+nd b\\em* c\n",
            ),
            "\\id GEN\n\\c 1\n\\p \\v 1 \\em a \\+nd b\\+nd*\\em* c\n",
        );
    }

    #[test]
    fn a_note_without_a_caller_gets_the_plus() {
        assert_eq!(
            fixed(
                Code::MissingNoteCaller,
                "\\id GEN\n\\c 1\n\\p \\v 1 a\\f \\ft note\\f* b\n",
            ),
            "\\id GEN\n\\c 1\n\\p \\v 1 a\\f + \\ft note\\f* b\n",
        );
    }

    #[test]
    fn an_unquoted_attribute_value_is_quoted() {
        assert_eq!(
            fixed(
                Code::AttributeValueNotQuoted,
                "\\id GEN\n\\c 1\n\\p \\v 1 \\w word|lemma=grace\\w*\n",
            ),
            "\\id GEN\n\\c 1\n\\p \\v 1 \\w word|lemma=\"grace\"\\w*\n",
        );
    }

    #[test]
    fn an_empty_milestone_attribute_list_is_deleted_with_its_space() {
        assert_eq!(
            fixed(
                Code::EmptyMilestoneAttributeList,
                "\\id GEN\n\\c 1\n\\p \\v 1 a \\ts-s |\\* b\n",
            ),
            "\\id GEN\n\\c 1\n\\p \\v 1 a \\ts-s\\* b\n",
        );
        // On a milestone the sheet does not define, whose own diagnostic
        // stays: the fix takes the list and leaves that alone.
        assert_eq!(
            fixed(
                Code::EmptyMilestoneAttributeList,
                "\\id GEN\n\\c 1\n\\p \\v 1 a \\zaln-s |\\* b\n",
            ),
            "\\id GEN\n\\c 1\n\\p \\v 1 a \\zaln-s\\* b\n",
        );
    }

    /// The title is what the lightbulb menu shows, so it names the marker it
    /// is about.
    #[test]
    fn a_fix_is_titled_after_what_it_edits() {
        let source = "\\id GEN\n\\c 1\n\\p \\v 1 a \\foo b\n";
        let result = usfm::parse(source);
        let diagnostic = &result.diagnostics[0];
        assert_eq!(
            fix(&result.document, source, diagnostic)
                .expect("a fix")
                .title,
            "Delete `\\foo`",
        );
    }

    /// A code with no obvious edit has no action rather than a guessed one.
    #[test]
    fn a_code_without_an_obvious_edit_has_no_fix() {
        let source = "\\c 1\n\\p \\v 1 a\n";
        let result = usfm::parse(source);
        let missing_id = result
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == Code::MissingId)
            .expect("`missing-id`");
        assert_eq!(fix(&result.document, source, missing_id), None);
    }

    /// A diagnostic from another text — an editor that changed the file
    /// between the parse and the request — edits nothing.
    #[test]
    fn a_span_past_the_end_of_the_source_has_no_fix() {
        let source = "\\id GEN\n\\c 1\n\\p \\v 1 a \\foo b\n";
        let result = usfm::parse(source);
        let mut diagnostic = Diagnostic::new(
            Code::UnknownMarker,
            Span::new(900, 904),
            "unknown marker `\\foo`",
        );
        assert_eq!(fix(&result.document, source, &diagnostic), None);
        // And one that points at text which is not a marker at all: `GEN`.
        diagnostic.span = Span::new(4, 7);
        assert_eq!(fix(&result.document, source, &diagnostic), None);
    }
}
