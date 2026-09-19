use std::{fmt::Display, io::Read};

use xml::{
    EventReader,
    attribute::OwnedAttribute,
    name::OwnedName,
    namespace::Namespace,
    reader::{Error, XmlEvent},
};

#[derive(Debug, PartialEq, Clone)]
pub enum XmlNode {
    Element(XmlElement),
    Text(String),
}

#[derive(Debug, PartialEq, Clone)]
pub struct XmlElement {
    pub name: OwnedName,
    pub attributes: Vec<OwnedAttribute>,
    pub namespace: Namespace,
    pub children: Vec<XmlNode>,
}

impl XmlElement {
    pub fn append_text(&mut self, text: String) {
        if let Some(XmlNode::Text(last_text)) = self.children.last_mut() {
            last_text.push_str(&text);
        } else {
            self.children.push(XmlNode::Text(text.to_string()));
        }
    }
}

enum WriteContext {
    Indent(usize),
    Inline,
}

/// Writes `text` with the characters XML reserves in character data escaped.
fn write_escaped(f: &mut std::fmt::Formatter<'_>, text: &str) -> std::fmt::Result {
    let mut rest = text;
    while let Some(index) = rest.find(['&', '<', '>']) {
        f.write_str(&rest[..index])?;
        f.write_str(match rest.as_bytes()[index] {
            b'&' => "&amp;",
            b'<' => "&lt;",
            _ => "&gt;",
        })?;
        rest = &rest[index + 1..];
    }
    f.write_str(rest)
}

impl XmlNode {
    /// An element whose children are all elements is indented, one child per
    /// line. An element with text among its children is written on one line,
    /// because whitespace there is content.
    fn write(&self, f: &mut std::fmt::Formatter<'_>, context: &WriteContext) -> std::fmt::Result {
        match self {
            XmlNode::Element(element) => {
                if let WriteContext::Indent(indent) = context {
                    write!(f, "{}", " ".repeat(*indent))?;
                }
                write!(f, "<{}", element.name)?;
                for attribute in element.attributes.iter() {
                    write!(f, " {}", attribute)?;
                }
                if element.children.is_empty() {
                    return write!(f, " />");
                }
                write!(f, ">")?;
                let child_context = match context {
                    WriteContext::Indent(indent)
                        if !element
                            .children
                            .iter()
                            .any(|child| matches!(child, XmlNode::Text(_))) =>
                    {
                        WriteContext::Indent(indent + 2)
                    }
                    _ => WriteContext::Inline,
                };
                for child in element.children.iter() {
                    if matches!(child_context, WriteContext::Indent(_)) {
                        writeln!(f)?;
                    }
                    child.write(f, &child_context)?;
                }
                if let (WriteContext::Indent(indent), WriteContext::Indent(_)) =
                    (context, &child_context)
                {
                    writeln!(f)?;
                    write!(f, "{}", " ".repeat(*indent))?;
                }
                write!(f, "</{}>", element.name)?;
            }
            XmlNode::Text(text) => write_escaped(f, text)?,
        }
        Ok(())
    }
}

impl Display for XmlNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write(f, &WriteContext::Indent(0))
    }
}

impl XmlElement {
    pub fn read<R: Read>(
        mut event_reader: EventReader<R>,
        name: OwnedName,
        attributes: Vec<OwnedAttribute>,
        namespace: Namespace,
    ) -> Result<(EventReader<R>, XmlElement), Error> {
        let mut element = XmlElement {
            name,
            attributes,
            namespace,
            children: Vec::new(),
        };
        loop {
            match event_reader.next()? {
                XmlEvent::StartElement {
                    name,
                    attributes,
                    namespace,
                } => {
                    let (moved_event_reader, child_element) =
                        XmlElement::read(event_reader, name, attributes, namespace)?;
                    element.children.push(XmlNode::Element(child_element));
                    event_reader = moved_event_reader;
                }
                XmlEvent::Characters(s) => {
                    element.append_text(s);
                }
                XmlEvent::EndElement { name: _end_name } => {
                    return Ok((event_reader, element));
                }
                XmlEvent::Whitespace(s) if s == " " => {
                    element.append_text(s);
                }
                _ => {}
            }
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct XmlDocument {
    pub root: XmlElement,
}

impl XmlDocument {
    pub fn from<R: Read>(s: R) -> Result<Self, Error> {
        let mut parser = EventReader::new(s);
        loop {
            if let XmlEvent::StartElement {
                name,
                attributes,
                namespace,
            } = parser.next()?
            {
                let (_, root) = XmlElement::read(parser, name, attributes, namespace)?;
                return Ok(XmlDocument { root });
            }
        }
    }
}

#[macro_export]
macro_rules! xml {
    ($tag:expr) => {
        XmlElement {
            name: xml::name::OwnedName::local($tag),
            attributes: vec![],
            namespace: Namespace::empty(),
            children: vec![],
        }
    };

    ($tag:expr, $($key:expr => $value:expr)*) => {
        XmlElement {
            name: xml::name::OwnedName::local($tag),
            attributes: vec![ $(xml::attribute::OwnedAttribute::new(xml::name::OwnedName::local($key), $value)),* ],
            namespace: Namespace::empty(),
            children: vec![],
        }
    };

    ($tag:expr, $children:expr) => {
        XmlElement {
            name: xml::name::OwnedName::local($tag),
            attributes: vec![],
            namespace: Namespace::empty(),
            children: $children,
        }
    };

    ($tag:expr, $($key:expr => $value:expr)*, $children:expr) => {
        XmlElement {
            name: xml::name::OwnedName::local($tag),
            attributes: vec![ $(xml::attribute::OwnedAttribute::new(xml::name::OwnedName::local($key), $value)),* ],
            namespace: Namespace::empty(),
            children: $children,
        }
    };
}
