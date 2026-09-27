//! What every fuzz target checks, in one place.
//!
//! The parser never fails: it recovers and reports `Diagnostic`s. So the
//! properties a fuzz target can assert are the ones that hold for *any* input,
//! valid or not:
//!
//! 1. parsing does not panic — the targets go through `usfm::parse`, so
//!    that is the parser *and* the semantic checks over what it built
//!    (ticket 19), and every seed exercises both;
//! 2. every span points at the source it was read from
//!    (`usfm::parser::span_check`, the same invariants `crates/usfm_parser/tests/spans.rs`
//!    asserts over hand-written inputs);
//! 3. serializing the recovered tree to USX does not panic and produces
//!    well-formed XML;
//! 4. serializing it to HTML does not panic and produces well-formed markup
//!    (`check_html`, ticket 14);
//! 5. writing the recovered tree back as USFM and parsing *that* gives the
//!    same tree, gains no diagnostic, and writes out identically
//!    (`check_roundtrip`, ticket 27);
//! 6. the USX reader, which never fails either, does not panic on any input,
//!    keeps its spans in its source, and builds a tree the writer turns into
//!    well-formed USX (`check_read_usx`, ticket 49);
//! 7. the USX a parse writes reads back to the parse, up to what USX cannot
//!    say, and the tree it reads to round-trips through USFM as 5 asks
//!    (`check_usx_roundtrip`, ticket 49).
//!
//! Nothing here catches a panic: a panic *is* the finding, and libFuzzer wants
//! to see it.

use std::collections::BTreeMap;

use usfm::codegen::to_usfm_string;
use usfm::html::to_html_string;
use usfm::parser::span_check;
use usfm::usx::testing::normalise;
use usfm::usx::to_usx_string;

/// Run every check over one input.
pub fn check_source(source: &str) {
    let result = usfm::parse(source);
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
    let result = usfm::parse(source);
    let document = result.document;
    let html = to_html_string(&document, document.style_sheet());
    assert_html_well_formed(&html, source);
}

/// Parse, write USFM, parse again, and assert the round trip held.
///
/// The three assertions are `usfm_tests::roundtrip`'s, which is where the
/// property is defined and explained; they are restated here rather than
/// called because this crate builds under a sanitizer on nightly and depends
/// only on the facade — `usfm_tests` would drag in `xml-rs`, `regex`, `serde`
/// and `diffy` and the conformance corpus's paths for forty lines of code.
/// The two must say the same thing: change one, change the other.
///
/// 1. the two trees are equal ignoring spans;
/// 2. the second parse reports no diagnostic code the first did not. Gaining
///    one means the writer wrote something the parser likes *less* than the
///    input did. Losing one is the writer canonicalising a spelling the AST
///    does not record (`\v 01` is `\v 1` once the number is a `usize`), which
///    is its job. It is deliberately not "the second parse reports no error":
///    an error about the document rather than about its spelling — a `\v` with
///    no `\c`, a book with no `\id` — is written back faithfully and reported
///    again, and 21 of the 275 conformance cases do exactly that;
/// 3. writing the second tree gives the same bytes: the output is a fixed
///    point, so a writer that produced something the parser reads differently
///    cannot hide behind 1 and 2.
pub fn check_roundtrip(source: &str) {
    let first = usfm::parse(source);
    let written = to_usfm_string(&first.document);
    let second = usfm::parse(&written);

    assert!(
        usfm::ast::eq_ignoring_spans(&first.document, &second.document),
        "the tree changed.\n--- written ---\n{written}\n--- before ---\n{:#?}\n\
         --- after ---\n{:#?}\nsource: {source:?}",
        first.document,
        second.document,
    );

    let gained = gained_codes(&first.diagnostics, &second.diagnostics);
    assert!(
        gained.is_empty(),
        "the second parse gained diagnostics: {}\n--- written ---\n{written}\nsource: {source:?}",
        gained.join(", "),
    );

    let rewritten = to_usfm_string(&second.document);
    assert!(
        rewritten == written,
        "the output is not a fixed point.\n--- first ---\n{written}\n--- second ---\n\
         {rewritten}\nsource: {source:?}",
    );
}

/// Read `source` as USX, check the spans, and check the tree writes
/// well-formed USX.
///
/// Through `usfm::parse_usx`, the reader and the semantic checks over what it
/// built, as the USFM targets go through `usfm::parse`. The spans are held to
/// `span_check::read_violations`, the invariants as they read for XML, which
/// `tasks/conformance/tests/usx_reader.rs` asserts over every reference.
pub fn check_read_usx(source: &str) {
    let result = usfm::parse_usx(source);
    span_check::check_read(source, &result.document);
    let usx = to_usx_string(&result.document);
    assert_well_formed(&usx, source);
}

/// Parse `source` as USFM, write USX, read it back, and assert the tree is
/// the parse's up to what USX cannot say; then write the read tree as USFM
/// and assert what [`check_roundtrip`] asserts of it.
///
/// 1. `read(usx(parse(source)))` equals `parse(source)` ignoring spans, after
///    `usfm::usx::testing::normalise` on both — the list ticket 45 measured,
///    shared with `tasks/conformance/tests/usx_reader.rs` rather than copied;
/// 2. the read tree, written as USFM and parsed, is the same tree, the parse
///    reports no code `usfm::parse_usx` did not, and writing it again gives
///    the same bytes: [`check_roundtrip`]'s three, with the read as the first
///    parse. The codes are compared with the read's, not with the parse of
///    `source`, because the read tree is not the parse's but the parse's
///    after the normalisation — an empty source has no `\id` to miss, while
///    the `\usfm 3.0` every USX file declares makes a document that does.
pub fn check_usx_roundtrip(source: &str) {
    let first = usfm::parse(source);
    let usx = to_usx_string(&first.document);
    let read = usfm::parse_usx(&usx);

    // No `Clone` on a `Document`: parse and read again for the copies the
    // normalisation rewrites.
    let mut parsed = usfm::parse(source).document;
    let mut reread = usfm::parse_usx(&usx).document;
    normalise(&mut parsed);
    normalise(&mut reread);
    assert!(
        usfm::ast::eq_ignoring_spans(&parsed, &reread),
        "USX read back to a different tree.\n--- USX ---\n{usx}\n--- parsed ---\n{}\n\
         --- read ---\n{}\nsource: {source:?}",
        to_usfm_string(&parsed),
        to_usfm_string(&reread),
    );

    let written = to_usfm_string(&read.document);
    let second = usfm::parse(&written);
    assert!(
        usfm::ast::eq_ignoring_spans(&read.document, &second.document),
        "the read tree changed through USFM.\n--- USX ---\n{usx}\n--- written ---\n{written}\n\
         --- read ---\n{:#?}\n--- after ---\n{:#?}\nsource: {source:?}",
        read.document,
        second.document,
    );
    let gained = gained_codes(&read.diagnostics, &second.diagnostics);
    assert!(
        gained.is_empty(),
        "the parse of the read tree's USFM gained diagnostics: {}\n--- USX ---\n{usx}\n\
         --- written ---\n{written}\nsource: {source:?}",
        gained.join(", "),
    );
    let rewritten = to_usfm_string(&second.document);
    assert!(
        rewritten == written,
        "the output is not a fixed point.\n--- first ---\n{written}\n--- second ---\n\
         {rewritten}\nsource: {source:?}",
    );
}

/// The codes `after` reports more often than `before` does, each with both
/// counts.
fn gained_codes(before: &[usfm::Diagnostic], after: &[usfm::Diagnostic]) -> Vec<String> {
    let before = codes(before);
    codes(after)
        .iter()
        .filter(|(code, count)| before.get(*code).unwrap_or(&0) < count)
        .map(|(code, count)| format!("{code} ({} -> {count})", before.get(code).unwrap_or(&0)))
        .collect()
}

/// The diagnostic codes of a parse, counted. Keyed by the code's name, which
/// `Code` has and `Ord` has not, so a report lists them in a fixed order.
fn codes(diagnostics: &[usfm::Diagnostic]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for diagnostic in diagnostics {
        *counts.entry(diagnostic.code.to_string()).or_insert(0) += 1;
    }
    counts
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
