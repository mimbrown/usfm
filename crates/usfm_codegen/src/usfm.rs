//! The AST as USFM text.
//!
//! The walk is a [`usfm_ast::visit::Visit`] implementation, the same shape as
//! the USX writer in `usfm_usx`, except that it appends to a
//! [`std::fmt::Write`] instead of building a tree. All the state it needs is
//! private to [`UsfmWriter`]: the document's stylesheet (a marker is written
//! from there, never from a hard-coded name) and whether the node being
//! written sits inside a character style, which is what decides `\+`.
//!
//! # What is written
//!
//! One line per block, and one canonical spelling per construct. The parser
//! accepts more spellings than it records, so writing the *one* that survives
//! a round trip is the whole job:
//!
//! | node | written as | note |
//! |---|---|---|
//! | `Book` | `\id GEN Description` | the description is a text run |
//! | `ChapterStart` | `\c 1`, then `\ca 4\ca*` and `\cp K` on lines of their own | `\cp` takes no closing marker: the parser reads one word |
//! | `ChapterEnd`, `VerseEnd` | *nothing* | synthesized; the parser re-emits them |
//! | `Para` | `\q2 content` | `\usfm 3.1` is one of these, so the version needs no line of its own |
//! | `VerseStart` | `\v 1 \va 2\va* \vp K\vp* ` | always a trailing space, which a paragraph-level trim drops when nothing follows |
//! | `Char` | `\nd Lord\nd*`, `\+nd Lord\+nd*` inside another `Char` | nesting is not recorded in the AST: a `Char` inside a `Char` is nested |
//! | `Note` | `\f + \cat People\cat*\fr 1.1 \ft text\f*` | the category is a leading `\cat`, which is how the parser reads one back; the content runs are not closed — see below |
//! | `Milestone` | `\qt-s \|who="God"\*`, `\ts\*` | no `\|` at all when `attributes` is `None` |
//! | `Attributes` | `\|lemma="grace" strong="H1234"`, `\|Speaker` | the default attribute is written bare |
//! | `Table` | `\tr \tc1 a \tcr2 b`, `\tc1-2` for a colspan | `\th`/`\thr` for a header cell |
//! | `Sidebar` | `\esb \cat People\cat*` … `\esbe` | |
//! | `Periph` | `\periph Title\|id="x"` | the attribute list ends with the line |
//! | `OptBreak` | `//` | |
//!
//! # Inside a note
//!
//! A note is the one place a closing marker is left out, because it is the one
//! place every USFM writer leaves it out. `\f + \fr 1.1 \ft the note\f*` is
//! what Paratext writes and what the corpora in this repo hold;
//! `\f + \fr 1.1 \ft the note\ft*\f*` is the same document and the shape
//! nobody spells (ticket 34 — until it, the writer reported 78 of the 86 books
//! of `tasks/benchmark/corpus/web/` under `usfm format --check`).
//!
//! Which styles those are is the stylesheet's answer, not a list here: a
//! `\StyleType Character` entry whose `\TextType` is `NoteText`
//! ([`UsfmWriter::is_note_text`]). The closer of such a child is left out when
//! what follows it is another one of them, or the note's own closing marker,
//! and written otherwise — the rule and its one sharp edge are on
//! [`UsfmWriter::omitted_closers`]. Two cases keep it for reasons of their
//! own: an attribute list runs to the closing marker, so a `\xt` that has one
//! is always closed; and `\xt` is the only note-internal marker that may nest,
//! so a sibling standing in front of a closed `\xt` keeps its closer too, or
//! the `\xt` would be read as its child.
//!
//! The omitted form reads back to the identical tree *and* to the identical
//! diagnostics: the parser says nothing about a character style implicitly
//! closed inside a note, neither by a sibling nor by the note's end, so the
//! round trip's "the second parse gains no code" holds with nothing to spare.
//! Outside a note the rule does not apply at all, `\xt` in a paragraph
//! included: there is nothing there to close it.
//!
//! # Escaping
//!
//! Text is written verbatim apart from three rewrites, which are exactly the
//! three the parser undoes (rules 3 and 4 on [`usfm_ast::Text`]):
//!
//! * `\` becomes `\\` — a bare `\` in the output would start a marker.
//! * `|` becomes `\|` — a bare `|` inside a character style would open an
//!   attribute list.
//! * U+00A0 becomes `~`, the no-break space USFM writes.
//!
//! A `"` is left alone: the lexer reads a bare `"` in running text as text, so
//! escaping it would be a second spelling of the same content. Inside a
//! *quoted attribute value* it is escaped as `\"`, because there it would end
//! the value; `\` is escaped there too, and nothing else is — a `|` inside
//! quotes is ordinary content to the parser.
//!
//! A `~` already in the content has no escape in USFM and is written as it is,
//! so it reads back as U+00A0. The parser cannot produce such a run — every
//! source `~` is U+00A0 by the time it reaches a `Text` — so this only
//! affects a tree built by hand.
//!
//! # Where the `|` goes
//!
//! A milestone's list is written the way USFM 3 spells it, with a space:
//! `\qt-s |who="God"\*`. The space is the marker's own whitespace, which the
//! parser eats before looking for the pipe — between blocks as well as inside
//! a paragraph, since ticket 29. (Until that fix the block path looked for the
//! pipe straight after the marker, so this writer had to put the `|` flush
//! against it: the spelling every real aligned text uses,
//! `\zaln-s |x-strong="H1"\*` on a line of its own, was the one the parser got
//! wrong. Both spellings read back as the same milestone now, and the writer
//! uses the one the spec shows.)
//!
//! A *character style's* list is a different thing and stays flush:
//! `\w grace|lemma="grace"\w*`. There the `|` ends the word, and a space
//! before it would be part of the word.

use std::fmt::{self, Write};

use usfm_ast::visit::{Visit, walk_block, walk_para, walk_table_cell, walk_table_row};
use usfm_ast::{
    Alignment, Attributes, Block, Book, ChapterEnd, ChapterStart, Char, Document, Inline,
    Milestone, Note, OptBreak, Para, Periph, Sidebar, StyleId, TableCell, TableRow, Text, VerseEnd,
    VerseStart,
};
use usfm_style::{StyleSheet, TextType};

/// The document as USFM text.
///
/// Every block that writes anything ends its line, so the output ends with a
/// newline unless the document is empty.
pub fn to_usfm_string(document: &Document<'_>) -> String {
    let mut out = String::new();
    // `String`'s `fmt::Write` is infallible, so this cannot fail.
    write_usfm(document, &mut out).expect("writing USFM to a String cannot fail");
    out
}

/// [`to_usfm_string`] into a caller's sink: a file, a socket, a buffer that is
/// already allocated.
///
/// The first write error stops the walk and is returned; nothing after it is
/// written.
pub fn write_usfm<W: Write>(document: &Document<'_>, out: &mut W) -> fmt::Result {
    let mut writer = UsfmWriter {
        out,
        style_sheet: document.style_sheet(),
        in_char: false,
        result: Ok(()),
    };
    writer.visit_document(document);
    writer.result
}

/// The walk, and the two things it has to remember.
struct UsfmWriter<'s, 'w, W: Write> {
    out: &'w mut W,
    /// The sheet the document owns, which is the base sheet plus every style
    /// the parser derived (hardening plan D3).
    style_sheet: &'s StyleSheet,
    /// Whether the node being written is directly inside a character style,
    /// which is the only place `\+` means anything: the parser reports
    /// `nested-marker-not-nested` for a `\+em` whose container is a note or a
    /// paragraph, so this is cleared when a note opens and restored after it.
    in_char: bool,
    /// The first write error, if there was one. Every push is a no-op after
    /// it, so the walk unwinds without a `?` on each of the several hundred
    /// writes a document costs.
    result: fmt::Result,
}

impl<'s, W: Write> UsfmWriter<'s, '_, W> {
    /// The marker name for a node's style, e.g. `"p"`.
    ///
    /// Borrowed from the stylesheet rather than from `self`, so the caller can
    /// go on writing while holding it: the sheet outlives the walk.
    fn marker(&self, style: StyleId) -> &'s str {
        let style_sheet: &'s StyleSheet = self.style_sheet;
        &style_sheet.get_rule(style.index()).marker
    }

    /// Whether a style is one of the note-internal character styles, which the
    /// stylesheet names rather than this crate: a `\StyleType Character` entry
    /// whose `\TextType` is `NoteText`. In Paratext's `usfm.sty` that is
    /// exactly the twenty-two markers a note's content is made of — `\fr`,
    /// `\ft`, `\fq`, `\fqa`, `\fk`, `\fl`, `\fp`, `\fv`, `\fw`, `\fdc`, `\fs`,
    /// `\xo`, `\xt`, `\xk`, `\xq`, `\xta`, `\xot`, `\xnt`, `\xdc`, `\xop`,
    /// `\xtSee` and `\cat` — and a derived style keeps its base's text type, so
    /// a `\ft2` the parser invented is one too.
    ///
    /// This is the only thing the writer asks about a style beyond its name.
    fn is_note_text(&self, style: StyleId) -> bool {
        let rule = self.style_sheet.get_rule(style.index());
        rule.is_character() && rule.text_type == TextType::NoteText
    }

    fn push(&mut self, text: &str) {
        if self.result.is_ok() {
            self.result = self.out.write_str(text);
        }
    }

    fn push_fmt(&mut self, args: fmt::Arguments<'_>) {
        if self.result.is_ok() {
            self.result = self.out.write_fmt(args);
        }
    }

    /// `\marker`, or `\+marker` where a `+` is what makes it nested.
    fn open_marker(&mut self, marker: &str, nested: bool) {
        self.push(if nested { "\\+" } else { "\\" });
        self.push(marker);
    }

    fn close_marker(&mut self, marker: &str, nested: bool) {
        self.open_marker(marker, nested);
        self.push("*");
    }

    fn newline(&mut self) {
        self.push("\n");
    }

    /// A text run, with the three rewrites of the module docs. One scan of the
    /// bytes, flushing the run between two rewrites in a single `write_str`:
    /// `\` and `|` are ASCII and U+00A0 is `C2 A0`, so every byte this stops
    /// on starts a character and every slice is a character-aligned range.
    fn write_text(&mut self, text: &str) {
        let bytes = text.as_bytes();
        let mut flushed = 0;
        let mut index = 0;
        while index < bytes.len() {
            let (width, replacement) = match bytes[index] {
                b'\\' => (1, "\\\\"),
                b'|' => (1, "\\|"),
                0xC2 if bytes.get(index + 1) == Some(&0xA0) => (2, "~"),
                _ => {
                    index += 1;
                    continue;
                }
            };
            self.push(&text[flushed..index]);
            self.push(replacement);
            index += width;
            flushed = index;
        }
        self.push(&text[flushed..]);
    }

    /// A quoted attribute value's text: `\` and `"` escaped, nothing else. The
    /// same byte scan as [`UsfmWriter::write_text`]; both characters are ASCII.
    fn write_attribute_value(&mut self, value: &str) {
        let bytes = value.as_bytes();
        let mut flushed = 0;
        for index in 0..bytes.len() {
            let replacement = match bytes[index] {
                b'\\' => "\\\\",
                b'"' => "\\\"",
                _ => continue,
            };
            self.push(&value[flushed..index]);
            self.push(replacement);
            flushed = index + 1;
        }
        self.push(&value[flushed..]);
    }

    /// An attribute list, flush against the marker or the text before it.
    ///
    /// A pair with no name is the marker's default attribute and is written
    /// bare, whether or not it is the only one: the name is not in the tree to
    /// write, and the parser reads a bare value as the default wherever it
    /// stands in the list. Its value goes out verbatim, because that is how
    /// the parser read it in — quotes, inner spaces and, when there was one,
    /// the space before the next pair.
    ///
    /// Which is why **nothing is written after a default pair**, not even when
    /// its value does not end in whitespace. The parser reads the default as
    /// the source from where it starts up to the `name=` that ends it, so any
    /// separator is already in the value; adding one puts it in the value a
    /// second time on the way back in. `\w x|"a=\w*` is the case that does not
    /// end in whitespace — the `"` and the `a` are separate tokens with
    /// nothing between them, so the default is `"` and the next pair is `a` —
    /// and writing `|" a=""` read back as the default `" `. Two runs of
    /// *words* cannot meet without a space (the lexer would have made them one
    /// word), so leaving the separator out can never merge a value into the
    /// name after it. The round-trip fuzz target found this.
    fn write_attributes(&mut self, attributes: &Attributes<'_>) {
        self.push("|");
        let mut separate = false;
        for pair in &attributes.pairs {
            if separate {
                self.push(" ");
            }
            if pair.name.is_empty() {
                self.push(&pair.value);
                separate = false;
            } else {
                self.push(&pair.name);
                self.push("=\"");
                self.write_attribute_value(&pair.value);
                self.push("\"");
                separate = true;
            }
        }
    }

    /// `\cat People\cat*`, the leading category of a note or a sidebar.
    fn write_category(&mut self, category: &Text<'_>) {
        self.push("\\cat ");
        self.write_text(&category.content);
        self.push("\\cat*");
    }

    /// The cell marker: `t`, `h` or `c`, the alignment letter, and the column
    /// the cell names — `\tc1`, `\tcr2`, `\th1-2`. The same string the USX
    /// writer puts in `cell@style`.
    fn write_cell_marker(&mut self, cell: &TableCell<'_>) {
        self.push("\\t");
        self.push(if cell.header { "h" } else { "c" });
        self.push(match cell.alignment {
            Alignment::Start => "",
            Alignment::Center => "c",
            Alignment::End => "r",
        });
        self.push_fmt(format_args!("{}", cell.column));
        if cell.colspan > 1 {
            self.push_fmt(format_args!("-{}", cell.column + cell.colspan - 1));
        }
    }

    /// Inline children with `in_char` set for the duration, restored
    /// afterwards. A note clears it, a character style sets it.
    fn write_children(&mut self, children: &[Inline<'_>], in_char: bool) {
        let outer = std::mem::replace(&mut self.in_char, in_char);
        for child in children {
            self.visit_inline(child);
        }
        self.in_char = outer;
    }

    /// A note's own children, which are the one place a closing marker is left
    /// out — see the module docs. `in_char` is cleared for them: a character
    /// style directly inside a note is not nested.
    fn write_note_children(&mut self, children: &[Inline<'_>]) {
        let omitted = self.omitted_closers(children);
        let outer = std::mem::replace(&mut self.in_char, false);
        for (index, child) in children.iter().enumerate() {
            match child {
                Inline::Char(char) if omitted[index] => self.write_char(char, false),
                child => self.visit_inline(child),
            }
        }
        self.in_char = outer;
    }

    /// Which of a note's children are written without their closing marker.
    ///
    /// Three conditions on a child, all of them about what the parser will
    /// read back:
    ///
    /// * the style is note-internal ([`UsfmWriter::is_note_text`]), so a
    ///   following marker of the same family closes it with no diagnostic —
    ///   the parser is silent about an implicit close inside a note;
    /// * it carries no attribute list, because `|` and its pairs run to the
    ///   closing marker and would otherwise swallow whatever comes next;
    /// * what comes next either is nothing at all, in which case the note's
    ///   own closing marker ends it, or another note-internal style *that is
    ///   itself written without a closer or may not nest*.
    ///
    /// Anything else — text, a milestone, an optional break, a character style
    /// from outside the note vocabulary — keeps the closer, because without it
    /// the text or the node would be read as part of this style's content.
    ///
    /// The last clause is the one that is not obvious, and `\xt` is the only
    /// note-internal marker it can bite: it is the only one with `NEST` in its
    /// `\OccursUnder`, and the parser reads an unmarked `\xt` as nested exactly
    /// when its own `\xt*` lies ahead. So `\fq a\fq*\xt b|link-href="x"\xt*`
    /// keeps *both* closers: drop the `\fq*` and the `\xt*` ahead turns the
    /// `\xt` into a child of the `\fq` rather than its sibling. Because a
    /// child's answer depends on the one after it, the flags are computed from
    /// the right.
    fn omitted_closers(&self, children: &[Inline<'_>]) -> Vec<bool> {
        let mut omitted = vec![false; children.len()];
        for index in (0..children.len()).rev() {
            let Inline::Char(char) = &children[index] else {
                continue;
            };
            if char.attributes.is_some() || !self.is_note_text(char.style) {
                continue;
            }
            omitted[index] = match children.get(index + 1) {
                None => true,
                Some(Inline::Char(next)) if self.is_note_text(next.style) => {
                    !self.style_sheet.get_rule(next.style.index()).nest || omitted[index + 1]
                }
                Some(_) => false,
            };
        }
        omitted
    }

    /// One character style. `close` is false only for the note-internal styles
    /// of [`UsfmWriter::write_note_children`].
    fn write_char(&mut self, char: &Char<'_>, close: bool) {
        let marker = self.marker(char.style);
        let nested = self.in_char;
        self.open_marker(marker, nested);
        self.push(" ");
        self.write_children(&char.children, true);
        if let Some(attributes) = &char.attributes {
            self.write_attributes(attributes);
        }
        if close {
            self.close_marker(marker, nested);
        }
    }
}

impl<W: Write> Visit for UsfmWriter<'_, '_, W> {
    /// A milestone between blocks is the one node whose `visit_*` cannot tell
    /// where it stands — `Block::Milestone` and `Inline::Milestone` share
    /// `visit_milestone` — so the line break it needs is added here.
    fn visit_block(&mut self, block: &Block<'_>) {
        match block {
            Block::Milestone(milestone) => {
                self.visit_milestone(milestone);
                self.newline();
            }
            block => walk_block(self, block),
        }
    }

    fn visit_book(&mut self, book: &Book<'_>) {
        self.push("\\id ");
        self.push(book.code.as_str());
        if !book.description.is_empty() {
            self.push(" ");
            self.write_text(&book.description);
        }
        self.newline();
    }

    fn visit_chapter_start(&mut self, chapter: &ChapterStart<'_>) {
        self.push_fmt(format_args!("\\c {}", chapter.number));
        self.newline();
        // Each on a line of its own, which is how the spec's own example and
        // every corpus in the repo spell them; `\c` looks past the line break
        // for both. `\ca` is a character style and is closed; `\cp` has no
        // closing marker at all — the parser reads the one word after it, and
        // a `\cp*` would be an unmatched closing marker.
        if let Some(alt) = &chapter.alt_number {
            self.push_fmt(format_args!("\\ca {alt}\\ca*"));
            self.newline();
        }
        // Verbatim, unlike `\vp`'s: `\cp` has no closing marker and the parser
        // reads a raw source word after it, so what the tree holds is what the
        // source spelled and writing it back unchanged is what reads back
        // unchanged. Escaping it would not — `\cp ~` would come back as the
        // one-character number `~` rather than as a no-break space. The
        // parser keeps its side of that: a `\cp` *paragraph* folded into the
        // chapter before it only gives up a first word the source spells
        // verbatim (ticket 35).
        if let Some(published) = &chapter.pub_number {
            self.push_fmt(format_args!("\\cp {published}"));
            self.newline();
        }
    }

    /// Nothing: a chapter end is synthesized, and the parser re-emits it.
    fn visit_chapter_end(&mut self, _chapter: &ChapterEnd) {}

    fn visit_para(&mut self, para: &Para<'_>) {
        let marker = self.marker(para.style);
        self.open_marker(marker, false);
        if !para.children.is_empty() {
            // The space belongs to the marker: the parser eats it before the
            // first child, so it is not content (rule 6 on `Text`).
            self.push(" ");
            walk_para(self, para);
        }
        self.newline();
    }

    fn visit_table_row(&mut self, row: &TableRow<'_>) {
        self.push("\\tr");
        walk_table_row(self, row);
        self.newline();
    }

    fn visit_table_cell(&mut self, cell: &TableCell<'_>) {
        self.push(" ");
        self.write_cell_marker(cell);
        if !cell.children.is_empty() {
            self.push(" ");
            walk_table_cell(self, cell);
        }
    }

    fn visit_sidebar(&mut self, sidebar: &Sidebar<'_>) {
        let marker = self.marker(sidebar.style);
        self.open_marker(marker, false);
        if let Some(category) = &sidebar.category {
            self.push(" ");
            self.write_category(category);
        }
        self.newline();
        for block in &sidebar.blocks {
            self.visit_block(block);
        }
        // `\esbe` is not a style any node carries: a sidebar holds only its
        // opening marker, and the closing one is fixed by the spec.
        self.push("\\esbe");
        self.newline();
    }

    fn visit_periph(&mut self, periph: &Periph<'_>) {
        let marker = self.marker(periph.style);
        self.open_marker(marker, false);
        if let Some(title) = &periph.title {
            self.push(" ");
            self.write_text(&title.content);
        }
        if let Some(attributes) = &periph.attributes {
            self.write_attributes(attributes);
        }
        self.newline();
        // The attribute list is written above, so the blocks are walked here
        // rather than through `walk_periph`, which would visit it as content.
        for block in &periph.blocks {
            self.visit_block(block);
        }
    }

    fn visit_text(&mut self, text: &Text<'_>) {
        self.write_text(&text.content);
    }

    fn visit_verse_start(&mut self, verse: &VerseStart<'_>) {
        self.push_fmt(format_args!("\\v {}", verse.number));
        // An alternate number is a `NumberList`, whose `Display` writes digits,
        // alphabetic modifiers and `,-`: nothing to escape. A *published*
        // number is text, and is whatever stood between `\vp` and its closing
        // marker, so a `\` or a `|` in it has to be escaped the way text is or
        // the next parse reads something else. Written raw, the `\` of
        // `\v 1\vp\` and the `\` of the `\vp*` after it made one `\\`, the
        // closing marker was gone and the number came back as `\vp*`. The
        // round-trip fuzz target found it (ticket 35).
        if let Some(alt) = &verse.alt_number {
            self.push_fmt(format_args!(" \\va {alt}\\va*"));
        }
        if let Some(published) = &verse.pub_number {
            // Unlike `\cp`, `\vp` is a character style and has to be closed.
            self.push(" \\vp ");
            self.write_text(published);
            self.push("\\vp*");
        }
        // The space after the number is the marker's, never content.
        self.push(" ");
    }

    /// Nothing: a verse end is synthesized, and the parser re-emits it.
    fn visit_verse_end(&mut self, _verse: &VerseEnd) {}

    fn visit_char(&mut self, char: &Char<'_>) {
        self.write_char(char, true);
    }

    fn visit_note(&mut self, note: &Note<'_>) {
        let marker = self.marker(note.style);
        self.open_marker(marker, false);
        self.push_fmt(format_args!(" {} ", note.caller));
        if let Some(category) = &note.category {
            self.write_category(category);
        }
        self.write_note_children(&note.children);
        self.close_marker(marker, false);
    }

    fn visit_milestone(&mut self, milestone: &Milestone<'_>) {
        let marker = self.marker(milestone.style);
        self.open_marker(marker, false);
        if let Some(attributes) = &milestone.attributes {
            // The space USFM 3 spells (`\qt-s |who="God"\*`). It is the
            // marker's own whitespace, so the parser eats it before looking
            // for the pipe, between blocks as well as inside a paragraph
            // (ticket 29).
            self.push(" ");
            self.write_attributes(attributes);
        }
        self.push("\\*");
    }

    fn visit_opt_break(&mut self, _opt_break: &OptBreak) {
        self.push("//");
    }
}

#[cfg(test)]
mod tests {
    use usfm_ast::eq_ignoring_spans;
    use usfm_parser::DEFAULT_STYLESHEET;
    use usfm_parser::parser::Parser;

    use super::*;

    /// Parse, write, and check that the text is what the construct's row in
    /// the module docs promises *and* that parsing it again gives the same
    /// tree. The second half is what the round-trip test does over the whole
    /// conformance corpus; here it comes free with every expectation.
    #[track_caller]
    fn writes(source: &str, expected: &str) {
        let first = Parser::new(source).parse(&DEFAULT_STYLESHEET).document;
        let written = to_usfm_string(&first);
        assert_eq!(written, expected, "writing {source:?}");
        let second = Parser::new(&written).parse(&DEFAULT_STYLESHEET).document;
        assert!(
            eq_ignoring_spans(&first, &second),
            "{source:?} does not round-trip\n{first:#?}\n{second:#?}"
        );
    }

    /// The same, for a source that is only a body: the `\id` and `\c` lines
    /// every document needs are added around it and taken off the output.
    #[track_caller]
    fn writes_body(body: &str, expected: &str) {
        let head = "\\id GEN\n\\c 1\n";
        // `\c 1` opens a chapter, whose end is synthesized and written as
        // nothing, so the output is the head plus the body.
        writes(&format!("{head}{body}"), &format!("{head}{expected}"));
    }

    #[test]
    fn a_book_line_keeps_its_description() {
        writes("\\id GEN Genesis, in English\n", "\\id GEN Genesis, in English\n");
        writes("\\id GEN\n", "\\id GEN\n");
    }

    /// `\usfm` stays in the tree as an ordinary paragraph, so writing the
    /// paragraphs writes the version: there is no second place for it.
    #[test]
    fn the_usfm_version_is_written_once() {
        let source = "\\id GEN\n\\usfm 3.1\n\\p text\n";
        let document = Parser::new(source).parse(&DEFAULT_STYLESHEET).document;
        assert_eq!(document.usfm_version().as_deref(), Some("3.1"));
        writes(source, "\\id GEN\n\\usfm 3.1\n\\p text\n");
    }

    /// Each on its own line, and `\c` finds them there — which is the spelling
    /// the spec's example and the corpora use, whichever line the source put
    /// them on. `\ca` is closed, `\cp` is not: the parser reads one word after
    /// it.
    #[test]
    fn a_chapter_keeps_its_alternate_and_published_numbers() {
        writes(
            "\\id GEN\n\\c 3\n\\ca 4\\ca*\n\\cp K\n\\p text\n",
            "\\id GEN\n\\c 3\n\\ca 4\\ca*\n\\cp K\n\\p text\n",
        );
        // The same chapter written on one line comes back on three.
        writes(
            "\\id GEN\n\\c 3 \\ca 4\\ca* \\cp K\n\\p text\n",
            "\\id GEN\n\\c 3\n\\ca 4\\ca*\n\\cp K\n\\p text\n",
        );
        // One without the other, and a chapter with neither, keep their lines.
        writes("\\id GEN\n\\c 3 \\cp K\n\\p t\n", "\\id GEN\n\\c 3\n\\cp K\n\\p t\n");
        writes("\\id GEN\n\\c 3\n\\p t\n", "\\id GEN\n\\c 3\n\\p t\n");
    }

    #[test]
    fn a_verse_keeps_its_alternate_and_published_numbers() {
        writes_body(
            "\\p \\v 1 \\va 2\\va*\\vp 1 (2)\\vp* text\n",
            "\\p \\v 1 \\va 2\\va* \\vp 1 (2)\\vp* text\n",
        );
    }

    /// A published number is text, not a number — and the two markers that
    /// carry one read it differently, so the writer spells it differently.
    ///
    /// `\vp` is a character style with a closing marker, so its number is a
    /// text run and is written as text: a `\` as `\\`, a `|` as `\|`.
    /// `\v 1\vp\` leaves a published number of one backslash, and writing it
    /// raw made `\` + `\vp*` into `\\` + `vp*` — the closing marker was
    /// swallowed by the escape it accidentally completed and the number came
    /// back as `\vp*`. The round-trip fuzz target found it (ticket 35).
    ///
    /// `\cp` has no closing marker and its reader takes one `Word` token, so
    /// its number is written verbatim; escaping it would be the mismatch
    /// instead. The parser holds up the other end: the `\cp` paragraph that
    /// only a dropped marker can produce (ticket 27) gives up a first word
    /// only when a `Word` token could be it.
    #[test]
    fn a_published_number_is_written_the_way_its_marker_reads_it() {
        writes_body("\\p \\v 1\\vp\\\n", "\\p \\v 1 \\vp \\\\\\vp* \n");
        // A `|` only reaches a published number escaped, and goes back the
        // same way: unescaped it would open an attribute list.
        writes_body("\\p \\v 1 \\vp \\|\\vp* a\n", "\\p \\v 1 \\vp \\|\\vp* a\n");
        // A chapter's published number is the other way round: `\cp` has no
        // closing marker and its reader takes a raw source word, so the
        // writer spells it verbatim — and the `\cp` paragraph of ticket 27
        // gives up only a first word the source spells verbatim. A stray `\`
        // and a no-break space are not, so the paragraph stays a paragraph.
        writes(
            "\\id GEN\n\\c 3\\c\n\\cp \\ x\n\\p t\n",
            "\\id GEN\n\\c 3\n\\p \\\\ x\n\\p t\n",
        );
        // A no-break space *is* a word character, so it folds — and is written
        // as itself, since `\cp ~` would read back as the one-character
        // number `~`.
        writes(
            "\\id GEN\n\\c 3\\c\n\\cp ~ x\n\\p t\n",
            "\\id GEN\n\\c 3\n\\cp \u{a0}\n\\p x\n\\p t\n",
        );
        writes("\\id GEN\n\\c 3 \\cp ~\n\\p t\n", "\\id GEN\n\\c 3\n\\cp ~\n\\p t\n");
    }

    /// The end milestones the parser synthesizes are not written: parsing the
    /// output puts them back, which is what `writes` checks.
    #[test]
    fn verse_and_chapter_ends_are_not_written() {
        writes_body("\\p \\v 1 one\n\\p \\v 2 two\n", "\\p \\v 1 one\n\\p \\v 2 two\n");
    }

    #[test]
    fn a_character_style_inside_another_is_nested_with_plus() {
        // `\bk` may nest and is closed ahead, so the parser reads the `\nd` as
        // nested; the writer says so outright.
        writes_body(
            "\\p \\bk a \\nd Lord\\nd* b\\bk*\n",
            "\\p \\bk a \\+nd Lord\\+nd* b\\bk*\n",
        );
    }

    /// A character style directly inside a *note* is not nested: `\+ft` there
    /// is `nested-marker-not-nested`, so only `\pn`, one level further in,
    /// gets a `+`. The `\ft` still ends with the note, so its own closer goes.
    #[test]
    fn a_character_style_inside_a_note_is_not_nested() {
        writes_body(
            "\\p \\f + \\ft see \\+pn Rome\\+pn*\\f*\n",
            "\\p \\f + \\ft see \\+pn Rome\\+pn*\\f*\n",
        );
    }

    #[test]
    fn a_note_keeps_its_caller_and_category() {
        writes_body(
            "\\p \\f + \\cat People\\cat* \\ft text\\f*\n",
            "\\p \\f + \\cat People\\cat*\\ft text\\f*\n",
        );
        writes_body("\\p \\f - \\ft text\\f*\n", "\\p \\f - \\ft text\\f*\n");
    }

    /// The idiom of ticket 34: a note's internal styles run into one another
    /// and into the note's own closer, with no closing marker between them.
    /// Every USFM writer spells a note this way, and the parser reports
    /// nothing for an implicit close inside a note, so the codes of the second
    /// parse are the codes of the first.
    #[test]
    fn note_internal_styles_run_into_one_another() {
        writes_body(
            "\\p \\f + \\fr 1.1 \\fq word \\ft the note\\f*\n",
            "\\p \\f + \\fr 1.1 \\fq word \\ft the note\\f*\n",
        );
        // The explicitly closed spelling is the same tree, and comes back in
        // the idiomatic one.
        writes_body(
            "\\p \\f + \\fr 1.1 \\fq word\\fq*\\ft the note\\ft*\\f*\n",
            "\\p \\f + \\fr 1.1 \\fq word\\ft the note\\f*\n",
        );
        // A cross reference is the same rule with the `\x` vocabulary.
        writes_body(
            "\\p \\x - \\xo 1.1 \\xt Gen 1.1\\x*\n",
            "\\p \\x - \\xo 1.1 \\xt Gen 1.1\\x*\n",
        );
    }

    /// The closer stays when the next sibling is not another note-internal
    /// style: without it the text, the milestone or the outside style would be
    /// read as this style's content.
    #[test]
    fn a_note_internal_style_followed_by_anything_else_keeps_its_closer() {
        // Text after it: `\fq b\fq* trailing`.
        writes_body(
            "\\p \\f + \\fq b\\fq* trailing\\f*\n",
            "\\p \\f + \\fq b\\fq* trailing\\f*\n",
        );
        // A character style from outside the note vocabulary.
        writes_body(
            "\\p \\f + \\ft see\\ft*\\nd Lord\\nd*\\f*\n",
            "\\p \\f + \\ft see\\ft*\\nd Lord\\nd*\\f*\n",
        );
        // A milestone.
        writes_body(
            "\\p \\f + \\ft see\\ft*\\qt-s |who=\"God\"\\*\\f*\n",
            "\\p \\f + \\ft see\\ft*\\qt-s |who=\"God\"\\*\\f*\n",
        );
    }

    /// An attribute list runs to the closing marker, so a note-internal style
    /// that has one is closed whatever follows: `\xt ...|link-href="..."\xt*`.
    #[test]
    fn a_note_internal_style_with_attributes_keeps_its_closer() {
        writes_body(
            "\\p \\x - \\xo 1.1\\xo* \\xt Gen 1.1|link-href=\"GEN 1:1\"\\xt*\\x*\n",
            "\\p \\x - \\xo 1.1\\xo* \\xt Gen 1.1|link-href=\"GEN 1:1\"\\xt*\\x*\n",
        );
        // Followed by another note-internal style rather than the note's end:
        // still closed.
        writes_body(
            "\\p \\x - \\xt Gen 1.1|link-href=\"GEN 1:1\"\\xt*\\xq q\\x*\n",
            "\\p \\x - \\xt Gen 1.1|link-href=\"GEN 1:1\"\\xt*\\xq q\\x*\n",
        );
    }

    /// `\xt` is the one note-internal marker that may nest, and the parser
    /// nests an unmarked one exactly when its own `\xt*` lies ahead. So a
    /// sibling before an `\xt` that keeps its closer has to keep its own:
    /// without it the `\xt` would be read as a child. When the `\xt` loses its
    /// closer too, both go.
    #[test]
    fn a_style_before_a_closed_xt_keeps_its_closer() {
        writes_body(
            "\\p \\x - \\xq a\\xq*\\xt b|link-href=\"x\"\\xt*\\x*\n",
            "\\p \\x - \\xq a\\xq*\\xt b|link-href=\"x\"\\xt*\\x*\n",
        );
        writes_body(
            "\\p \\x - \\xq a\\xq*\\xt b\\xt*\\x*\n",
            "\\p \\x - \\xq a\\xt b\\x*\n",
        );
    }

    /// A style nested inside a note-internal one keeps its `\+` closer — it is
    /// a child, not a sibling — while the style around it follows the rule.
    #[test]
    fn a_nested_style_inside_a_note_keeps_its_closer() {
        writes_body(
            "\\p \\f + \\ft text \\+nd Lord\\+nd* more\\f*\n",
            "\\p \\f + \\ft text \\+nd Lord\\+nd* more\\f*\n",
        );
        writes_body(
            "\\p \\f + \\ft text \\+nd Lord\\+nd*\\fq q\\f*\n",
            "\\p \\f + \\ft text \\+nd Lord\\+nd*\\fq q\\f*\n",
        );
    }

    /// The rule is a note's, not a paragraph's: `\xt` is note-internal by text
    /// type and may also stand in a paragraph, where nothing would close it.
    #[test]
    fn a_note_internal_style_outside_a_note_keeps_its_closer() {
        writes_body("\\p see \\xt Gen 1.1\\xt*\n", "\\p see \\xt Gen 1.1\\xt*\n");
    }

    #[test]
    fn word_attributes_are_written_as_pairs() {
        writes_body(
            "\\p \\w grace|lemma=\"grace\" strong=\"G5485\"\\w*\n",
            "\\p \\w grace|lemma=\"grace\" strong=\"G5485\"\\w*\n",
        );
    }

    /// A `"` inside a quoted value would end it, so it goes out as `\"` —
    /// which is exactly the escape the parser resolves on the way in.
    #[test]
    fn a_quote_in_an_attribute_value_is_escaped() {
        writes_body(
            "\\p \\w x|lemma=\"a\\\"b\" strong=\"c\\\\d\"\\w*\n",
            "\\p \\w x|lemma=\"a\\\"b\" strong=\"c\\\\d\"\\w*\n",
        );
    }

    /// The default attribute has no name in the tree to write, so it is
    /// written bare — value verbatim, quotes and all, because that is how the
    /// parser read it.
    #[test]
    fn the_default_attribute_is_written_bare() {
        writes_body(
            "\\p \\w grace|a special concept\\w*\n",
            "\\p \\w grace|a special concept\\w*\n",
        );
        // A bare `|` with no pairs is not the same document as no `|` at all.
        writes_body("\\p \\w grace|\\w*\n", "\\p \\w grace|\\w*\n");
    }

    /// Nothing separates a default attribute from the pair after it: the space
    /// that did is already inside the verbatim value, and where there was none
    /// adding one changes the value. `|"a=` is the second case — the `"` and
    /// the `a` are separate tokens — and it goes back out as it came in. See
    /// `write_attributes`.
    #[test]
    fn a_default_attribute_gets_no_separator_after_it() {
        writes_body(
            "\\p \\w x|a special concept strong=\"G1\"\\w*\n",
            "\\p \\w x|a special concept strong=\"G1\"\\w*\n",
        );
        writes_body("\\p \\w x|\"a=\"\"\\w*\n", "\\p \\w x|\"a=\"\"\\w*\n");
    }

    #[test]
    fn a_milestone_with_no_pipe_keeps_none() {
        writes_body("\\p \\ts-s\\*text\\ts-e\\*\n", "\\p \\ts-s\\*text\\ts-e\\*\n");
        // The default attribute of `\qt-s` is `who`, and the AST keeps the
        // pair unnamed, so it goes back out bare.
        writes_body(
            "\\p \\qt-s |Pilate\\*text\\qt-e\\*\n",
            "\\p \\qt-s |Pilate\\*text\\qt-e\\*\n",
        );
        writes_body(
            "\\p \\qt-s |who=\"Pilate\"\\*text\\qt-e\\*\n",
            "\\p \\qt-s |who=\"Pilate\"\\*text\\qt-e\\*\n",
        );
    }

    /// A milestone between blocks, written the way USFM 3 spells it and the
    /// way unfoldingWord's aligned texts do: a space before the `|`. Before
    /// ticket 29 the parser read that as an unknown marker followed by text,
    /// so the writer had to leave the space out; now the two spellings give
    /// the same milestone and this one goes out unchanged.
    #[test]
    fn a_milestone_between_blocks_is_on_its_own_line() {
        writes(
            "\\id GEN\n\\c 1\n\\zaln-s |x-strong=\"H1\"\\*\n\\p text\n",
            "\\id GEN\n\\c 1\n\\zaln-s |x-strong=\"H1\"\\*\n\\p text\n",
        );
        // The spelling with no space is the same milestone and is written
        // back with one.
        writes(
            "\\id GEN\n\\c 1\n\\zaln-s|x-strong=\"H1\"\\*\n\\p text\n",
            "\\id GEN\n\\c 1\n\\zaln-s |x-strong=\"H1\"\\*\n\\p text\n",
        );
    }

    #[test]
    fn a_table_keeps_its_columns_spans_and_alignment() {
        writes_body(
            "\\tr \\th1 Tribe \\thr2 Number\n\\tr \\tc1-2 Reuben \\tcr3 46,500\n\\p after\n",
            "\\tr \\th1 Tribe \\thr2 Number\n\\tr \\tc1-2 Reuben \\tcr3 46,500\n\\p after\n",
        );
    }

    #[test]
    fn a_sidebar_keeps_its_category_and_is_closed() {
        writes_body(
            "\\esb \\cat History\\cat*\n\\p aside\n\\esbe\n\\p after\n",
            "\\esb \\cat History\\cat*\n\\p aside\n\\esbe\n\\p after\n",
        );
    }

    #[test]
    fn a_periph_keeps_its_title_and_attributes() {
        writes(
            "\\id FRT\n\\periph Title Page|id=\"title\"\n\\p text\n",
            "\\id FRT\n\\periph Title Page|id=\"title\"\n\\p text\n",
        );
        writes("\\id FRT\n\\periph Preface\n\\p text\n", "\\id FRT\n\\periph Preface\n\\p text\n");
    }

    /// The three text rewrites, in one line each.
    #[test]
    fn text_is_escaped_the_way_the_parser_reads_it_back() {
        // `\\` and `\|` resolve to the literal; `~` is a no-break space.
        writes_body("\\p a\\\\b \\|c \\\"d\n", "\\p a\\\\b \\|c \"d\n");
        writes_body("\\p verse~text\n", "\\p verse~text\n");
        writes_body("\\p Man // Who\n", "\\p Man // Who\n");
        writes_body("\\p Man//Who\n", "\\p Man//Who\n");
    }

    /// A stray `\` is kept as text by the parser, and the writer has one
    /// spelling for a literal backslash: `\\`.
    #[test]
    fn a_stray_backslash_comes_back_escaped() {
        writes_body("\\p a \\ b\n", "\\p a \\\\ b\n");
    }

    /// The error path: the walk stops at the first failure and returns it,
    /// rather than swallowing it or panicking.
    #[test]
    fn a_write_error_is_returned() {
        struct Full;
        impl Write for Full {
            fn write_str(&mut self, _text: &str) -> fmt::Result {
                Err(fmt::Error)
            }
        }
        let document = Parser::new("\\id GEN\n\\p text\n")
            .parse(&DEFAULT_STYLESHEET)
            .document;
        assert_eq!(write_usfm(&document, &mut Full), Err(fmt::Error));
    }
}
