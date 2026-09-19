use super::{NumberList, Span};
use std::borrow::Cow;

#[derive(Debug, PartialEq, Clone)]
pub struct ChapterStart<'a> {
    pub number: usize,
    pub alt_number: Option<NumberList>,
    pub pub_number: Option<Cow<'a, str>>,
    /// Source range of the `\c` marker and its number.
    pub span: Span,
}

#[derive(Debug, PartialEq, Clone)]
pub struct ChapterEnd {
    pub number: usize,
    /// Chapter ends are synthesized, so this is [`SPAN`].
    pub span: Span,
}
