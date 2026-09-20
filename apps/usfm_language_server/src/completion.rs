//! `textDocument/completion`: the markers that may stand where the cursor is
//! (ticket 32).
//!
//! Triggered by the `\` that opens every marker, and answered from the
//! *document's own* stylesheet (hardening plan D3), so a project that ships a
//! `custom.sty` gets its own markers offered and a marker the parser derived
//! (`\k-s`) is offered as readily as one Paratext's sheet names.
//!
//! # What is offered where
//!
//! The filter is ticket 20's placement rule, which now lives in
//! `usfm_semantic::placement::check` — one implementation, read by the check
//! that reports a misplaced marker and by this, which declines to offer one.
//! [`parent`] answers the half the rule needs from the tree: the marker the
//! cursor is written under, by the same three rules `Analyzer::placement_parent`
//! reads off its scopes —
//!
//! * inside a table cell nothing is filtered: cell markers are in no
//!   `OccursUnder` list, so every list would reject them;
//! * inside a note, the note is the parent, whatever character styles stand
//!   between;
//! * inside a character style nothing is filtered either — `NEST` governs
//!   that, not `OccursUnder` — and otherwise the parent is the paragraph.
//!
//! In practice the parent is a paragraph nearly everywhere: the parser opens
//! an implicit `\p` for anything written outside one, so even a `\` typed
//! between blocks is inside a paragraph in the tree, and that is the truth
//! about where the marker being typed would land.
//!
//! Only the rule's **Error** half filters: a marker whose `OccursUnder` is
//! note styles alone (`\fq`, `\fr`, `\xo`) is not offered outside a note,
//! because writing it there is `marker-not-allowed-here`. A marker that is
//! merely *not listed* stays in the list, because that is an Info the parser
//! itself is relaxed about — Paratext accepts `\f` under `\cl` — and a
//! completion list that hid it would be wrong more often than the check is.
//!
//! # The text that is inserted
//!
//! Every item carries a [`TextEdit`] over the marker name **after** the `\`,
//! never over the `\` itself: the user has typed it, and re-inserting it is
//! how a completion turns `\` into `\\p`. The edit covers whatever was typed
//! after it, so `\` and `\n` both complete to `\nd`.
//!
//! A marker that has a closing marker is inserted as a snippet with it —
//! `nd $1\nd*` — so a character style is never left open; a paragraph marker
//! stands alone and is inserted as itself.

use tower_lsp_server::ls_types::{
    CompletionItem, CompletionItemKind, Documentation, InsertTextFormat, TextEdit,
};

use usfm::ast::{Document, NodeRef, StyleId};
use usfm::span::{LineIndex, Span};
use usfm::style::{StyleRule, StyleType};

use crate::convert;
use crate::locate::locate;

/// The markers that may be written at `offset`, or `None` when the cursor is
/// not after a `\` at all.
///
/// `None` and an empty list are different answers: `None` is "this is not a
/// marker position", which leaves the editor's word completion alone, and an
/// empty list would claim there is nothing to write here.
pub fn completions(
    document: &Document<'_>,
    source: &str,
    index: &LineIndex,
    offset: u32,
) -> Option<Vec<CompletionItem>> {
    let backslash = marker_start(source, offset)?;
    // What the item replaces: the marker name typed so far, never the `\`.
    let replaced = convert::range(index, Span::new(backslash + 1, offset));
    let parent = parent(document, backslash).map(|style| document.marker(style).to_owned());

    let sheet = document.style_sheet();
    let items = sheet
        .rules
        .iter()
        .enumerate()
        // A sheet can hold the same marker twice — a project `.sty` amending
        // Paratext's, a style added while parsing — and the map answers with
        // the one in force, so the others are skipped rather than offered as
        // duplicates.
        .filter(|(position, rule)| sheet.get_marker_index(&rule.marker) == Some(position))
        .filter(|(_, rule)| !rule.marker.is_empty())
        .filter(|(_, rule)| match &parent {
            Some(parent) => usfm::semantic::placement::check(sheet, rule, parent).is_allowed(),
            None => true,
        })
        .map(|(_, rule)| item(rule, replaced))
        .collect();
    Some(items)
}

/// The offset of the `\` the cursor is writing a marker after, or `None` when
/// it is not writing one.
///
/// A marker name is letters, digits and the `-` of a milestone's `-s`/`-e`;
/// the `+` of a nested `\+nd` comes straight after the `\`. Scanning back over
/// those from the cursor and finding a `\` is the whole test — which is why an
/// explicitly invoked completion in the middle of `\nd` answers the same as
/// one triggered by the `\` itself.
fn marker_start(source: &str, offset: u32) -> Option<u32> {
    let before = source.get(..offset as usize)?;
    let name = before.trim_end_matches(|c: char| c.is_ascii_alphanumeric() || c == '-');
    let name = name.strip_suffix('+').unwrap_or(name);
    let backslash = name.strip_suffix('\\')?;
    // `\\` is an escaped backslash, not the start of a marker.
    if backslash.ends_with('\\') {
        return None;
    }
    Some(backslash.len() as u32)
}

/// The marker a style written at `offset` would be placed under, or `None`
/// where the placement rule checks nothing.
///
/// The three rules of `usfm_semantic`'s `placement_parent`, read off the path
/// [`locate`] returns rather than off a walk's scopes: a table cell answers
/// `None` however deep the cursor is inside it, and otherwise the innermost
/// container decides — a character style filters nothing, a note is the
/// parent of what it holds, a paragraph is the parent of everything else.
fn parent(document: &Document<'_>, offset: u32) -> Option<StyleId> {
    // The `\` being typed is not in the tree yet (nothing was parsed from it),
    // so the node to ask about is the one it was typed into, which is the node
    // at the character before it.
    let located = locate(document, offset)
        .node
        .or_else(|| locate(document, offset.checked_sub(1)?).node)?;

    let mut innermost = None;
    for depth in (0..=located.path.len()).rev() {
        let node = NodeRef::Document(document).descend(&located.path[..depth])?;
        match node {
            NodeRef::TableCell(_) => return None,
            NodeRef::Char(_) if innermost.is_none() => return None,
            NodeRef::Note(note) if innermost.is_none() => innermost = Some(note.style),
            NodeRef::Para(para) if innermost.is_none() => innermost = Some(para.style),
            _ => {}
        }
    }
    innermost
}

/// One stylesheet entry as a completion item.
///
/// `detail` is the sheet's `\Name` and `documentation` its `\Description`,
/// which is the same pair the hover shows (ticket 31) — the editor puts the
/// first beside the label and the second in the panel behind it.
fn item(rule: &StyleRule, replaced: tower_lsp_server::ls_types::Range) -> CompletionItem {
    let (kind, text) = match insertion(rule) {
        Some(snippet) => (CompletionItemKind::SNIPPET, snippet),
        None => (CompletionItemKind::KEYWORD, rule.marker.clone()),
    };
    CompletionItem {
        label: rule.marker.clone(),
        kind: Some(kind),
        detail: rule.name.clone(),
        documentation: rule.description.clone().map(Documentation::String),
        text_edit: Some(tower_lsp_server::ls_types::CompletionTextEdit::Edit(
            TextEdit {
                range: replaced,
                new_text: text,
            },
        )),
        insert_text_format: Some(match kind {
            CompletionItemKind::SNIPPET => InsertTextFormat::SNIPPET,
            _ => InsertTextFormat::PLAIN_TEXT,
        }),
        ..CompletionItem::default()
    }
}

/// The snippet for a marker that is written with a closing marker, or `None`
/// for one that stands alone.
///
/// Three of the four style types close: a character style with `\nd*`, a note
/// with `\f*`, a milestone with `\*`. `$1` is where the text goes and `$0`
/// where the cursor is left afterwards, which is the protocol's snippet
/// syntax; a note also gets the `+` caller it must have, so the completion
/// does not leave a `missing-note-caller` behind it.
fn insertion(rule: &StyleRule) -> Option<String> {
    let marker = &rule.marker;
    match rule.style_type {
        StyleType::Paragraph => None,
        StyleType::Character => Some(format!("{marker} $1\\{marker}*$0")),
        StyleType::Note => Some(format!("{marker} + $1\\{marker}*$0")),
        // A milestone carries no text, only attributes: `\qt-s |who="God"\*`.
        StyleType::Milestone => Some(format!("{marker}$1\\*$0")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn complete(source: &str, cursor: &str) -> Vec<CompletionItem> {
        let offset =
            (source.find(cursor).expect("the cursor is in the source") + cursor.len()) as u32;
        let result = usfm::parse(source);
        completions(&result.document, source, &LineIndex::new(source), offset)
            .unwrap_or_else(|| panic!("a completion at {offset} in {source:?}"))
    }

    fn labels(items: &[CompletionItem]) -> Vec<&str> {
        items.iter().map(|item| item.label.as_str()).collect()
    }

    fn find<'a>(items: &'a [CompletionItem], label: &str) -> &'a CompletionItem {
        items
            .iter()
            .find(|item| item.label == label)
            .unwrap_or_else(|| panic!("`\\{label}` is offered"))
    }

    fn new_text(item: &CompletionItem) -> &str {
        match item.text_edit.as_ref().expect("an edit") {
            tower_lsp_server::ls_types::CompletionTextEdit::Edit(edit) => &edit.new_text,
            other => panic!("an insert-replace edit: {other:?}"),
        }
    }

    fn replaced(item: &CompletionItem) -> tower_lsp_server::ls_types::Range {
        match item.text_edit.as_ref().expect("an edit") {
            tower_lsp_server::ls_types::CompletionTextEdit::Edit(edit) => edit.range,
            other => panic!("an insert-replace edit: {other:?}"),
        }
    }

    /// The stylesheet's own words come with the marker, and a paragraph
    /// marker is inserted as itself.
    #[test]
    fn a_marker_is_offered_with_the_sheet_s_name_and_description() {
        let items = complete("\\id GEN\n\\c 1\n\\p text \\", "text \\");
        let p = find(&items, "p");
        assert_eq!(
            p.detail.as_deref(),
            Some("p - Paragraph - Normal - First Line Indent"),
        );
        assert!(matches!(
            p.documentation,
            Some(Documentation::String(ref text))
                if text.contains("Paragraph text, with first line indent"),
        ));
        assert_eq!(p.kind, Some(CompletionItemKind::KEYWORD));
        assert_eq!(new_text(p), "p");
        assert_eq!(p.insert_text_format, Some(InsertTextFormat::PLAIN_TEXT));
    }

    /// A character style brings its closing marker, as a snippet.
    #[test]
    fn a_character_style_is_inserted_with_its_closer() {
        let items = complete("\\id GEN\n\\c 1\n\\p text \\", "text \\");
        let nd = find(&items, "nd");
        assert_eq!(nd.kind, Some(CompletionItemKind::SNIPPET));
        assert_eq!(new_text(nd), "nd $1\\nd*$0");
        assert_eq!(nd.insert_text_format, Some(InsertTextFormat::SNIPPET));
        // A note gets its caller with it, and a milestone its `\*`.
        assert_eq!(new_text(find(&items, "f")), "f + $1\\f*$0");
        assert_eq!(new_text(find(&items, "qt-s")), "qt-s$1\\*$0");
    }

    /// The `\` the user typed is not part of what the item replaces, so the
    /// same list serves a bare `\` and a partly typed marker.
    #[test]
    fn the_backslash_is_never_re_inserted() {
        // A bare `\`: the replaced range is empty, right after it.
        let items = complete("\\id GEN\n\\c 1\n\\p text \\", "text \\");
        let range = replaced(find(&items, "nd"));
        assert_eq!(range.start, range.end);
        assert_eq!(range.start.character, "\\p text \\".len() as u32);

        // `\n`: the range covers the `n` alone, so accepting `nd` writes
        // `\nd` and not `\nnd`.
        let source = "\\id GEN\n\\c 1\n\\p text \\n";
        let items = complete(source, "text \\n");
        let range = replaced(find(&items, "nd"));
        assert_eq!(range.start.character, "\\p text \\".len() as u32);
        assert_eq!(range.end.character, "\\p text \\n".len() as u32);
        assert_eq!(new_text(find(&items, "nd")), "nd $1\\nd*$0");
    }

    /// The placement filter: a note-only marker is not offered in a
    /// paragraph, and is offered inside a note.
    #[test]
    fn a_note_only_marker_is_offered_only_inside_a_note() {
        let items = complete("\\id GEN\n\\c 1\n\\p \\v 1 text \\", "text \\");
        assert!(!labels(&items).contains(&"fq"), "`\\fq` is note-only");
        assert!(labels(&items).contains(&"p"));
        assert!(labels(&items).contains(&"nd"));

        // Inside the note, `\fq` is exactly what belongs.
        let items = complete(
            "\\id GEN\n\\c 1\n\\p \\v 1 a\\f + \\ft note \\\\f* b\n",
            "note \\",
        );
        assert!(labels(&items).contains(&"fq"), "inside `\\f`, `\\fq` fits");
    }

    /// Where the rule checks nothing, nothing is filtered: a table cell (no
    /// cell marker is in any `OccursUnder`) and the inside of a character
    /// style (`NEST` governs that, not `OccursUnder`).
    #[test]
    fn a_cell_and_a_character_style_filter_nothing() {
        let items = complete("\\id GEN\n\\c 1\n\\tr \\tc1 text \\", "text \\");
        assert!(labels(&items).contains(&"fq"));

        let items = complete("\\id GEN\n\\c 1\n\\p \\v 1 \\nd text \\", "text \\");
        assert!(labels(&items).contains(&"fq"));
    }

    /// Between blocks the paragraph markers are what is wanted, and they are
    /// offered — the parent there is the implicit `\p` the parser opens for
    /// anything written outside a paragraph, which is the truth about where
    /// the marker would land.
    #[test]
    fn between_blocks_the_paragraph_markers_are_offered() {
        let items = complete("\\id GEN\n\\c 1\n\\", "\\c 1\n\\");
        for marker in ["p", "q1", "s1", "nd"] {
            assert!(labels(&items).contains(&marker), "`\\{marker}`");
        }
        // No marker is offered twice, however often the sheet names it.
        let mut markers = labels(&items);
        markers.sort_unstable();
        let count = markers.len();
        markers.dedup();
        assert_eq!(markers.len(), count);
    }

    /// A `\` in an otherwise empty file still answers. There is no node
    /// around it in the source, but the parser opens an implicit `\p` for
    /// anything written outside a paragraph, so the parent is that `\p` —
    /// which is where the marker would in fact land.
    #[test]
    fn a_lone_backslash_completes_from_the_implicit_paragraph() {
        let items = complete("\\", "\\");
        assert!(labels(&items).contains(&"id"));
        assert!(labels(&items).contains(&"p"));
        assert!(!labels(&items).contains(&"fq"));
    }

    /// Not after a `\`, no completion at all — and an escaped `\\` is text,
    /// not the start of a marker.
    #[test]
    fn a_position_that_is_not_a_marker_is_no_completion() {
        let source = "\\id GEN\n\\c 1\n\\p text more\n";
        let result = usfm::parse(source);
        let offset = source.find("more").unwrap() as u32;
        assert!(completions(&result.document, source, &LineIndex::new(source), offset).is_none());

        let source = "\\id GEN\n\\c 1\n\\p text \\\\";
        let result = usfm::parse(source);
        let index = LineIndex::new(source);
        assert!(
            completions(&result.document, source, &index, source.len() as u32).is_none(),
            "`\\\\` is an escaped backslash",
        );
    }
}
