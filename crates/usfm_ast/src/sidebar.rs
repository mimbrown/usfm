use super::{Block, Span, StyleId, Text};

/// A study-Bible sidebar: `\esb` … `\esbe`.
///
/// The one block-level container. A sidebar holds paragraphs (and tables)
/// and is self-contained: it never spans a chapter or a verse, so nesting it
/// is faithful where nesting chapters or verses would not be (plan D4). The
/// scripture text flow stops before a sidebar and resumes after it, which is
/// why an open verse ends before it.
#[derive(Debug, PartialEq)]
pub struct Sidebar<'a> {
    /// The `\esb` style, resolved against the document's stylesheet.
    pub style: StyleId,
    /// The `\cat` category, when one directly follows `\esb`. Its span is the
    /// whole `\cat …\cat*`.
    pub category: Option<Text<'a>>,
    pub blocks: Vec<Block<'a>>,
    /// Source range from `\esb` to the end of `\esbe`, or to wherever the
    /// sidebar was implicitly closed.
    pub span: Span,
}
