use std::borrow::Cow;

use xml::namespace::Namespace;

use crate::{ast::*, context::Context, xml, xml_document::*};

/// Whether `name` can be added to `attrs`: a valid XML name that is not
/// already there. XML has no repeated attribute, so a name given twice
/// (`duplicate-attribute`) keeps its first value.
fn is_writable(attrs: &[xml::attribute::OwnedAttribute], name: &str) -> bool {
    is_valid_attribute_name(name) && !attrs.iter().any(|attr| attr.name.local_name == name)
}

pub trait ToUsx {
    fn to_usx(&self, context: &mut Context) -> XmlNode;
}

impl<'a> ToUsx for Text<'a> {
    fn to_usx(&self, context: &mut Context) -> XmlNode {
        self.content.to_usx(context)
    }
}

impl<'a> ToUsx for Cow<'a, str> {
    fn to_usx(&self, _context: &mut Context) -> XmlNode {
        XmlNode::Text(self.to_string())
    }
}

impl<'a> ToUsx for VerseStart<'a> {
    fn to_usx(&self, context: &mut Context) -> XmlNode {
        context.verse_number = Some(self.number.clone());
        let mut attrs = vec![
            attribute("number", self.number.to_string()),
            attribute("style", "v".to_string()),
        ];
        if let Some(alt) = &self.alt_number {
            attrs.push(attribute("altnumber", alt.to_string()));
        }
        if let Some(pub_number) = &self.pub_number {
            attrs.push(attribute("pubnumber", pub_number.to_string()));
        }
        attrs.push(attribute(
            "sid",
            format!(
                "{} {}:{}",
                context.book_code.unwrap_or(BookCode::Oth),
                context.chapter_number.unwrap_or(0),
                self.number
            ),
        ));
        XmlNode::Element(XmlElement {
            name: xml::name::OwnedName::local("verse"),
            attributes: attrs,
            namespace: xml::namespace::Namespace::empty(),
            children: vec![],
        })
    }
}

fn attribute(name: &str, value: String) -> xml::attribute::OwnedAttribute {
    xml::attribute::OwnedAttribute::new(xml::name::OwnedName::local(name), value)
}

impl ToUsx for VerseEnd {
    fn to_usx(&self, context: &mut Context) -> XmlNode {
        context.verse_number = None;
        XmlNode::Element(xml!("verse",
            "eid" => format!("{} {}:{}", context.book_code.unwrap_or(BookCode::Oth), context.chapter_number.unwrap_or(0), self.number)
        ))
    }
}

impl<'a> ToUsx for Char<'a> {
    fn to_usx(&self, context: &mut Context) -> XmlNode {
        let marker = &context.rule(self.style).marker;
        let style_name = marker.to_string();

        // Build attributes list, starting with style
        let (name, mut attrs) = if style_name == "ref" {
            ("ref", vec![])
        } else {
            (
                if style_name == "fig" {
                    "figure"
                } else {
                    "char"
                },
                vec![xml::attribute::OwnedAttribute::new(
                    xml::name::OwnedName::local("style"),
                    style_name.clone(),
                )],
            )
        };

        if let Some(attributes) = &self.attributes {
            for attr in &attributes.pairs {
                let name = if attr.name.is_empty() {
                    // The marker's default attribute, if it has one; `\fig`
                    // has none, so an unnamed value there is dropped.
                    default_attribute_name(&style_name)
                        .unwrap_or_default()
                        .to_string()
                } else if attr.name == "src" && style_name == "fig" {
                    "file".to_string()
                } else {
                    attr.name.to_string()
                };
                // Only add a name USX can carry: an empty one (a default
                // attribute the marker does not have), a malformed one
                // (`malformed-attribute-name`) and a repeat
                // (`duplicate-attribute`) are not writable as XML.
                if is_writable(&attrs, &name) {
                    attrs.push(xml::attribute::OwnedAttribute::new(
                        xml::name::OwnedName::local(name),
                        attr.value.to_string(),
                    ));
                }
            }
        }

        XmlNode::Element(XmlElement {
            name: xml::name::OwnedName::local(name),
            attributes: attrs,
            namespace: xml::namespace::Namespace::empty(),
            children: self
                .children
                .iter()
                .map(|child| child.to_usx(context))
                .collect(),
        })
    }
}

impl<'a> ToUsx for Note<'a> {
    fn to_usx(&self, context: &mut Context) -> XmlNode {
        let mut attrs = vec![
            xml::attribute::OwnedAttribute::new(xml::name::OwnedName::local("caller"), self.caller.to_string()),
            xml::attribute::OwnedAttribute::new(
                xml::name::OwnedName::local("style"),
                context.rule(self.style).marker.to_string(),
            ),
        ];
        if let Some(category) = &self.category {
            attrs.push(xml::attribute::OwnedAttribute::new(
                xml::name::OwnedName::local("category"),
                category.content.to_string(),
            ));
        }
        XmlNode::Element(XmlElement {
            name: xml::name::OwnedName::local("note"),
            attributes: attrs,
            namespace: xml::namespace::Namespace::empty(),
            children: self.children.iter().map(|child| child.to_usx(context)).collect(),
        })
    }
}

impl<'a> ToUsx for Periph<'a> {
    fn to_usx(&self, context: &mut Context) -> XmlNode {
        let mut attrs = vec![];
        if let Some(title) = &self.title {
            attrs.push(attribute("alt", title.content.to_string()));
        }
        if let Some(attributes) = &self.attributes {
            let marker = &context.rule(self.style).marker;
            for attr in &attributes.pairs {
                let name = if attr.name.is_empty() {
                    default_attribute_name(marker)
                } else {
                    Some(attr.name.as_ref())
                };
                if let Some(name) = name.filter(|name| is_writable(&attrs, name)) {
                    attrs.push(attribute(name, attr.value.to_string()));
                }
            }
        }
        XmlNode::Element(XmlElement {
            name: xml::name::OwnedName::local("periph"),
            attributes: attrs,
            namespace: xml::namespace::Namespace::empty(),
            children: self.blocks.iter().map(|block| block.to_usx(context)).collect(),
        })
    }
}

impl<'a> ToUsx for Sidebar<'a> {
    fn to_usx(&self, context: &mut Context) -> XmlNode {
        let mut attrs = vec![xml::attribute::OwnedAttribute::new(
            xml::name::OwnedName::local("style"),
            context.rule(self.style).marker.to_string(),
        )];
        if let Some(category) = &self.category {
            attrs.push(xml::attribute::OwnedAttribute::new(
                xml::name::OwnedName::local("category"),
                category.content.to_string(),
            ));
        }
        // A sidebar is outside the verse flow: its paragraphs carry no `vid`,
        // but the verse is still open for the paragraphs after `\esbe`.
        let verse_number = context.verse_number.take();
        let children = self.blocks.iter().map(|block| block.to_usx(context)).collect();
        context.verse_number = verse_number;
        XmlNode::Element(XmlElement {
            name: xml::name::OwnedName::local("sidebar"),
            attributes: attrs,
            namespace: xml::namespace::Namespace::empty(),
            children,
        })
    }
}

impl<'a> ToUsx for Milestone<'a> {
    fn to_usx(&self, context: &mut Context) -> XmlNode {
        let marker = &context.rule(self.style).marker;

        // Build attributes list, starting with style
        let mut attrs = vec![xml::attribute::OwnedAttribute::new(
            xml::name::OwnedName::local("style"),
            marker.to_string(),
        )];

        // Add milestone-specific attributes (sid, eid, who, etc.). An unnamed
        // value is the marker's default attribute: `\qt-s |Speaker\*` is `who`.
        for attr in &self.attributes.pairs {
            let name = if attr.name.is_empty() {
                default_attribute_name(marker)
            } else {
                Some(attr.name.as_ref())
            };
            if let Some(name) = name.filter(|name| is_writable(&attrs, name)) {
                attrs.push(xml::attribute::OwnedAttribute::new(
                    xml::name::OwnedName::local(name),
                    attr.value.to_string(),
                ));
            }
        }

        // Milestones are self-closing <ms> elements with no children
        XmlNode::Element(XmlElement {
            name: xml::name::OwnedName::local("ms"),
            attributes: attrs,
            namespace: xml::namespace::Namespace::empty(),
            children: vec![],
        })
    }
}

impl ToUsx for OptBreak {
    fn to_usx(&self, _context: &mut Context) -> XmlNode {
        XmlNode::Element(XmlElement {
            name: xml::name::OwnedName::local("optbreak"),
            attributes: vec![],
            namespace: xml::namespace::Namespace::empty(),
            children: vec![],
        })
    }
}

impl<'a> ToUsx for Inline<'a> {
    fn to_usx(&self, context: &mut Context) -> XmlNode {
        match self {
            Inline::Text(text) => text.to_usx(context),
            Inline::VerseStart(verse_start) => verse_start.to_usx(context),
            Inline::VerseEnd(verse_end) => verse_end.to_usx(context),
            Inline::Char(char) => char.to_usx(context),
            Inline::Note(note) => note.to_usx(context),
            Inline::Milestone(milestone) => milestone.to_usx(context),
            Inline::OptBreak(opt_break) => opt_break.to_usx(context),
            // Attributes are handled by the parent Char, not rendered directly
        }
    }
}

impl<'a> ToUsx for ChapterStart<'a> {
    fn to_usx(&self, context: &mut Context) -> XmlNode {
        context.chapter_number = Some(self.number);
        let mut attrs = vec![
            attribute("number", self.number.to_string()),
            attribute("style", "c".to_string()),
        ];
        if let Some(alt) = &self.alt_number {
            attrs.push(attribute("altnumber", alt.to_string()));
        }
        if let Some(pub_number) = &self.pub_number {
            attrs.push(attribute("pubnumber", pub_number.to_string()));
        }
        attrs.push(attribute(
            "sid",
            format!("{} {}", context.book_code.unwrap_or(BookCode::Oth), self.number),
        ));
        XmlNode::Element(XmlElement {
            name: xml::name::OwnedName::local("chapter"),
            attributes: attrs,
            namespace: xml::namespace::Namespace::empty(),
            children: vec![],
        })
    }
}

impl ToUsx for ChapterEnd {
    fn to_usx(&self, context: &mut Context) -> XmlNode {
        context.chapter_number = None;
        XmlNode::Element(xml!("chapter",
            "eid" => format!("{} {}", context.book_code.unwrap_or(BookCode::Oth), self.number)
        ))
    }
}

impl<'a> ToUsx for Book<'a> {
    fn to_usx(&self, context: &mut Context) -> XmlNode {
        context.book_code = Some(self.code);
        XmlNode::Element(
            xml!("book", "code" => self.code.to_string() "style" => "id",
                if self.description.is_empty() {
                    vec![]
                } else {
                    vec![self.description.to_usx(context)]
                }
            ),
        )
    }
}

impl<'a> ToUsx for Para<'a> {
    fn to_usx(&self, context: &mut Context) -> XmlNode {
        let style = context.rule(self.style).marker.to_string();

        // Build attributes, starting with style
        let mut attrs = vec![xml::attribute::OwnedAttribute::new(
            xml::name::OwnedName::local("style"),
            style,
        )];

        // Add vid attribute if we're continuing a verse from a previous paragraph
        if context.include_vid
            && let Some(verse_number) = &context.verse_number
        {
            let vid = format!(
                "{} {}:{}",
                context.book_code.unwrap_or(crate::ast::BookCode::Oth),
                context.chapter_number.unwrap_or(0),
                verse_number
            );
            attrs.push(xml::attribute::OwnedAttribute::new(
                xml::name::OwnedName::local("vid"),
                vid,
            ));
        }

        XmlNode::Element(XmlElement {
            name: xml::name::OwnedName::local("para"),
            attributes: attrs,
            namespace: xml::namespace::Namespace::empty(),
            children: self
                .children
                .iter()
                .map(|child| child.to_usx(context))
                .collect(),
        })
    }
}

impl<'a> ToUsx for Table<'a> {
    fn to_usx(&self, context: &mut Context) -> XmlNode {
        // Build attributes, starting with vid if we're in a verse
        let mut attrs = Vec::new();
        if context.include_vid
            && let Some(verse_number) = &context.verse_number
        {
            let vid = format!(
                "{} {}:{}",
                context.book_code.unwrap_or(crate::ast::BookCode::Oth),
                context.chapter_number.unwrap_or(0),
                verse_number
            );
            attrs.push(xml::attribute::OwnedAttribute::new(
                xml::name::OwnedName::local("vid"),
                vid,
            ));
        }

        XmlNode::Element(XmlElement {
            name: xml::name::OwnedName::local("table"),
            attributes: attrs,
            namespace: xml::namespace::Namespace::empty(),
            children: self.rows.iter().map(|row| row.to_usx(context)).collect(),
        })
    }
}

impl<'a> ToUsx for TableRow<'a> {
    fn to_usx(&self, context: &mut Context) -> XmlNode {
        let children: Vec<XmlNode> = self
            .cells
            .iter()
            .map(|cell| table_cell_to_usx(cell, context))
            .collect();

        XmlNode::Element(xml!("row", "style" => "tr", children))
    }
}

fn table_cell_to_usx(cell: &TableCell, context: &mut Context) -> XmlNode {
    let column = cell.column;
    // Build style: t + (h if header else c) + the alignment letter + column
    // number(s), which is the `t[hc][rc]?\d+(-\d+)?` pattern USX gives
    // `cell@style`. The letter is part of the style, not only of `align`:
    // tcdocs writes `<cell style="tcr3" align="end">`, so `\tcc3` is
    // `<cell style="tcc3" align="center">`.
    let header_char = if cell.header { 'h' } else { 'c' };
    let align_char = match cell.alignment {
        Alignment::Start => "",
        Alignment::Center => "c",
        Alignment::End => "r",
    };
    let column_str = if cell.colspan > 1 {
        format!("{}-{}", column, column + cell.colspan - 1)
    } else {
        column.to_string()
    };
    let style = format!("t{}{}{}", header_char, align_char, column_str);

    let attrs = vec![
        xml::attribute::OwnedAttribute::new(xml::name::OwnedName::local("style"), style),
        xml::attribute::OwnedAttribute::new(
            xml::name::OwnedName::local("align"),
            cell.alignment.to_string(),
        ),
    ];

    XmlNode::Element(XmlElement {
        name: xml::name::OwnedName::local("cell"),
        attributes: attrs,
        namespace: xml::namespace::Namespace::empty(),
        children: cell
            .children
            .iter()
            .map(|child| child.to_usx(context))
            .collect(),
    })
}

impl<'a> ToUsx for Block<'a> {
    fn to_usx(&self, context: &mut Context) -> XmlNode {
        match self {
            Block::ChapterStart(chapter_start) => chapter_start.to_usx(context),
            Block::ChapterEnd(chapter_end) => chapter_end.to_usx(context),
            Block::Para(para) => para.to_usx(context),
            Block::Book(book) => book.to_usx(context),
            Block::Table(table) => table.to_usx(context),
            Block::Milestone(milestone) => milestone.to_usx(context),
            Block::Sidebar(sidebar) => sidebar.to_usx(context),
            Block::Periph(periph) => periph.to_usx(context),
        }
    }
}

impl<'a> ToUsx for Document<'a> {
    fn to_usx(&self, context: &mut Context) -> XmlNode {
        // `\usfm 3.1` becomes the version attribute, not a paragraph.
        let version = self.usfm_version().unwrap_or_else(|| DEFAULT_USX_VERSION.to_string());
        let children = self
            .blocks
            .iter()
            .filter(|block| !matches!(block, Block::Para(para) if para.is_usfm_version(context.style_sheet)))
            .map(|child| child.to_usx(context))
            .collect::<Vec<_>>();
        XmlNode::Element(xml!("usx", "version" => version, children))
    }
}

/// The USX version emitted when the source has no `\usfm` marker.
pub const DEFAULT_USX_VERSION: &str = "3.0";

/// The document as a USX tree, resolved against the document's own stylesheet.
pub fn to_usx_node(document: &Document) -> XmlNode {
    document.to_usx(&mut Context::new(document.style_sheet()))
}

/// The document as USX text.
pub fn to_usx_string(document: &Document) -> String {
    format!("{}\n", to_usx_node(document))
}
