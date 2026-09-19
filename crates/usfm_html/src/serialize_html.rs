//! HTML output, node by node.
//!
//! **Escaping.** Everything that comes from the document is written through
//! [`write_escaped`] (character data) or [`write_escaped_attribute`] (the body
//! of a double-quoted attribute), so text holding `&`, `<`, `>` or `"` reads
//! back as itself and a character HTML cannot carry becomes U+FFFD. What is
//! *not* escaped is what cannot need it: a marker name, which the lexer builds
//! from `[A-Za-z0-9_-]` only; a book code, three ASCII letters or digits; a
//! chapter or verse number, digits with alphabetic modifiers; and the ids and
//! tag names this module makes up itself.

use crate::{
    context::Context,
    escape::{write_escaped, write_escaped_attribute},
};
use std::fmt::{Display, Formatter, Result, Write};
use usfm_ast::*;

/// Trait for converting AST nodes to HTML
///
/// This trait provides a more extensible approach to HTML serialization
/// by allowing each AST node type to define its own HTML representation.
/// It can be used alongside or as an alternative to the existing Serialize trait.
pub trait ToHtml {
    /// Convert this AST node to HTML, writing to the provided formatter
    ///
    /// # Arguments
    /// * `f` - The formatter to write HTML output to
    /// * `context` - The serialization context containing style information and state
    ///
    /// # Returns
    /// A Result indicating success or failure of the formatting operation
    fn to_html<W: Write>(&self, f: &mut W, context: &mut Context) -> Result;
}

pub trait HtmlElement {
    fn tag(&self, context: &Context<'_>) -> &'static str;
}

/// Trait for customizable HTML serialization
///
/// This trait provides default implementations for all HTML serialization methods,
/// allowing implementors to override only the specific methods they want to customize.
/// Any type implementing this trait automatically gets a `Serialize` implementation.
pub trait SerializeHtml {
    /// Serialize a chapter start marker
    fn serialize_chapter_start<W: Write>(
        &self,
        f: &mut W,
        context: &mut Context,
        chapter: &ChapterStart,
    ) -> Result {
        chapter.to_html(f, context)
    }

    /// Serialize a chapter end marker
    fn serialize_chapter_end<W: Write>(
        &self,
        f: &mut W,
        context: &mut Context,
        chapter: &ChapterEnd,
    ) -> Result {
        chapter.to_html(f, context)
    }

    /// Serialize a verse start marker
    fn serialize_verse_start<W: Write>(
        &self,
        f: &mut W,
        context: &mut Context,
        verse: &VerseStart,
    ) -> Result {
        verse.to_html(f, context)
    }

    /// Serialize a verse end marker
    fn serialize_verse_end<W: Write>(
        &self,
        f: &mut W,
        context: &mut Context,
        verse: &VerseEnd,
    ) -> Result {
        verse.to_html(f, context)
    }

    /// Serialize plain text, escaped as HTML character data (see
    /// [`write_escaped`]). An override that writes `text` straight out is
    /// writing markup, not text, and owns the escaping itself.
    fn serialize_text<W: Write>(&self, f: &mut W, _context: &mut Context, text: &str) -> Result {
        write_escaped(f, text)
    }

    /// Serialize a book marker
    fn serialize_book<W: Write>(&self, f: &mut W, context: &mut Context, book: &Book) -> Result {
        book.to_html(f, context)
    }

    /// Serialize a character style marker
    fn serialize_char<W: Write>(&self, f: &mut W, context: &mut Context, char: &Char) -> Result {
        char.to_html(f, context)
    }

    /// Serialize a note
    fn serialize_note<W: Write>(&self, f: &mut W, context: &mut Context, note: &Note) -> Result {
        note.to_html(f, context)
    }

    /// Serialize a table
    fn serialize_table<W: Write>(&self, f: &mut W, context: &mut Context, table: &Table) -> Result {
        table.to_html(f, context)
    }

    /// Serialize a paragraph
    fn serialize_para<W: Write>(&self, f: &mut W, context: &mut Context, para: &Para) -> Result {
        para.to_html(f, context)
    }

    /// Serialize an inline element
    fn serialize_inline<W: Write>(
        &self,
        f: &mut W,
        context: &mut Context,
        inline: &Inline,
    ) -> Result {
        inline.to_html(f, context)
    }

    /// Serialize a block element
    fn serialize_block<W: Write>(&self, f: &mut W, context: &mut Context, block: &Block) -> Result {
        block.to_html(f, context)
    }

    /// Serialize a document
    fn serialize_document<W: Write>(
        &self,
        f: &mut W,
        context: &mut Context,
        document: &Document,
    ) -> Result {
        document.to_html(f, context)
    }
}

pub struct XmlAttributes<'a>(Vec<(&'a str, &'a str)>);

impl<'a> Default for XmlAttributes<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> XmlAttributes<'a> {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn from(attributes: Vec<(&'a str, &'a str)>) -> Self {
        Self(attributes)
    }

    pub fn append(&mut self, name: &'a str, value: &'a str) {
        self.0.push((name, value));
    }
}

impl<'a> Display for XmlAttributes<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        for (name, value) in self.0.iter() {
            write!(f, " {name}=\"")?;
            write_escaped_attribute(f, value)?;
            f.write_str("\"")?;
        }
        Ok(())
    }
}

impl<'a> Context<'a> {
    fn next_id(&mut self) -> String {
        format!("id-{}", self.next_counter("id"))
    }
}

// Implementation of ToHtml trait for all AST node types

impl<'a> ToHtml for Document<'a> {
    fn to_html<W: Write>(&self, f: &mut W, context: &mut Context) -> Result {
        for block in &self.blocks {
            block.to_html(f, context)?;
        }
        Ok(())
    }
}

impl<'a> ToHtml for Block<'a> {
    fn to_html<W: Write>(&self, f: &mut W, context: &mut Context) -> Result {
        match self {
            Block::Book(book) => {
                context.book_code = Some(book.code);
                book.to_html(f, context)
            }
            Block::ChapterStart(chapter) => {
                context.chapter_number = Some(chapter.number);
                chapter.to_html(f, context)
            }
            Block::ChapterEnd(chapter) => {
                let result = chapter.to_html(f, context);
                context.chapter_number = None;
                result
            }
            Block::Para(para) => para.to_html(f, context),
            Block::Table(table) => table.to_html(f, context),
            Block::Milestone(milestone) => milestone.to_html(f, context),
            Block::Sidebar(sidebar) => sidebar.to_html(f, context),
            Block::Periph(periph) => periph.to_html(f, context),
        }
    }
}

impl<'a> ToHtml for Periph<'a> {
    fn to_html<W: Write>(&self, f: &mut W, context: &mut Context) -> Result {
        write!(f, "<section class=\"{}\"", context.rule(self.style).marker)?;
        if let Some(attributes) = &self.attributes {
            for attr in &attributes.pairs {
                if attr.name.is_empty() || attr.name == "id" {
                    write!(f, " id=\"")?;
                    write_escaped_attribute(f, &attr.value)?;
                    write!(f, "\"")?;
                }
            }
        }
        writeln!(f, ">")?;
        if let Some(title) = &self.title {
            write!(f, "<h1>")?;
            write_escaped(f, &title.content)?;
            writeln!(f, "</h1>")?;
        }
        for block in &self.blocks {
            block.to_html(f, context)?;
        }
        writeln!(f, "</section>")
    }
}

impl<'a> ToHtml for Sidebar<'a> {
    fn to_html<W: Write>(&self, f: &mut W, context: &mut Context) -> Result {
        write!(f, "<aside class=\"{}\"", context.rule(self.style).marker)?;
        if let Some(category) = &self.category {
            write!(f, " data-category=\"")?;
            write_escaped_attribute(f, &category.content)?;
            write!(f, "\"")?;
        }
        writeln!(f, ">")?;
        for block in &self.blocks {
            block.to_html(f, context)?;
        }
        writeln!(f, "</aside>")
    }
}

impl<'a> ToHtml for Inline<'a> {
    fn to_html<W: Write>(&self, f: &mut W, context: &mut Context) -> Result {
        match self {
            Inline::Text(text) => write_escaped(f, text),
            Inline::VerseStart(verse) => {
                context.verse_number = Some(verse.number.clone());
                verse.to_html(f, context)
            }
            Inline::VerseEnd(verse) => {
                let result = verse.to_html(f, context);
                context.verse_number = None;
                result
            }
            Inline::Char(char) => char.to_html(f, context),
            Inline::Note(note) => note.to_html(f, context),
            Inline::Milestone(milestone) => milestone.to_html(f, context),
            Inline::OptBreak(opt_break) => opt_break.to_html(f, context),
        }
    }
}

impl ToHtml for OptBreak {
    /// A word-break opportunity is what `//` asks for.
    fn to_html<W: Write>(&self, f: &mut W, _context: &mut Context) -> Result {
        f.write_str("<wbr>")
    }
}

impl<'a> ToHtml for Milestone<'a> {
    fn to_html<W: Write>(&self, f: &mut W, context: &mut Context) -> Result {
        let marker = &context.rule(self.style).marker;

        // `write_str` rather than `write!` throughout: an alignment-heavy
        // document is mostly milestones with several attributes each, and the
        // formatting machinery is measurable there (`docs/benchmarks.md`,
        // "After ticket 14").
        f.write_str("<span class=\"ms ")?;
        f.write_str(marker)?;
        f.write_str("\"")?;

        for attr in self.pairs() {
            // A name that is not a valid XML/HTML attribute name cannot be
            // written as one; the USX writer drops such a pair too
            // (`usfm_ast::is_valid_attribute_name`), and the source it came
            // from is malformed enough to have been reported already.
            if is_valid_attribute_name(&attr.name) {
                f.write_str(" data-")?;
                f.write_str(&attr.name)?;
                f.write_str("=\"")?;
                write_escaped_attribute(f, &attr.value)?;
                f.write_str("\"")?;
            }
        }

        // Milestones are rendered as self-closing spans with data attributes
        f.write_str("></span>")
    }
}

impl<'a> ToHtml for Book<'a> {
    fn to_html<W: Write>(&self, f: &mut W, _context: &mut Context) -> Result {
        // A book code is three ASCII letters or digits, so it needs no
        // escaping; the description is whatever followed `\id`.
        write!(f, "<div class=\"id\" data-code=\"{}\">", self.code)?;
        write_escaped(f, &self.description)?;
        write!(f, "</div>")
    }
}

impl<'a> ToHtml for ChapterStart<'a> {
    fn to_html<W: Write>(&self, _f: &mut W, context: &mut Context) -> Result {
        context.metadata.insert(
            "chapter_to_output".to_string(),
            self.pub_number
                .as_ref()
                .map(|number| number.to_string())
                .unwrap_or(self.number.to_string()),
        );
        Ok(())
    }
}

impl ToHtml for ChapterEnd {
    fn to_html<W: Write>(&self, _f: &mut W, _context: &mut Context) -> Result {
        Ok(())
    }
}

impl<'a> ToHtml for VerseStart<'a> {
    fn to_html<W: Write>(&self, f: &mut W, _context: &mut Context) -> Result {
        f.write_str("<span class=\"v start\">")?;
        write!(f, "{}", self.number)?;
        f.write_str("</span>")
    }
}

impl ToHtml for VerseEnd {
    fn to_html<W: Write>(&self, f: &mut W, _context: &mut Context) -> Result {
        f.write_str("<span class=\"v end\">")?;
        write!(f, "{}", self.number)?;
        f.write_str("</span>")
    }
}

impl<'a> HtmlElement for Para<'a> {
    fn tag(&self, context: &Context<'_>) -> &'static str {
        match context.rule(self.style).marker.as_str() {
            "s" | "s1" | "mt1" => "h1",
            "s2" | "mt2" => "h2",
            "s3" | "mt3" => "h3",
            "s4" | "mt4" => "h4",
            "s5" | "mt5" => "h5",
            "s6" | "mt6" => "h6",
            _ => "p",
        }
    }
}

impl<'a> ToHtml for Para<'a> {
    fn to_html<W: Write>(&self, f: &mut W, context: &mut Context) -> Result {
        let rule = context.rule(self.style);
        let chapter_to_output = if rule.is_verse_text() {
            context.metadata.remove("chapter_to_output")
        } else {
            None
        };
        let tag = self.tag(context);
        f.write_str("<")?;
        f.write_str(tag)?;
        f.write_str(" class=\"")?;
        f.write_str(&rule.marker)?;
        f.write_str("\">")?;
        if !self.children.is_empty() || chapter_to_output.is_some() {
            f.write_str("\n  ")?;
            if let Some(chapter_to_output) = chapter_to_output {
                // `\cp` puts arbitrary text where the chapter number goes.
                f.write_str("<span class=\"c\">")?;
                write_escaped(f, &chapter_to_output)?;
                f.write_str("</span>")?;
            }
            for child in &self.children {
                child.to_html(f, context)?;
            }
            writeln!(f)?;
        }
        writeln!(f, "</{tag}>")?;
        Ok(())
    }
}

impl<'a> ToHtml for Char<'a> {
    fn to_html<W: Write>(&self, f: &mut W, context: &mut Context) -> Result {
        let rule = context.rule(self.style);
        if rule.marker == "flag" {
            let id = context.next_id();
            write!(
                f,
                "<button popovertarget=\"{id}\" class=\"{}\"></button>",
                rule.marker
            )?;
            write!(
                f,
                "<span popover id=\"{id}\" class=\"{}-content\">",
                rule.marker
            )?;
            for child in &self.children {
                child.to_html(f, context)?;
            }
            write!(f, "</span>")?;
            return Ok(());
        }
        f.write_str("<span class=\"")?;
        f.write_str(&rule.marker)?;
        f.write_str("\">")?;
        for child in &self.children {
            child.to_html(f, context)?;
        }
        f.write_str("</span>")?;
        Ok(())
    }
}

impl<'a> ToHtml for Note<'a> {
    fn to_html<W: Write>(&self, f: &mut W, context: &mut Context) -> Result {
        let number = context.get_and_increment_note_number(&self.caller);
        let id = format!(
            "note-{}{}",
            if matches!(self.caller, Caller::Plus) {
                ""
            } else {
                "c"
            },
            number
        );
        let content = match &self.caller {
            Caller::Plus => number.to_string(),
            Caller::Minus => String::new(),
            Caller::Custom(custom) => custom.to_string(),
        };
        let rule = context.rule(self.style);
        write!(
            f,
            "<button class=\"note {}-trigger\" popovertarget=\"{}\">",
            rule.marker, id
        )?;
        // A custom caller (`\f * …`) is text from the source.
        write_escaped(f, &content)?;
        write!(f, "</button>")?;
        write!(f, "<span id=\"{}\" class=\"{}\" popover>", id, rule.marker)?;
        for child in &self.children {
            child.to_html(f, context)?;
        }
        write!(f, "</span>")?;
        Ok(())
    }
}

impl<'a> ToHtml for Table<'a> {
    fn to_html<W: Write>(&self, f: &mut W, context: &mut Context) -> Result {
        write!(f, "<table>\n  <tbody>\n")?;
        for row in &self.rows {
            row.to_html(f, context)?;
        }
        write!(f, "  </tbody>\n</table>\n")?;
        Ok(())
    }
}

impl<'a> ToHtml for TableRow<'a> {
    fn to_html<W: Write>(&self, f: &mut W, context: &mut Context) -> Result {
        writeln!(f, "  <tr>")?;
        for cell in &self.cells {
            let column = cell.column;
            write!(
                f,
                "    <td style=\"t{}{}{}\" align=\"{}\"",
                if cell.header { 'h' } else { 'c' },
                match cell.alignment {
                    Alignment::Start => "",
                    Alignment::Center => "c",
                    Alignment::End => "r",
                },
                if cell.colspan > 1 {
                    format!("{}-{}", column, column + cell.colspan - 1)
                } else {
                    column.to_string()
                },
                cell.alignment
            )?;
            if cell.colspan > 1 {
                write!(f, " colspan=\"{}\"", cell.colspan)?;
            }
            write!(f, ">")?;
            for child in &cell.children {
                child.to_html(f, context)?;
            }
            writeln!(f, "</td>")?;
        }
        writeln!(f, "  </tr>")?;
        Ok(())
    }
}

/// Helper function to convert any ToHtml implementor to an HTML string
///
/// This provides a convenient way to use the ToHtml trait without dealing with
/// formatters and contexts directly.
///
/// # Arguments
/// * `node` - The AST node to convert to HTML
/// * `style_sheet` - The style sheet to use for formatting
///
/// # Returns
/// A String containing the HTML representation of the node
pub fn to_html_string<T: ToHtml>(node: &T, style_sheet: &usfm_style::StyleSheet) -> String {
    struct ToHtmlWrapper<'a, T: ToHtml> {
        node: &'a T,
        style_sheet: &'a usfm_style::StyleSheet,
    }

    impl<'a, T: ToHtml> Display for ToHtmlWrapper<'a, T> {
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            let mut context = Context::new(self.style_sheet);
            self.node.to_html(f, &mut context)
        }
    }

    format!("{}", ToHtmlWrapper { node, style_sheet })
}

/// Helper function to serialize a document using a SerializeHtml implementation
///
/// This provides a convenient way to use custom SerializeHtml implementations
/// without dealing with formatters and contexts directly.
///
/// # Arguments
/// * `document` - The document to serialize
/// * `style_sheet` - The style sheet to use for formatting
/// * `serializer` - The SerializeHtml implementation to use
///
/// # Returns
/// A String containing the HTML representation of the document
pub fn serialize_html<S: SerializeHtml>(
    document: &Document,
    style_sheet: &usfm_style::StyleSheet,
    serializer: S,
) -> String {
    struct SerializeHtmlWrapper<'a, S: SerializeHtml> {
        document: &'a Document<'a>,
        style_sheet: &'a usfm_style::StyleSheet,
        serializer: S,
    }

    impl<'a, S: SerializeHtml> Display for SerializeHtmlWrapper<'a, S> {
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            let mut context = Context::new(self.style_sheet);
            self.serializer
                .serialize_document(f, &mut context, self.document)
        }
    }

    format!(
        "{}",
        SerializeHtmlWrapper {
            document,
            style_sheet,
            serializer
        }
    )
}

#[cfg(test)]
mod tests {
    use usfm_parser::{DEFAULT_STYLESHEET, parser::Parser};

    use super::*;

    /// Parse a fragment and render it with the document's own stylesheet
    /// (hardening plan D3), which is what every caller should do.
    fn html(source: &str) -> String {
        let document = Parser::new(source).parse(&DEFAULT_STYLESHEET).document;
        let style_sheet = std::sync::Arc::clone(document.style_sheet());
        to_html_string(&document, &style_sheet)
    }

    #[test]
    fn test_to_html_trait() {
        let document = Parser::new(
            r#"
        \c 1
        \s This is a title
        \p \v 1 This is a \em test\em*\f + \ft This is a footnote\f*.
        \p This is another. \v 2 More content.
        "#,
        )
        .parse(&DEFAULT_STYLESHEET)
        .document;

        let html_output = to_html_string(&document, &DEFAULT_STYLESHEET);
        println!("ToHtml output: {}", html_output);
        assert!(html_output.contains("<h1 class=\"s\">"));
        assert!(html_output.contains("<span class=\"v start\">1</span>"));
        assert!(html_output.contains("<span class=\"v end\">1</span>"));
    }

    /// `&`, `<` and `>` in the source are content, not markup: an HTML parser
    /// has to read back what the AST held.
    #[test]
    fn reserved_characters_in_text_are_escaped() {
        let output = html(r#"\p Tom & <Jerry> "said" it's"#);
        assert!(
            output.contains(r#"Tom &amp; &lt;Jerry&gt; "said" it's"#),
            "{output}"
        );
    }

    /// Text is escaped wherever it is written, not only in a paragraph: a
    /// book description, a note's custom caller, `\cp`'s replacement chapter
    /// number, a sidebar category and a milestone's attribute value.
    #[test]
    fn reserved_characters_are_escaped_everywhere_text_is_written() {
        let output = html(r#"\id GEN A & B"#);
        assert!(output.contains("A &amp; B"), "{output}");

        let output = html(r#"\p \f <c> \ft note\f*"#);
        assert!(output.contains(">&lt;c&gt;</button>"), "{output}");

        let output = html("\\c 1\n\\cp <A>\n\\p text");
        assert!(
            output.contains(r#"<span class="c">&lt;A&gt;</span>"#),
            "{output}"
        );

        let output = html("\\esb \\cat <x>\\cat*\n\\p text\n\\esbe");
        assert!(output.contains(r#"data-category="&lt;x&gt;""#), "{output}");

        let output = html(r#"\p \qt-s |who="a\"b<c"\*text"#);
        assert!(output.contains(r#"data-who="a&quot;b&lt;c""#), "{output}");
    }

    /// The characters HTML cannot carry are replaced by U+FFFD wherever they
    /// reach the writer, exactly as the USX writer replaces them.
    #[test]
    fn characters_html_forbids_are_replaced() {
        let output = html("\\p a\u{0}b\u{fffe}c");
        assert!(output.contains("a\u{fffd}b\u{fffd}c"), "{output:?}");
        assert!(!output.contains('\u{0}'), "{output:?}");

        let output = html("\\p \\qt-s |who=\"x\u{1}y\"\\*text");
        assert!(output.contains("data-who=\"x\u{fffd}y\""), "{output:?}");
    }

    /// An attribute whose name is not a valid HTML attribute name cannot be
    /// written as one, so the pair is dropped — as the USX writer drops it.
    #[test]
    fn milestone_attribute_with_an_unwritable_name_is_dropped() {
        let output = html(r#"\p \qt-s |who="a" x<y="b"\*text"#);
        assert!(output.contains(r#"data-who="a""#), "{output}");
        assert!(!output.contains("x<y"), "{output}");
    }
}
