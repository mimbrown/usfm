#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    MalformedVerseNumber,
    MalformedChapterNumber,
}
