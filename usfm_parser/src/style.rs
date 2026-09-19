use std::ops::Deref;

pub struct Style<'a>(&'a str);

impl<'a> Deref for Style<'a> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<'a> Style<'a> {
    pub fn new(name: &'a str) -> Self {
        Self(name)
    }

    pub fn base(&self) -> &str {
        self.without_milestone()
            .trim_end_matches(|ch: char| ch.is_ascii_digit())
    }

    pub fn without_milestone(&self) -> &str {
        self.trim_end_matches("-s").trim_end_matches("-e")
    }

    pub fn is_start(&self) -> bool {
        self.ends_with("-s")
    }

    pub fn is_end(&self) -> bool {
        self.ends_with("-e")
    }

    /// Parses the level from the style name,
    /// e.g. "imt3" -> Some(3)
    pub fn level(&self) -> Option<usize> {
        let last_digits_rev = self
            .without_milestone()
            .chars()
            .rev()
            .take_while(|ch| ch.is_ascii_digit())
            .collect::<String>();
        if last_digits_rev.is_empty() {
            None
        } else {
            Some(
                last_digits_rev
                    .chars()
                    .rev()
                    .collect::<String>()
                    .parse()
                    .unwrap(),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_level() {
        assert_eq!(Style("imt3").level(), Some(3));
        assert_eq!(Style("qt2-s").level(), Some(2));
        assert_eq!(Style("qt1-e").level(), Some(1));
        assert_eq!(Style("qt").level(), None);
        assert_eq!(Style("tc12").level(), Some(12));
    }

    #[test]
    fn test_base() {
        assert_eq!(Style("imt3").base(), "imt");
        assert_eq!(Style("qt2-s").base(), "qt");
        assert_eq!(Style("qt1-e").base(), "qt");
        assert_eq!(Style("qt").base(), "qt");
        assert_eq!(Style("tc12").base(), "tc");
    }
}
