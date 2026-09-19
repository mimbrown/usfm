#[derive(Debug, PartialEq)]
pub enum ParseError {
    MalformedVerseNumber,
    MalformedChapterNumber,
}
