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

/// The replacement written in place of a character XML cannot carry.
const REPLACEMENT: &str = "\u{fffd}";

/// A character XML 1.0 does not allow anywhere, not even as a numeric
/// character reference (XML 1.0 §2.2: `Char` is everything but the C0
/// controls other than tab, newline and carriage return, the surrogates, and
/// U+FFFE/U+FFFF). The parser keeps whatever the input held, so such a
/// character can reach the writer, and writing it out would produce something
/// no XML reader accepts.
fn is_forbidden_in_xml(c: char) -> bool {
    matches!(
        c,
        '\u{0}'..='\u{8}' | '\u{b}' | '\u{c}' | '\u{e}'..='\u{1f}' | '\u{fffe}' | '\u{ffff}'
    )
}

/// The bytes `write_escaped` has to stop at: the three characters XML reserves
/// in character data, the C0 controls it forbids, and `0xEF`, which begins
/// U+FFFE and U+FFFF (and a great many characters it does allow, so a hit
/// there is checked). Everything else is copied through as bytes, which is
/// what keeps the writer off the UTF-8 decoding path for ordinary text.
const fn stop_bytes() -> [bool; 256] {
    let mut table = [false; 256];
    let mut byte = 0usize;
    while byte < 256 {
        table[byte] = matches!(
            byte as u8,
            b'&' | b'<' | b'>' | 0x00..=0x08 | 0x0b | 0x0c | 0x0e..=0x1f | 0xef
        );
        byte += 1;
    }
    table
}

static STOP: [bool; 256] = stop_bytes();

/// Writes `text` with the characters XML reserves in character data escaped,
/// and the characters XML forbids replaced by U+FFFD.
fn write_escaped(f: &mut std::fmt::Formatter<'_>, text: &str) -> std::fmt::Result {
    let bytes = text.as_bytes();
    // Everything from `written` to the character being replaced is copied in
    // one go, so unremarkable text costs one scan and one `write_str`.
    let mut written = 0;
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if !STOP[byte as usize] {
            index += 1;
            continue;
        }
        let (width, replacement) = match byte {
            b'&' => (1, "&amp;"),
            b'<' => (1, "&lt;"),
            b'>' => (1, "&gt;"),
            // U+FFFE and U+FFFF are `EF BF BE` and `EF BF BF`; every other
            // character that starts with `0xEF` is allowed.
            0xef => {
                let tail = &bytes[index + 1..];
                if tail.starts_with(b"\xbf\xbe") || tail.starts_with(b"\xbf\xbf") {
                    (3, REPLACEMENT)
                } else {
                    index += 1;
                    continue;
                }
            }
            _ => (1, REPLACEMENT),
        };
        f.write_str(&text[written..index])?;
        f.write_str(replacement)?;
        index += width;
        written = index;
    }
    f.write_str(&text[written..])
}

/// An attribute value with the characters XML forbids replaced, or the value
/// unchanged when it has none. Escaping the rest is `xml-rs`'s job, through
/// `Display for OwnedAttribute`.
fn without_forbidden(value: &str) -> std::borrow::Cow<'_, str> {
    if !value.contains(is_forbidden_in_xml) {
        return std::borrow::Cow::Borrowed(value);
    }
    let mut out = String::with_capacity(value.len());
    for character in value.chars() {
        if is_forbidden_in_xml(character) {
            out.push_str(REPLACEMENT);
        } else {
            out.push(character);
        }
    }
    std::borrow::Cow::Owned(out)
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
                    match without_forbidden(&attribute.value) {
                        std::borrow::Cow::Borrowed(_) => write!(f, " {}", attribute)?,
                        std::borrow::Cow::Owned(value) => write!(
                            f,
                            " {}",
                            OwnedAttribute {
                                name: attribute.name.clone(),
                                value,
                            }
                        )?,
                    }
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
