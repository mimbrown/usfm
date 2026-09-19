//! What every fuzz target checks, in one place.
//!
//! The parser never fails: it recovers and reports `Diagnostic`s. So the
//! properties a fuzz target can assert are the ones that hold for *any* input,
//! valid or not:
//!
//! 1. parsing does not panic;
//! 2. every span points at the source it was read from
//!    (`usfm::parser::span_check`, the same invariants `crates/usfm_parser/tests/spans.rs`
//!    asserts over hand-written inputs);
//! 3. serializing the recovered tree to USX does not panic and produces
//!    well-formed XML;
//! 4. serializing it to HTML does not panic and produces well-formed markup
//!    (`check_html`, ticket 14).
//!
//! Nothing here catches a panic: a panic *is* the finding, and libFuzzer wants
//! to see it.

use usfm::html::to_html_string;
use usfm::parser::parser::Parser;
use usfm::parser::{DEFAULT_STYLESHEET, span_check};
use usfm::usx::to_usx_string;

/// Run every check over one input.
pub fn check_source(source: &str) {
    let result = Parser::new(source).parse(&DEFAULT_STYLESHEET);
    span_check::check_parse(source, &result);
    let usx = to_usx_string(&result.document);
    assert_well_formed(&usx, source);
}

/// Parse, write HTML, and check the HTML is well formed.
///
/// The styles are resolved against the document's own stylesheet, which is
/// the parser's extended with anything it had to derive (hardening plan D3);
/// resolving against `DEFAULT_STYLESHEET` instead would panic on an input
/// holding an unknown marker.
pub fn check_html(source: &str) {
    let result = Parser::new(source).parse(&DEFAULT_STYLESHEET);
    let document = result.document;
    let html = to_html_string(&document, document.style_sheet());
    assert_html_well_formed(&html, source);
}

/// Parse `usx` back with `xml-rs` and drain it to the end of the document.
///
/// A serializer that forgets to escape something, or writes a character XML
/// does not allow, shows up here as a reader error rather than as invalid
/// output nobody looked at.
/// The elements that close themselves: the HTML writer emits `<wbr>` for
/// `//`, and the rest are here so the check does not have to be revisited when
/// one of them turns up.
const VOID_ELEMENTS: [&str; 7] = ["br", "wbr", "hr", "img", "meta", "link", "input"];

/// Check `html` the way `assert_well_formed` checks USX, but without an HTML
/// parser: nothing in the fuzz crate should pull in a dependency big enough to
/// have bugs of its own, and the properties worth asserting are small.
///
/// 1. every start tag is closed, by the right name, in the right order, with
///    a self-closing tag or a void element closing itself;
/// 2. every attribute value is quoted with `"` and holds no raw `<`;
/// 3. outside a tag, `<` only ever starts one, and `&` only ever starts a
///    character reference;
/// 4. no character HTML cannot carry reaches the output.
fn assert_html_well_formed(html: &str, source: &str) {
    if let Err(problem) = check_markup(html) {
        panic!("HTML is not well formed: {problem}\nHTML:\n{html}\nsource: {source:?}");
    }
}

/// A character no HTML document can carry, not even as a numeric character
/// reference. The same set `usfm::html::write_escaped` replaces, so an escape
/// this misses is one the writer let through.
fn is_forbidden_in_html(c: char) -> bool {
    matches!(
        c,
        '\u{0}'..='\u{8}' | '\u{b}' | '\u{c}' | '\u{e}'..='\u{1f}' | '\u{fffe}' | '\u{ffff}'
    )
}

/// The first 20 characters from `offset`, for an error message.
fn snippet(html: &str, offset: usize) -> String {
    html[offset..].chars().take(20).collect()
}

fn check_markup(html: &str) -> Result<(), String> {
    if let Some((offset, character)) = html.char_indices().find(|(_, c)| is_forbidden_in_html(*c)) {
        return Err(format!(
            "U+{:04X} at byte {offset} is not writable in HTML",
            character as u32
        ));
    }

    let bytes = html.as_bytes();
    let mut open: Vec<&str> = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        index = match bytes[index] {
            b'<' if html[index..].starts_with("<!--") => {
                match html[index..].find("-->") {
                    Some(end) => index + end + 3,
                    None => return Err(format!("unterminated comment at byte {index}")),
                }
            }
            b'<' => scan_tag(html, index, &mut open)?,
            b'&' => scan_entity(html, index)?,
            _ => index + 1,
        };
    }
    match open.last() {
        Some(name) => Err(format!("<{name}> is never closed")),
        None => Ok(()),
    }
}

/// Scan one tag starting at `start`, updating the stack of open elements.
/// Returns the offset just past the tag.
fn scan_tag<'a>(html: &'a str, start: usize, open: &mut Vec<&'a str>) -> Result<usize, String> {
    let bytes = html.as_bytes();
    let mut index = start + 1;
    let closing = bytes.get(index) == Some(&b'/');
    if closing {
        index += 1;
    }
    let name_start = index;
    while index < bytes.len() && bytes[index].is_ascii_alphanumeric() {
        index += 1;
    }
    let name = &html[name_start..index];
    if name.is_empty() {
        return Err(format!(
            "a raw '<' at byte {start}: {:?}",
            snippet(html, start)
        ));
    }

    if closing {
        if bytes.get(index) != Some(&b'>') {
            return Err(format!("</{name} at byte {start} does not end in '>'"));
        }
        return match open.pop() {
            None => Err(format!("</{name}> at byte {start} closes nothing")),
            Some(expected) if expected != name => Err(format!(
                "</{name}> at byte {start} closes <{expected}>",
            )),
            Some(_) => Ok(index + 1),
        };
    }

    let mut self_closing = false;
    loop {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        match bytes.get(index) {
            None => return Err(format!("<{name} at byte {start} is never closed")),
            Some(b'>') => {
                index += 1;
                break;
            }
            Some(b'/') => {
                if bytes.get(index + 1) != Some(&b'>') {
                    return Err(format!("<{name} at byte {start} has a stray '/'"));
                }
                self_closing = true;
                index += 2;
                break;
            }
            Some(_) => index = scan_attribute(html, index, name)?,
        }
    }

    if !self_closing && !VOID_ELEMENTS.contains(&name) {
        open.push(name);
    }
    Ok(index)
}

/// Scan one attribute — `name` or `name="value"` — and return the offset just
/// past it.
fn scan_attribute(html: &str, start: usize, tag: &str) -> Result<usize, String> {
    let bytes = html.as_bytes();
    let mut index = start;
    while index < bytes.len()
        && (bytes[index].is_ascii_alphanumeric() || matches!(bytes[index], b'-' | b'_' | b'.'))
    {
        index += 1;
    }
    if index == start {
        return Err(format!(
            "<{tag}> has no attribute name at byte {start}: {:?}",
            snippet(html, start)
        ));
    }
    if bytes.get(index) != Some(&b'=') {
        // A bare attribute, such as `popover`.
        return Ok(index);
    }
    index += 1;
    if bytes.get(index) != Some(&b'"') {
        return Err(format!(
            "<{tag}>: the value at byte {index} is not quoted with '\"': {:?}",
            snippet(html, index)
        ));
    }
    index += 1;
    while index < bytes.len() && bytes[index] != b'"' {
        index = match bytes[index] {
            b'<' => {
                return Err(format!(
                    "<{tag}>: a raw '<' in an attribute value at byte {index}"
                ));
            }
            b'&' => scan_entity(html, index)?,
            _ => index + 1,
        };
    }
    if index >= bytes.len() {
        return Err(format!("<{tag}>: unterminated attribute value"));
    }
    Ok(index + 1)
}

/// Scan one character reference — `&name;`, `&#123;` or `&#x7b;` — and return
/// the offset just past it.
fn scan_entity(html: &str, start: usize) -> Result<usize, String> {
    let bytes = html.as_bytes();
    let mut index = start + 1;
    if bytes.get(index) == Some(&b'#') {
        index += 1;
        let hexadecimal = matches!(bytes.get(index), Some(b'x' | b'X'));
        if hexadecimal {
            index += 1;
        }
        let digits = index;
        while index < bytes.len()
            && (if hexadecimal {
                bytes[index].is_ascii_hexdigit()
            } else {
                bytes[index].is_ascii_digit()
            })
        {
            index += 1;
        }
        if index == digits {
            return Err(format!(
                "'&#' at byte {start} has no digits: {:?}",
                snippet(html, start)
            ));
        }
    } else {
        let name = index;
        while index < bytes.len() && bytes[index].is_ascii_alphanumeric() {
            index += 1;
        }
        if index == name {
            return Err(format!(
                "a raw '&' at byte {start}: {:?}",
                snippet(html, start)
            ));
        }
    }
    if bytes.get(index) != Some(&b';') {
        return Err(format!(
            "the '&' at byte {start} does not end a character reference: {:?}",
            snippet(html, start)
        ));
    }
    Ok(index + 1)
}

fn assert_well_formed(usx: &str, source: &str) {
    let parser = xml::EventReader::from_str(usx);
    for event in parser {
        match event {
            Ok(xml::reader::XmlEvent::EndDocument) => return,
            Ok(_) => {}
            Err(error) => {
                panic!("USX is not well-formed XML: {error}\nUSX:\n{usx}\nsource: {source:?}")
            }
        }
    }
}
