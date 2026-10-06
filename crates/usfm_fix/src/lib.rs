//! Fixes for what the toolchain reports: for a [`Diagnostic`], the edit to the
//! source that makes it go away.
//!
//! This is the one place a fix is computed. `usfm fix` applies them over
//! files from the command line, and the language server's
//! `textDocument/codeAction` hands the same [`Fix`] to an editor (ticket 32
//! wrote them there; they moved here on 2026-10-06 so both could use them).
//!
//! One rule per [`Code`], and only where the edit is *obvious* — where the
//! author's intention is not in doubt and the fix is what the parser already
//! did to the tree, written back into the file. The fix for a dropped marker
//! is to drop it in the text too; the fix for a style left open is the closing
//! marker the parser supplied; the fix for a note with no caller is the `+`
//! the parser assumed.
//!
//! Five codes have one ([`FIXABLE`]):
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
//! author meant. Those are edits for a person.
//!
//! # Where the edit comes from
//!
//! The diagnostic's span and the source, with the tree consulted for the one
//! thing a span cannot say. Each fix has a test that applies it and parses
//! the result: **the code must be gone and no new code may appear**, which is
//! the only definition of "fixed" that does not need a human to read the
//! output.
//!
//! The one that needs the tree is `character-style-not-closed`, whose span is
//! the *opening* marker alone. The closer goes at the end of the style's
//! content — the end of its last child, with the whitespace the container's
//! own end swept into it trimmed off, because the node's span runs to
//! wherever the style was closed and that is sometimes past an outer closing
//! marker (`\em a \+nd b\em*`, where `\+nd`'s span ends after `\em*`; the
//! closer belongs before it).
//!
//! # Applying several
//!
//! [`fixes`] collects the fixes for a parse's diagnostics and [`apply`]
//! writes them into the source in one pass, leaving out any whose edits
//! overlap one already taken. A fix can uncover another diagnostic (and a
//! skipped one is still there), so a caller that wants a file fixed parses
//! the result and asks again until nothing is left; this crate does not
//! parse.

use usfm_ast::visit::{self, Visit};
use usfm_ast::{Char, Document, Inline};
use usfm_diagnostics::{Code, Diagnostic};
use usfm_span::Span;

/// The codes that have a fix.
pub const FIXABLE: &[Code] = &[
    Code::UnknownMarker,
    Code::CharacterStyleNotClosed,
    Code::MissingNoteCaller,
    Code::AttributeValueNotQuoted,
    Code::EmptyMilestoneAttributeList,
];

/// Whether `code` is one of [`FIXABLE`]. A fixable code's diagnostic may
/// still have no fix where the edit is not obvious (an unquoted value that
/// holds a `"`).
pub fn is_fixable(code: Code) -> bool {
    FIXABLE.contains(&code)
}

/// One fix: the diagnostic it answers, what to call it, and the edits that
/// make it.
#[derive(Debug, Clone, PartialEq)]
pub struct Fix {
    /// The code of the diagnostic this fixes.
    pub code: Code,
    /// That diagnostic's span.
    pub span: Span,
    pub title: String,
    /// The edits, each a span of the source replaced by a text. They never
    /// overlap, and the protocol takes them in any order.
    pub edits: Vec<Edit>,
}

#[derive(Debug, Clone, PartialEq)]
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

/// The fix for `diagnostic`, or `None` where there is no obvious one.
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
    let (title, edits) = match diagnostic.code {
        Code::UnknownMarker => delete_marker(source, span),
        Code::CharacterStyleNotClosed => close_style(document, source, span),
        Code::MissingNoteCaller => add_caller(source, span),
        Code::AttributeValueNotQuoted => quote_value(source, span),
        Code::EmptyMilestoneAttributeList => delete_pipe(source, span),
        _ => None,
    }?;
    Some(Fix {
        code: diagnostic.code,
        span,
        title,
        edits,
    })
}

/// The fixes for `diagnostics`, in their order: every one that has a fix,
/// or with `only`, every one whose code is listed.
pub fn fixes(
    document: &Document<'_>,
    source: &str,
    diagnostics: &[Diagnostic],
    only: Option<&[Code]>,
) -> Vec<Fix> {
    diagnostics
        .iter()
        .filter(|diagnostic| only.is_none_or(|codes| codes.contains(&diagnostic.code)))
        .filter_map(|diagnostic| fix(document, source, diagnostic))
        .collect()
}

/// `source` with `fixes` written into it, and which of them were: the
/// indices into `fixes`, ascending. A fix whose edits overlap those of one
/// already taken is left out — the earlier in the source wins — and is
/// there to be found again when the result is parsed.
pub fn apply(source: &str, fixes: &[Fix]) -> (String, Vec<usize>) {
    let extent = |fix: &Fix| {
        let start = fix.edits.iter().map(|edit| edit.span.start).min();
        let end = fix.edits.iter().map(|edit| edit.span.end).max();
        start.zip(end)
    };
    let mut order: Vec<(usize, u32, u32)> = fixes
        .iter()
        .enumerate()
        .filter_map(|(index, fix)| extent(fix).map(|(start, end)| (index, start, end)))
        .filter(|&(_, _, end)| end as usize <= source.len())
        .collect();
    order.sort_by_key(|&(index, start, _)| (start, index));

    let mut taken: Vec<usize> = Vec::new();
    let mut edits: Vec<&Edit> = Vec::new();
    // The end of the last fix taken. Two insertions at one place would
    // have no order of their own, so touching counts as overlapping.
    let mut covered: Option<u32> = None;
    for (index, start, end) in order {
        if covered.is_some_and(|covered| start <= covered) {
            continue;
        }
        covered = Some(end);
        taken.push(index);
        edits.extend(&fixes[index].edits);
    }

    // From the back, so the earlier spans still index what they did.
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.span.start));
    let mut text = source.to_owned();
    for edit in edits {
        text.replace_range(edit.span.start as usize..edit.span.end as usize, &edit.text);
    }
    taken.sort_unstable();
    (text, taken)
}

/// A fix before it knows which diagnostic it answers.
type Found = Option<(String, Vec<Edit>)>;

/// `unknown-marker`: take the marker out, as the parser did.
///
/// The whitespace after it goes too, so the text on either side joins into
/// the one run the tree already holds — `a \foo b` parses to `a b`, and the
/// fix writes exactly that. Only spaces and tabs: a line break is a
/// paragraph's business, not this marker's.
fn delete_marker(source: &str, span: Span) -> Found {
    let marker = source.get(span.start as usize..span.end as usize)?;
    if !marker.starts_with('\\') {
        return None;
    }
    let end = span.end + horizontal_whitespace_after(source, span.end);
    Some((
        format!("Delete `{marker}`"),
        vec![Edit::delete(Span::new(span.start, end))],
    ))
}

/// `character-style-not-closed`: write the closing marker the parser assumed.
///
/// The closer is spelled from the source's own opening marker — `\+nd` closes
/// with `\+nd*`, not with `\nd*` — so the fix never has to decide how the
/// style was nested.
fn close_style(document: &Document<'_>, source: &str, span: Span) -> Found {
    let marker = source.get(span.start as usize..span.end as usize)?;
    if !marker.starts_with('\\') {
        return None;
    }
    // The end of the style's content. The node's own span runs to wherever
    // the style was closed, which may be past a closing marker that belongs
    // to something else; its last child ends where the text does.
    let mut find = ContentEnd {
        start: span.start,
        found: None,
    };
    find.visit_document(document);
    let content = find.found?.unwrap_or(span.end).max(span.end);
    let at = trim_end(source, span.end, content);
    Some((
        format!("Close `{marker}` with `{marker}*`"),
        vec![Edit::insert(at, format!("{marker}*"))],
    ))
}

/// `missing-note-caller`: write the `+` the parser assumed.
///
/// `+` is the caller USFM's own examples use and the one the parser puts in
/// the tree, so this is the repair written down rather than a choice of ours.
fn add_caller(source: &str, span: Span) -> Found {
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
    Some((
        format!("Add the `+` caller to `{marker}`"),
        vec![Edit::insert(span.end, text)],
    ))
}

/// `attribute-value-not-quoted`: put the value in double quotes.
///
/// A value that already holds a `"` is left alone: quoting it would change
/// where the value ends, and USFM has no escape to write one with.
fn quote_value(source: &str, span: Span) -> Found {
    let value = source.get(span.start as usize..span.end as usize)?;
    if value.contains('"') {
        return None;
    }
    Some((
        format!("Put `{value}` in quotes"),
        vec![Edit::replace(span, format!("\"{value}\""))],
    ))
}

/// `empty-milestone-attribute-list`: take the `|` out.
///
/// `\ts-s |\*` and `\ts-s\*` are the same milestone — there is nothing after
/// the `|` to lose — so the fix is the shorter spelling. The space before the
/// `|` goes with it, because that is how USFM 3 spells the list (`\qt-s
/// |who="God"\*`) and leaving it would put a space before the `\*`.
fn delete_pipe(source: &str, span: Span) -> Found {
    if source.get(span.start as usize..span.end as usize)? != "|" {
        return None;
    }
    let start = span.start - horizontal_whitespace_before(source, span.start);
    Some((
        "Delete the empty attribute list".to_owned(),
        vec![Edit::delete(Span::new(start, span.end))],
    ))
}

/// Finds the character style that opens at `start` and where its last child
/// ends: `Some(None)` for a style with no children.
struct ContentEnd {
    start: u32,
    found: Option<Option<u32>>,
}

impl Visit for ContentEnd {
    fn visit_char(&mut self, char: &Char<'_>) {
        if self.found.is_some() {
            return;
        }
        if char.span.start == self.start {
            self.found = Some(char.children.last().map(|child| inline_span(child).end));
            return;
        }
        visit::walk_char(self, char);
    }
}

fn inline_span(inline: &Inline<'_>) -> Span {
    match inline {
        Inline::Text(text) => text.span,
        Inline::VerseStart(verse) => verse.span,
        Inline::VerseEnd(verse) => verse.span,
        Inline::Char(char) => char.span,
        Inline::Note(note) => note.span,
        Inline::Milestone(milestone) => milestone.span,
        Inline::OptBreak(brk) => brk.span,
    }
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
