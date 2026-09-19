//! Writing text and attribute values that an HTML parser reads back as what
//! the AST held.
//!
//! This is the twin of `usfm_usx::xml_document`'s `write_escaped` /
//! `without_forbidden`, deliberately copied rather than shared: the ADR
//! (`docs/adr/0001-oxc-style-crate-layout.md`) says no output crate depends on
//! another output crate. The two must agree on which characters are replaced,
//! so a change here is a change there. The differences are that this one
//! writes to any [`std::fmt::Write`] rather than to a `Formatter`, and that it
//! escapes the quote as well when it is writing an attribute value, because
//! nothing downstream does that for HTML (in USX `xml-rs` does it).

use std::fmt::Write;

/// The replacement written in place of a character the output cannot carry.
const REPLACEMENT: &str = "\u{fffd}";

/// A character that cannot be written to HTML at all, not even as a numeric
/// character reference: the C0 controls other than tab, newline and carriage
/// return, and U+FFFE/U+FFFF. This is XML 1.0 §2.2's `Char` set, which HTML5
/// also treats as a parse error in every case; keeping to it means the HTML
/// writer and the USX writer replace exactly the same characters. (HTML5 also
/// calls the C1 controls U+0080–U+009F a parse error, but they round-trip, so
/// they are left alone.)
const fn is_forbidden_in_html(c: char) -> bool {
    matches!(
        c,
        '\u{0}'..='\u{8}' | '\u{b}' | '\u{c}' | '\u{e}'..='\u{1f}' | '\u{fffe}' | '\u{ffff}'
    )
}

/// The bytes the scan has to stop at: the characters HTML reserves in markup,
/// the quote (only escaped in an attribute value), the C0 controls it cannot
/// carry, and `0xEF`, which begins U+FFFE and U+FFFF — and a great many
/// characters it does allow, so a hit there is checked. Everything else is
/// copied through as bytes, which is what keeps the writer off the UTF-8
/// decoding path for ordinary text.
const fn stop_bytes() -> [bool; 256] {
    let mut table = [false; 256];
    let mut byte = 0usize;
    while byte < 256 {
        // Below 0x80 a byte is its own character, so the table is built from
        // the predicate above rather than from a second copy of the list.
        table[byte] = matches!(byte as u8, b'&' | b'<' | b'>' | b'"' | 0xef)
            || (byte < 0x80 && is_forbidden_in_html(byte as u8 as char));
        byte += 1;
    }
    table
}

static STOP: [bool; 256] = stop_bytes();

/// Writes `text` with the characters HTML reserves in character data escaped
/// and the characters it cannot carry replaced by U+FFFD. `IN_ATTRIBUTE` adds
/// the quote to what is escaped.
fn escape<W: Write, const IN_ATTRIBUTE: bool>(f: &mut W, text: &str) -> std::fmt::Result {
    let bytes = text.as_bytes();
    // Everything from `written` to the character being replaced is copied in
    // one go, so unremarkable text costs one scan and one `write_str`.
    let mut written = 0;
    let mut index = 0;
    // The search for the next byte worth stopping at is a `position` over the
    // remaining slice rather than an indexed loop: it is the same scan with
    // the bounds check hoisted out, and text with nothing to escape — which
    // is nearly all of it — costs that scan and one `write_str`.
    while let Some(offset) = bytes[index..].iter().position(|&b| STOP[b as usize]) {
        index += offset;
        let byte = bytes[index];
        let (width, replacement) = match byte {
            b'&' => (1, "&amp;"),
            b'<' => (1, "&lt;"),
            b'>' => (1, "&gt;"),
            b'"' if IN_ATTRIBUTE => (1, "&quot;"),
            // A quote in character data is content, and needs no escape.
            b'"' => {
                index += 1;
                continue;
            }
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

/// Writes `text` as HTML character data: `&`, `<` and `>` escaped, the
/// characters HTML cannot carry replaced by U+FFFD, everything else verbatim.
pub fn write_escaped<W: Write>(f: &mut W, text: &str) -> std::fmt::Result {
    escape::<W, false>(f, text)
}

/// Writes `value` as the body of a double-quoted attribute value: as
/// [`write_escaped`], and the quote escaped as well.
pub fn write_escaped_attribute<W: Write>(f: &mut W, value: &str) -> std::fmt::Result {
    escape::<W, true>(f, value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn escaped(text: &str) -> String {
        let mut out = String::new();
        write_escaped(&mut out, text).unwrap();
        out
    }

    fn escaped_attribute(value: &str) -> String {
        let mut out = String::new();
        write_escaped_attribute(&mut out, value).unwrap();
        out
    }

    /// The three characters HTML reserves in character data, and nothing else:
    /// a quote or an apostrophe in text needs no escape.
    #[test]
    fn reserved_characters_are_escaped_in_text() {
        assert_eq!(
            escaped(r#"Tom & <Jerry> "said" it's"#),
            r#"Tom &amp; &lt;Jerry&gt; "said" it's"#
        );
    }

    /// An attribute value is written between double quotes, so the quote is
    /// escaped there as well.
    #[test]
    fn reserved_characters_are_escaped_in_attribute_values() {
        assert_eq!(escaped_attribute(r#"x&y<z">"#), r#"x&amp;y&lt;z&quot;&gt;"#);
    }

    /// HTML has no way to write a C0 control other than tab, newline and
    /// carriage return — not even as `&#0;` — nor U+FFFE/U+FFFF, so the writer
    /// replaces them, in text and in attribute values alike. Same set as
    /// `usfm_usx::xml_document`, checked by the same input.
    #[test]
    fn characters_html_forbids_are_replaced() {
        assert_eq!(
            escaped("a\u{0}b\u{1f}c\u{fffe}d\u{ffff}e"),
            "a\u{fffd}b\u{fffd}c\u{fffd}d\u{fffd}e"
        );
        assert_eq!(escaped_attribute("d\u{c}e"), "d\u{fffd}e");
    }

    /// The bytes either side of a replacement are copied through untouched,
    /// including the many characters that also begin with `0xEF`.
    #[test]
    fn allowed_characters_survive_the_scan() {
        let allowed = "tab\there\nline\rreturn \u{feff}\u{fffd}\u{ffef} é 漢";
        assert_eq!(escaped(allowed), allowed);
        assert_eq!(escaped_attribute(allowed), allowed);
    }

    /// Every character the table stops at is either escaped or replaced, and
    /// no other character is touched.
    #[test]
    fn the_stop_table_matches_the_rules() {
        for byte in 0u8..=0x7f {
            let character = byte as char;
            let expected =
                matches!(character, '&' | '<' | '>' | '"') || is_forbidden_in_html(character);
            assert_eq!(STOP[byte as usize], expected, "byte {byte:#04x}");
        }
    }
}
