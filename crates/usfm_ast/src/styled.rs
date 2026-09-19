use super::{Char, Note, Para, StyleId};

pub trait Styled {
    fn style(&self) -> StyleId;
}

impl<'a> Styled for Para<'a> {
    fn style(&self) -> StyleId {
        self.style
    }
}

impl<'a> Styled for Note<'a> {
    fn style(&self) -> StyleId {
        self.style
    }
}

impl<'a> Styled for Char<'a> {
    fn style(&self) -> StyleId {
        self.style
    }
}
