use super::{Attributes, Block, Span, StyleId, Text};

/// A peripheral division: `\periph Title|id="x"` and everything up to the
/// next `\periph` or the end of the book.
///
/// Like [`Sidebar`](super::Sidebar) it is a block container: front and back
/// matter (title page, preface, glossary) is divided into these, and USX
/// writes each as `<periph alt="Title" id="x">` around its paragraphs.
#[derive(Debug, PartialEq)]
pub struct Periph<'a> {
    /// The `\periph` style, resolved against the document's stylesheet.
    pub style: StyleId,
    /// The division's title, the text before `|`; `None` when empty.
    pub title: Option<Text<'a>>,
    /// Attributes after `|`. The default attribute is `id`.
    pub attributes: Option<Attributes<'a>>,
    pub blocks: Vec<Block<'a>>,
    /// Source range from `\periph` to the end of its last block.
    pub span: Span,
}
