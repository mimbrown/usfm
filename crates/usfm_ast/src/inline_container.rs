use std::borrow::Cow;

use super::{Char, Inline, Note, Para, SPAN, Span, TableCell};

pub trait InlineContainer<'a> {
    fn children(&self) -> &Vec<Inline<'a>>;
    fn children_mut(&mut self) -> &mut Vec<Inline<'a>>;

    fn add_child(&mut self, mut child: Inline<'a>) {
        // Rule 6 on `Text`: leading whitespace after a marker is part of the
        // marker. Two places in a child list start "right after a marker" —
        // the beginning, which follows the container's own marker, and just
        // after a `VerseStart`, which is `\v N` and its space. The parser eats
        // that whitespace as it reads the marker, but not when a marker it
        // then *drops* stands in between (`\p\* n`, `\v 3\* x`: the `\*` ends
        // no milestone, so nothing is left to have eaten the space). Rule 6 is
        // about the tree, so it is enforced here rather than at every site
        // that can drop a node.
        //
        // The cheapest question first: the text almost never starts with
        // whitespace, and one byte answers that without touching the child
        // list at all. A multi-byte character's first byte is never ASCII
        // whitespace, so the byte test and `trim_start_matches` agree
        // (ticket 37).
        if let Inline::Text(text) = &mut child
            && text.as_bytes().first().is_some_and(u8::is_ascii_whitespace)
            && matches!(self.children().last(), None | Some(Inline::VerseStart(_)))
        {
            let trimmed = text.trim_start_matches(|c: char| c.is_ascii_whitespace());
            if trimmed.is_empty() {
                return;
            }
            // The span still covers the run the text was read from; see
            // the note on `Text`.
            text.content = Cow::Owned(trimmed.to_string());
        }
        // If the last child is a text node, and the new child is also a text node, merge them
        let children = self.children_mut();
        if let Some(Inline::Text(prev_text)) = children.last_mut()
            && let Inline::Text(child_text) = child
        {
            // Two runs merge when whatever stood between them left no node —
            // a `\*` with no milestone open, a closing marker with nothing to
            // close. What is left is one run of text, so rule 1 applies to it
            // as to any other: the whitespace that ended the first run and the
            // whitespace that starts the second are one space, not two. A
            // `Text` holding `"a  b"` is one no source could produce and no
            // writer could write back; the round-trip fuzz target found it.
            // The last byte answers it: a character that is ASCII whitespace
            // is one byte, and no other byte of a UTF-8 sequence can be
            // mistaken for one (ticket 37).
            let ends_in_space = prev_text
                .as_bytes()
                .last()
                .is_some_and(u8::is_ascii_whitespace);
            let addition: &str = if ends_in_space {
                child_text.trim_start_matches(|c: char| c.is_ascii_whitespace())
            } else {
                &child_text
            };
            let mut string = String::with_capacity(prev_text.len() + addition.len());
            string.push_str(prev_text);
            string.push_str(addition);
            prev_text.content = Cow::Owned(string);
            // The merged run covers both sources. A synthesized run has no
            // position, so it must not drag the span back to 0.
            prev_text.span = if prev_text.span == SPAN {
                child_text.span
            } else if child_text.span == SPAN {
                prev_text.span
            } else {
                Span::new(prev_text.span.start, child_text.span.end)
            };
            return;
        }
        children.push(child);
    }

    /// Drop trailing ASCII whitespace from a final text child, removing the
    /// child if nothing is left. Not recursive: text inside a trailing
    /// character style ended at that style's closing marker, where trailing
    /// whitespace is content (see rule 5 on `Text`).
    fn trim_trailing_whitespace(&mut self) {
        let children = self.children_mut();
        if let Some(Inline::Text(text)) = children.last_mut() {
            let trimmed = text.trim_end_matches(|c: char| c.is_ascii_whitespace());
            if trimmed.is_empty() {
                children.pop();
            } else if trimmed.len() != text.len() {
                // The span still covers the run this text was read from; see
                // the note on `Text`. Narrowing it here would need the source,
                // since normalised whitespace makes content length and source
                // length differ.
                text.content = Cow::Owned(trimmed.to_string());
            }
        }
    }
}

impl<'a> InlineContainer<'a> for Para<'a> {
    fn children(&self) -> &Vec<Inline<'a>> {
        &self.children
    }

    fn children_mut(&mut self) -> &mut Vec<Inline<'a>> {
        &mut self.children
    }
}

impl<'a> InlineContainer<'a> for Note<'a> {
    fn children(&self) -> &Vec<Inline<'a>> {
        &self.children
    }

    fn children_mut(&mut self) -> &mut Vec<Inline<'a>> {
        &mut self.children
    }
}

impl<'a> InlineContainer<'a> for Char<'a> {
    fn children(&self) -> &Vec<Inline<'a>> {
        &self.children
    }

    fn children_mut(&mut self) -> &mut Vec<Inline<'a>> {
        &mut self.children
    }
}

impl<'a> InlineContainer<'a> for TableCell<'a> {
    fn children(&self) -> &Vec<Inline<'a>> {
        &self.children
    }

    fn children_mut(&mut self) -> &mut Vec<Inline<'a>> {
        &mut self.children
    }
}
