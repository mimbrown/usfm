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

    pub fn expect_str<M: Matcher>(&mut self, mut matcher: M) -> Result<&str, ()> {
        let mut i = 0;
        for c in self.source.chars() {
            if !matcher.matches(c) {
                break;
            }
            i += c.len_utf8();
        }
        if i == 0 {
            Err(())
        } else {
            let expected = unsafe { self.source.get_unchecked(0..i) };
            self.source = unsafe { self.source.get_unchecked(i..self.source.len()) };
            Ok(expected)
        }
    }

    pub fn expect_usize(&mut self) -> Result<usize, ()> {
        let number = self.expect_str(|c: char| c.is_ascii_digit())?;
        number.parse::<usize>().map_err(|_| ())
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

    fn consume_parser(parser: &mut StringParser) -> Result<Self, ()>;

    fn parse_str(string: &str) -> Result<Self, Self::Err> {
        let mut parser = StringParser::new(string);
        Self::consume_parser(&mut parser).map_err(|_| Self::create_err())
    }

    fn create_err() -> Self::Err;
}
