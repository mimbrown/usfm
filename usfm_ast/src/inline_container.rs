use std::borrow::Cow;

use super::{Char, Inline, Note, Para, SPAN, Span, TableCell};

pub trait InlineContainer<'a> {
    fn children(&self) -> &Vec<Inline<'a>>;
    fn children_mut(&mut self) -> &mut Vec<Inline<'a>>;

    fn add_child(&mut self, child: Inline<'a>) {
        // If the last child is a text node, and the new child is also a text node, merge them
        let children = self.children_mut();
        if let Some(Inline::Text(prev_text)) = children.last_mut()
            && let Inline::Text(child_text) = child
        {
            let mut string = String::with_capacity(prev_text.len() + child_text.len());
            string.push_str(prev_text);
            string.push_str(&child_text);
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
