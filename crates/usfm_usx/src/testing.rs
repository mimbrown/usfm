//! What USX cannot say, as one definition (ticket 45, shared since ticket 49).
//!
//! A tree read from USX and the parse of the USFM it was written from are
//! equal *modulo* this list: each rewrite puts a construct into the one form
//! USX has for it, which is the form the reader builds. Apply [`normalise`]
//! to both trees, then compare them with `usfm_ast::eq_ignoring_spans`.
//!
//! Measured over the conformance corpus by ticket 45, each entry naming the
//! case that showed it; entries 4 and 5 were found by `tasks/fuzz`'s
//! `usx_roundtrip` target on its seeds (ticket 49), in `fail` cases the
//! harness never compares. One the spec expected is not on it: whether a
//! `\+` was written is not in the tree to begin with.
//!
//! The module is shared: `tasks/conformance/tests/usx_reader.rs` compares
//! every reference with the parse of its USFM through it, and `tasks/fuzz`'s
//! `usx_roundtrip` target compares the parse of any input with the tree its
//! USX reads back to. It is behind the `testing` feature, so the default
//! build does not carry it — the same arrangement as `usfm_parser`'s
//! `span_check`.

use std::borrow::Cow;
use std::sync::Arc;

use usfm_ast::visit_mut::VisitMut;
use usfm_ast::{
    Attributes, Block, Book, Caller, ChapterStart, Char, Document, Inline, Milestone, Note, Para,
    Periph, SPAN, Sidebar, StyleId, Text, VerseStart,
    is_valid_attribute_name,
};
use usfm_style::StyleSheet;

use crate::DEFAULT_USX_VERSION;
use crate::xml_document::is_forbidden_in_xml;

/// Rewrite `document` into the form USX has for everything it cannot tell
/// apart: entries 1–6 below.
pub fn normalise(document: &mut Document<'_>) {
    let mut normalise = Normalise {
        style_sheet: Arc::clone(document.style_sheet()),
    };
    normalise.visit_document(document);
    declare_version(document);
}

struct Normalise {
    style_sheet: Arc<StyleSheet>,
}

impl Normalise {
    fn marker(&self, style: StyleId) -> &str {
        &self.style_sheet.get_rule(style.index()).marker
    }

    /// 1. **The default attribute has its name.** `\w a|b\w*` and
    ///    `\w a|lemma="b"\w*` are one `<char lemma="b">`; the reader cannot
    ///    tell them apart and names every attribute
    ///    (`specExamples/cross-ref`: `\ref 1|GEN 2:1\ref*` is `loc`;
    ///    `usfm-grammar/bugfixes/attrib-for-tl`: `\tl …|es\tl*` is `lang`).
    ///
    /// 4. **An attribute USX cannot carry is not there** (ticket 49, found by
    ///    `tasks/fuzz`'s `usx_roundtrip`; `paratextTests/EmptyFigure`,
    ///    `…/InvalidAttributeValuesReported` and
    ///    `special-cases/empty-attributes5` show it, none of them a case the
    ///    harness compares). The writer drops what it cannot write as XML —
    ///    a bare value where the marker has no default attribute
    ///    (`no-default-attribute`), a name that is not an XML name
    ///    (`malformed-attribute-name`), a repeat (`duplicate-attribute`), and
    ///    a name the element spells for itself (`style`, and `alt` on a
    ///    `<periph>`) — and the reader skips Paratext's bookkeeping
    ///    (`status`, `closed`). `\fig`'s `src` is USX's `file`, so the two
    ///    are one name.
    ///
    /// 5. **An empty attribute list is no list.** `\ts-s |\*` and `\ts-s\*`
    ///    are one `<ms>`, and so is a list 4 has emptied (the spec expected
    ///    this entry; `usfm-grammar/autofix/fr-textTranslation-FR_TLX`, a
    ///    fuzz seed, is the first input to show it).
    fn attributes(&self, attributes: &mut Option<Attributes<'_>>, marker: &str, reserved: &[&str]) {
        let Some(list) = attributes else {
            return;
        };
        let mut kept: Vec<String> = Vec::new();
        list.pairs.retain_mut(|pair| {
            if pair.name.is_empty() {
                match self.style_sheet.default_attribute(marker) {
                    Some(name) => pair.name = Cow::Owned(name.to_string()),
                    None => return false,
                }
            }
            if marker == "fig" && pair.name == "file" {
                pair.name = Cow::Borrowed("src");
            }
            xml_chars(&mut pair.value);
            let name = pair.name.as_ref();
            let writable = is_valid_attribute_name(name)
                && !reserved.contains(&name)
                && !matches!(name, "status" | "closed")
                && !kept.iter().any(|seen| seen == name);
            if writable {
                kept.push(name.to_string());
            }
            writable
        });
        if list.pairs.is_empty() {
            *attributes = None;
        }
    }
}

impl VisitMut for Normalise {
    /// 6. **A character XML cannot carry is U+FFFD** (ticket 49, found by
    ///    `usx_roundtrip`): the C0 controls but tab, line feed and carriage
    ///    return, and U+FFFE and U+FFFF, which the parser keeps and the
    ///    writer replaces rather than write a file no XML reader accepts
    ///    (ticket 06). Wherever the tree holds text: a `Text`, an attribute
    ///    value, a book's description, a published number, a caller, a
    ///    category, a title.
    fn visit_text(&mut self, text: &mut Text<'_>) {
        xml_chars(&mut text.content);
    }

    fn visit_book(&mut self, book: &mut Book<'_>) {
        xml_chars(&mut book.description);
    }

    fn visit_chapter_start(&mut self, chapter: &mut ChapterStart<'_>) {
        if let Some(number) = &mut chapter.pub_number {
            xml_chars(number);
        }
    }

    fn visit_verse_start(&mut self, verse: &mut VerseStart<'_>) {
        if let Some(number) = &mut verse.pub_number {
            xml_chars(number);
        }
    }

    fn visit_sidebar(&mut self, sidebar: &mut Sidebar<'_>) {
        if let Some(category) = &mut sidebar.category {
            self.visit_text(category);
        }
        for block in &mut sidebar.blocks {
            self.visit_block(block);
        }
    }

    /// 3. **A note's trailing whitespace is not compared**, for the reason
    ///    the conformance harness gives in `normalize_tree`: Paratext writes
    ///    `\ft text \f*` as `text </char></note>` in one reference and
    ///    `text </char> </note>` in another (`usfmjsTests/isa_inline_quotes`),
    ///    and the reader reads each as written. A cell's end is the same rule,
    ///    which the reader already applies (rule 5), so only a note needs it
    ///    here.
    fn visit_note(&mut self, note: &mut Note<'_>) {
        if let Caller::Custom(caller) = &mut note.caller {
            xml_chars(caller);
        }
        if let Some(category) = &mut note.category {
            self.visit_text(category);
        }
        for child in &mut note.children {
            self.visit_inline(child);
        }
        trim_trailing_text(&mut note.children);
    }

    fn visit_char(&mut self, char: &mut Char<'_>) {
        let marker = self.marker(char.style).to_string();
        self.attributes(&mut char.attributes, &marker, &["style"]);
        for child in &mut char.children {
            self.visit_inline(child);
        }
    }

    fn visit_milestone(&mut self, milestone: &mut Milestone<'_>) {
        let marker = self.marker(milestone.style).to_string();
        self.attributes(&mut milestone.attributes, &marker, &["style"]);
    }

    fn visit_periph(&mut self, periph: &mut Periph<'_>) {
        self.attributes(&mut periph.attributes, "periph", &["alt", "style"]);
        if let Some(title) = &mut periph.title {
            self.visit_text(title);
        }
        for block in &mut periph.blocks {
            self.visit_block(block);
        }
    }
}

/// Entry 6 over one string: every character XML cannot carry replaced by
/// U+FFFD, as the writer replaces it.
fn xml_chars(text: &mut Cow<'_, str>) {
    if text.contains(is_forbidden_in_xml) {
        *text = Cow::Owned(
            text.chars()
                .map(|c| {
                    if is_forbidden_in_xml(c) {
                        '\u{fffd}'
                    } else {
                        c
                    }
                })
                .collect(),
        );
    }
}

/// Trim ASCII whitespace from the end of the last text in `children`,
/// descending into a trailing character style and dropping what becomes
/// empty: the harness's `trim_trailing_text`, over the tree.
fn trim_trailing_text(children: &mut Vec<Inline<'_>>) {
    while let Some(last) = children.last_mut() {
        match last {
            Inline::Text(text) => {
                let trimmed = text.trim_end_matches(|c: char| c.is_ascii_whitespace());
                if trimmed.is_empty() {
                    children.pop();
                    continue;
                }
                text.content = Cow::Owned(trimmed.to_string());
            }
            Inline::Char(char) => trim_trailing_text(&mut char.children),
            _ => {}
        }
        return;
    }
}

/// 2. **One version is declared, right after `\id`.** `<usx version>` is
///    always there, and it is all USX has of `\usfm`: the reader reads it as
///    the `\usfm` paragraph after the book. A USFM file without one is
///    written with [`DEFAULT_USX_VERSION`], so it reads as if it had declared
///    that (`advanced/complex`, and most of tcdocs). Where a `\usfm` stood,
///    and every one after the first, USX cannot say either: the writer takes
///    the first one's version and writes no paragraph for any of them (ticket
///    49, found by `usx_roundtrip` in inputs with a `\usfm` before `\id` or
///    after a chapter). So every top-level `\usfm` paragraph is replaced by
///    one holding the document's version, after the book.
fn declare_version(document: &mut Document<'_>) {
    let Some(&usfm) = document.style_sheet().get_marker_index("usfm") else {
        return;
    };
    let version = document
        .usfm_version()
        .unwrap_or_else(|| DEFAULT_USX_VERSION.to_string());
    let style_sheet = Arc::clone(document.style_sheet());
    document
        .blocks
        .retain(|block| !matches!(block, Block::Para(para) if para.is_usfm_version(&style_sheet)));
    let para = Block::Para(Para {
        style: StyleId::new(usfm as u32),
        children: vec![Inline::Text(Text::synthesized(version))],
        span: SPAN,
    });
    let at = usize::from(matches!(document.blocks.first(), Some(Block::Book(_))));
    document.blocks.insert(at, para);
}
