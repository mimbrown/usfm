//! Diglot output: two translations of the same book, side by side.
//!
//! Two shapes, both moved out of `usfm_parser/src/main.rs` by ticket 15:
//!
//! * [`serialize_html_diglot`] writes each document as a `<section>` of HTML
//!   whose paragraphs are broken at the section boundaries [`sections`] finds,
//!   so a stylesheet can line the two columns up;
//! * [`weave_prompt`] writes the two side by side as `<lang>` pairs, one pair
//!   per section — the "prompt" format, which is text for a language model
//!   rather than a document for a reader.
//!
//! **Escaping.** The HTML serializer writes text into an accumulator that also
//! holds markup from [`usfm_html`], so every run of document text goes through
//! [`usfm_html::write_escaped`] on the way in. Before ticket 15 it did not,
//! and a `<` in the text became a tag (ticket 14 found it).

use std::fmt::{Display, Formatter};

use usfm_ast::{Block, Document, Inline};
use usfm_html::{Context, HtmlElement, ToHtml, write_escaped};
use usfm_style::StyleSheet;

use crate::sections::{document_sections, sections};

/// Which of the two documents is being written, and so which class its
/// `<section>` carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

impl Side {
    fn class(self) -> &'static str {
        match self {
            Side::Left => "left",
            Side::Right => "right",
        }
    }
}

/// One side of a diglot as HTML: every vernacular paragraph is re-opened at
/// each section boundary, so the two sides can be aligned section by section.
pub struct DocumentSectionHtmlSerializer<'a> {
    document: &'a Document<'a>,
    style_sheet: &'a StyleSheet,
    side: Side,
}

impl<'a> DocumentSectionHtmlSerializer<'a> {
    pub fn new(document: &'a Document<'a>, style_sheet: &'a StyleSheet, side: Side) -> Self {
        Self {
            document,
            style_sheet,
            side,
        }
    }
}

/// Write what has accumulated since the last section boundary, wrapped in the
/// tags that are open around it, and clear it. Whitespace alone is dropped:
/// the gap between two sentences belongs to neither section.
fn drain(
    f: &mut Formatter<'_>,
    opening: &[String],
    closing: &[String],
    accumulator: &mut String,
) -> std::fmt::Result {
    if !accumulator.is_empty() && !accumulator.chars().all(char::is_whitespace) {
        for tag in opening.iter() {
            write!(f, "{}", tag)?;
        }
        write!(f, "{}", accumulator)?;
        for tag in closing.iter().rev() {
            write!(f, "{}", tag)?;
        }
    }
    accumulator.clear();
    Ok(())
}

impl Display for DocumentSectionHtmlSerializer<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut context = Context::new(self.style_sheet);

        write!(f, "<section class=\"{}\">", self.side.class())?;

        let mut opening = Vec::with_capacity(10);
        let mut closing = Vec::with_capacity(10);
        let mut accumulator = String::with_capacity(4096);

        for block in self.document.blocks.iter() {
            match block {
                Block::Para(para)
                    if self
                        .style_sheet
                        .get_rule(para.style.index())
                        .is_nonvernacular() =>
                {
                    block.to_html(f, &mut context)?;
                }
                Block::Para(para) => {
                    let tag = para.tag(&context);
                    write!(
                        f,
                        "<div class=\"para-start\">{}</div>",
                        self.style_sheet.get_rule(para.style.index()).marker
                    )?;
                    opening.push(format!(
                        "<{tag} class=\"{}\">",
                        self.style_sheet.get_rule(para.style.index()).marker
                    ));
                    closing.push(format!("</{tag}>"));

                    for inline in para.children.iter() {
                        match inline {
                            Inline::Text(text) => {
                                let mut s = sections(text);
                                // Text, not markup: it is escaped on the way
                                // into the accumulator, which the rest of this
                                // loop fills with HTML.
                                if let Some(first_section) = s.next() {
                                    write_escaped(&mut accumulator, first_section)?;
                                }
                                for section in s {
                                    drain(f, &opening, &closing, &mut accumulator)?;
                                    write_escaped(&mut accumulator, section)?;
                                }
                            }
                            _ => {
                                inline.to_html(&mut accumulator, &mut context)?;
                            }
                        }
                    }
                    drain(f, &opening, &closing, &mut accumulator)?;
                    opening.pop();
                    closing.pop();
                    write!(
                        f,
                        "<div class=\"para-end\">{}</div>",
                        self.style_sheet.get_rule(para.style.index()).marker
                    )?;
                }
                _ => {
                    block.to_html(f, &mut context)?;
                }
            }
        }

        write!(f, "</section>")?;

        Ok(())
    }
}

/// Both sides of a diglot as HTML, left section first.
///
/// Each stylesheet is its own document's (`document.style_sheet()`): the two
/// sides are parsed separately and may have had different styles derived
/// (hardening plan D3).
pub fn serialize_html_diglot(
    left: &Document,
    left_style_sheet: &StyleSheet,
    right: &Document,
    right_style_sheet: &StyleSheet,
) -> String {
    format!(
        "{}\n{}",
        DocumentSectionHtmlSerializer::new(left, left_style_sheet, Side::Left),
        DocumentSectionHtmlSerializer::new(right, right_style_sheet, Side::Right),
    )
}

/// The two sides of a diglot woven section by section, as `<lang>` pairs.
pub struct DocumentWeaver<'a> {
    pub left: &'a Document<'a>,
    pub left_style_sheet: &'a StyleSheet,
    pub right: &'a Document<'a>,
    pub right_style_sheet: &'a StyleSheet,
}

impl Display for DocumentWeaver<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let left_sections = document_sections(self.left, self.left_style_sheet);
        let right_sections = document_sections(self.right, self.right_style_sheet);
        // `zip` stops at the shorter side: a section with nothing to pair with
        // is not a translation pair, and the prompt format has no room for a
        // half of one.
        for (left, right) in left_sections.iter().zip(right_sections.iter()) {
            write!(
                f,
                "<lang dialect=\"a\">{}</lang>\n<lang dialect=\"b\">{}</lang>\n\n",
                left, right
            )?;
        }
        Ok(())
    }
}

/// The prompt format: [`DocumentWeaver`] as a string.
pub fn weave_prompt(
    left: &Document,
    left_style_sheet: &StyleSheet,
    right: &Document,
    right_style_sheet: &StyleSheet,
) -> String {
    format!(
        "{}",
        DocumentWeaver {
            left,
            left_style_sheet,
            right,
            right_style_sheet,
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::parse;

    /// Each vernacular paragraph is re-opened at every section boundary, and
    /// each side is wrapped in a `<section>` carrying its class.
    #[test]
    fn each_section_gets_its_own_paragraph() {
        let (left, left_sheet) = parse("\\id GEN\n\\c 1\n\\p \\v 1 One. Two.");
        let (right, right_sheet) = parse("\\id GEN\n\\c 1\n\\p \\v 1 Eins. Zwei.");
        let html = serialize_html_diglot(&left, &left_sheet, &right, &right_sheet);

        assert!(html.contains("<section class=\"left\">"), "{html}");
        assert!(html.contains("<section class=\"right\">"), "{html}");
        // Two sentences, so the `\p` is opened twice on each side.
        assert_eq!(html.matches("<p class=\"p\">").count(), 4, "{html}");
        assert!(html.contains("One."), "{html}");
        assert!(html.contains("Zwei."), "{html}");
    }

    /// Text goes into the same buffer as the markup around it, so `<` and `&`
    /// in the document have to be escaped there. Before ticket 15 they were
    /// written raw and `<Jerry>` became a tag.
    #[test]
    fn text_is_escaped_into_the_html() {
        let (left, left_sheet) = parse("\\id GEN\n\\c 1\n\\p \\v 1 Tom & <Jerry> won.");
        let (right, right_sheet) = parse("\\id GEN\n\\c 1\n\\p \\v 1 Tom & <Jerry> gewann.");
        let html = serialize_html_diglot(&left, &left_sheet, &right, &right_sheet);

        assert!(html.contains("Tom &amp; &lt;Jerry&gt; won."), "{html}");
        assert!(!html.contains("<Jerry>"), "{html}");
    }

    /// The prompt format pairs the sections of the two sides in order, and
    /// stops at the shorter side.
    #[test]
    fn the_prompt_format_pairs_sections() {
        let (left, left_sheet) = parse("\\id GEN\n\\c 1\n\\p \\v 1 One. Two.");
        let (right, right_sheet) = parse("\\id GEN\n\\c 1\n\\p \\v 1 Eins. Zwei. Drei.");
        let prompt = weave_prompt(&left, &left_sheet, &right, &right_sheet);

        assert_eq!(
            prompt,
            "<lang dialect=\"a\">One.</lang>\n<lang dialect=\"b\">Eins.</lang>\n\n\
             <lang dialect=\"a\">Two.</lang>\n<lang dialect=\"b\">Zwei.</lang>\n\n"
        );
    }
}
