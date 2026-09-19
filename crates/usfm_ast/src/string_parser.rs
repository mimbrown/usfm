pub trait Matcher {
    fn matches(&mut self, c: char) -> bool;
}

impl Matcher for char {
    fn matches(&mut self, c: char) -> bool {
        *self == c
    }
}

impl<F> Matcher for F
where
    F: FnMut(char) -> bool,
{
    fn matches(&mut self, c: char) -> bool {
        self(c)
    }
}

/// What a [`StringParser`] returns when the input does not match. It carries
/// no position: the caller turns it into its own error with
/// [`ParseStr::create_err`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoMatch;

pub struct StringParser<'a> {
    source: &'a str,
}

impl<'a> StringParser<'a> {
    pub fn new(source: &'a str) -> Self {
        Self { source }
    }

    pub fn eat_char<M: Matcher>(&mut self, mut ch: M) -> bool {
        let Some(first) = self.source.chars().next() else {
            return false;
        };
        if ch.matches(first) {
            self.source = &self.source[first.len_utf8()..];
            true
        } else {
            false
        }
    }

    pub fn maybe_expect_char<M: Matcher>(&mut self, mut ch: M) -> Option<char> {
        let first = self.source.chars().next()?;
        if ch.matches(first) {
            self.source = &self.source[first.len_utf8()..];
            Some(first)
        } else {
            None
        }
    }

    pub fn expect_str<M: Matcher>(&mut self, mut matcher: M) -> Result<&str, NoMatch> {
        let mut i = 0;
        for c in self.source.chars() {
            if !matcher.matches(c) {
                break;
            }
            i += c.len_utf8();
        }
        if i == 0 {
            Err(NoMatch)
        } else {
            // `i` is a sum of `char::len_utf8`s taken from the front of
            // `self.source`, so it is a character boundary within it.
            let (expected, rest) = self.source.split_at(i);
            self.source = rest;
            Ok(expected)
        }
    }

    pub fn expect_usize(&mut self) -> Result<usize, NoMatch> {
        let number = self.expect_str(|c: char| c.is_ascii_digit())?;
        number.parse::<usize>().map_err(|_| NoMatch)
    }

    pub fn has_remaining(&self) -> bool {
        !self.source.is_empty()
    }
}

pub trait ParseStr
where
    Self: Sized,
{
    type Err;

    fn consume_parser(parser: &mut StringParser) -> Result<Self, NoMatch>;

    fn parse_str(string: &str) -> Result<Self, Self::Err> {
        let mut parser = StringParser::new(string);
        Self::consume_parser(&mut parser).map_err(|_| Self::create_err())
    }

    fn create_err() -> Self::Err;
}
