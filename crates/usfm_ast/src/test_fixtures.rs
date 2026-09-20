//! A hand-built document for the traversal tests, with a stylesheet just
//! big enough to resolve its markers. This crate has no parser, so trees
//! are built by hand.

use std::sync::Arc;

use usfm_style::{StyleRule, StyleSheet, StyleType, TextProperties, TextType};

use crate::*;

fn rule(marker: &str, style_type: StyleType, text_type: TextType) -> StyleRule {
    StyleRule {
        marker: marker.to_string(),
        name: None,
        description: None,
        style_type,
        text_type,
        text_properties: TextProperties::default(),
        nest: false,
        occurs_under: Vec::new(),
    }
}

/// A stylesheet with the markers [`sample_document`] uses.
pub fn sample_style_sheet() -> Arc<StyleSheet> {
    use StyleType::*;
    use TextType::*;
    Arc::new(StyleSheet::new(vec![
        rule("p", Paragraph, VerseText),
        rule("q1", Paragraph, VerseText),
        rule("s", Paragraph, Section),
        rule("nd", Character, VerseText),
        rule("w", Character, VerseText),
        rule("f", Note, NoteText),
        rule("ft", Character, NoteText),
        rule("qt-s", Milestone, Other),
        rule("qt-e", Milestone, Other),
        rule("esb", Paragraph, Other),
    ]))
}

fn style(sheet: &StyleSheet, marker: &str) -> StyleId {
    StyleId::new(*sheet.get_marker_index(marker).unwrap() as u32)
}

fn text(content: &'static str) -> Inline<'static> {
    Inline::Text(Text::synthesized(content))
}

fn verse_start(number: usize) -> Inline<'static> {
    Inline::VerseStart(VerseStart {
        number: NumberList::collapsed(number),
        alt_number: None,
        pub_number: None,
        span: SPAN,
    })
}

fn verse_end(number: usize) -> Inline<'static> {
    Inline::VerseEnd(VerseEnd {
        number: NumberList::collapsed(number),
        span: SPAN,
    })
}

fn attributes(pairs: &[(&'static str, &'static str)]) -> Attributes<'static> {
    Attributes {
        pairs: pairs
            .iter()
            .map(|(name, value)| Attribute {
                name: (*name).into(),
                value: (*value).into(),
                span: SPAN,
            })
            .collect(),
        pipe: SPAN,
    }
}

/// Genesis 1:1–2 as the parser would build it (ends included), with a
/// character style, a word with attributes, a footnote, quotation
/// milestones, a sidebar and a one-cell table:
///
/// ```text
/// \id GEN
/// \c 1
/// \p \v 1 In the beginning \nd God\nd* \w created|lemma="create" strong="H1254"\w* the heavens\f + \ft a note\f*
/// \q1 \qt-s |God\* and \v 2 the earth\qt-e\*
/// \esb \p Aside \esbe
/// \tr \tc1 Reuben
/// ```
pub fn sample_document() -> Document<'static> {
    let sheet = sample_style_sheet();
    let s = |marker: &str| style(&sheet, marker);
    let blocks = vec![
        Block::Book(Book {
            code: BookCode::Gen,
            description: "".into(),
            span: SPAN,
        }),
        Block::ChapterStart(ChapterStart {
            number: 1,
            alt_number: None,
            pub_number: None,
            span: SPAN,
        }),
        Block::Para(Para {
            style: s("p"),
            children: vec![
                verse_start(1),
                text("In the beginning "),
                Inline::Char(Char {
                    style: s("nd"),
                    children: vec![text("God")],
                    attributes: None,
                    span: SPAN,
                }),
                text(" "),
                Inline::Char(Char {
                    style: s("w"),
                    children: vec![text("created")],
                    attributes: Some(attributes(&[("lemma", "create"), ("strong", "H1254")])),
                    span: SPAN,
                }),
                text(" the heavens"),
                Inline::Note(Note {
                    style: s("f"),
                    caller: Caller::Plus,
                    category: None,
                    children: vec![Inline::Char(Char {
                        style: s("ft"),
                        children: vec![text("a note")],
                        attributes: None,
                        span: SPAN,
                    })],
                    span: SPAN,
                }),
                verse_end(1),
            ],
            span: SPAN,
        }),
        Block::Para(Para {
            style: s("q1"),
            children: vec![
                Inline::Milestone(Milestone {
                    style: s("qt-s"),
                    attributes: Some(attributes(&[("who", "God")])),
                    span: SPAN,
                }),
                text("and "),
                verse_start(2),
                text("the earth"),
                Inline::Milestone(Milestone {
                    style: s("qt-e"),
                    attributes: None,
                    span: SPAN,
                }),
                verse_end(2),
            ],
            span: SPAN,
        }),
        Block::Sidebar(Sidebar {
            style: s("esb"),
            category: None,
            blocks: vec![Block::Para(Para {
                style: s("p"),
                children: vec![text("Aside")],
                span: SPAN,
            })],
            span: SPAN,
        }),
        Block::Table(Table {
            rows: vec![TableRow {
                cells: vec![TableCell {
                    header: false,
                    alignment: Alignment::Start,
                    column: 1,
                    colspan: 1,
                    children: vec![text("Reuben")],
                    span: SPAN,
                }],
                span: SPAN,
            }],
            span: SPAN,
        }),
        Block::ChapterEnd(ChapterEnd {
            number: 1,
            span: SPAN,
        }),
    ];
    Document::new(blocks, sheet)
}
