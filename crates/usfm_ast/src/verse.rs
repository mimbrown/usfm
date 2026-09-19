use super::{NumberList, Span};
use std::borrow::Cow;

#[derive(Debug, PartialEq, Clone)]
pub struct VerseStart<'a> {
    pub number: NumberList,
    pub alt_number: Option<NumberList>,
    pub pub_number: Option<Cow<'a, str>>,
    /// Source range of the `\v` marker and its number.
    pub span: Span,
}

#[derive(Debug, PartialEq, Clone)]
pub struct VerseEnd {
    pub number: NumberList,
    /// Verse ends are synthesized, so this is [`SPAN`].
    pub span: Span,
}
