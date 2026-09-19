use std::fmt::{Formatter, Result, Write};
use usfm_parser::{
    DEFAULT_STYLESHEET,
    ast::*,
    context::Context,
    parser::Parser,
    serialize_html::{SerializeHtml, ToHtml, serialize_html, to_html_string},
};

/// Example showing how to create custom HTML serialization using the ToHtml trait
fn main() {
    let usfm_text = r#"
        \id GEN Genesis
        \c 1
        \s In the Beginning
        \p \v 1 In the beginning God created the heavens and the earth.
        \v 2 Now the earth was formless and empty, darkness was over the surface of the deep.
        \p \v 3 And God said, "Let there be light," and there was light.
        \f + \ft This is a footnote about light.\f*
    "#;

    let result = Parser::new(usfm_text).parse(&DEFAULT_STYLESHEET);
    for diagnostic in &result.diagnostics {
        eprintln!("{diagnostic}");
    }
    let document = result.document;

    println!("=== Basic ToHtml Usage ===");
    let basic_html = to_html_string(&document, &DEFAULT_STYLESHEET);
    println!("{}", basic_html);

    println!("\n=== Custom Implementation Example ===");
    // Example of how you could create a custom wrapper for specific formatting
    let custom_html = CustomHtmlWrapper::new(&document, &DEFAULT_STYLESHEET)
        .with_custom_verse_style()
        .to_string();
    println!("{}", custom_html);

    println!("\n=== SerializeHtml Trait Example ===");
    // Example using the new SerializeHtml trait for extensible customization
    let custom_serializer = CustomHtmlSerializer::new();
    let custom_serialized = serialize_html(&document, &DEFAULT_STYLESHEET, custom_serializer);
    println!("{}", custom_serialized);
}

/// Example of a custom HTML wrapper that demonstrates extensibility
struct CustomHtmlWrapper<'a> {
    document: &'a Document<'a>,
    style_sheet: &'a usfm_style::StyleSheet,
    custom_verse_style: bool,
}

impl<'a> CustomHtmlWrapper<'a> {
    fn new(document: &'a Document<'a>, style_sheet: &'a usfm_style::StyleSheet) -> Self {
        Self {
            document,
            style_sheet,
            custom_verse_style: false,
        }
    }

    fn with_custom_verse_style(mut self) -> Self {
        self.custom_verse_style = true;
        self
    }
}

impl<'a> std::fmt::Display for CustomHtmlWrapper<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        if self.custom_verse_style {
            // Example: Custom verse styling
            write!(f, "<div class=\"custom-bible-text\">")?;
            self.document
                .to_html(f, &mut Context::new(self.style_sheet))?;
            write!(f, "</div>")?;
        } else {
            self.document
                .to_html(f, &mut Context::new(self.style_sheet))?;
        }
        Ok(())
    }
}

/// Example of how you could extend individual AST nodes with custom HTML behavior
/// This shows how the ToHtml trait enables extensible customization
pub struct CustomVerseRenderer;

impl CustomVerseRenderer {
    /// Example method showing how you could customize verse rendering
    pub fn render_verse_with_custom_style(
        verse: &VerseStart,
        f: &mut Formatter<'_>,
        _context: &mut Context,
    ) -> Result {
        // Custom verse rendering with additional CSS classes and attributes
        write!(
            f,
            "<span class=\"verse-number custom-verse\" data-verse=\"{}\">{}</span>",
            verse.number, verse.number
        )
    }
}

/// Example custom HTML serializer using the SerializeHtml trait
/// This demonstrates how to create extensible HTML serializers by overriding specific methods
pub struct CustomHtmlSerializer {
    add_custom_classes: bool,
}

impl Default for CustomHtmlSerializer {
    fn default() -> Self {
        Self::new()
    }
}

impl CustomHtmlSerializer {
    pub fn new() -> Self {
        Self {
            add_custom_classes: true,
        }
    }
}

impl SerializeHtml for CustomHtmlSerializer {
    // Override verse rendering to add custom styling
    fn serialize_verse_start<W: Write>(
        &self,
        f: &mut W,
        _context: &mut Context,
        verse: &VerseStart,
    ) -> Result {
        if self.add_custom_classes {
            write!(
                f,
                "<span class=\"verse-number enhanced-verse\" data-verse=\"{}\">{}</span>",
                verse.number, verse.number
            )
        } else {
            // Fall back to default implementation
            verse.to_html(f, _context)
        }
    }

    // Override paragraph rendering to add custom wrapper
    fn serialize_para<W: Write>(&self, f: &mut W, context: &mut Context, para: &Para) -> Result {
        let rule = context.rule(para.style);
        if rule.marker == "p" && self.add_custom_classes {
            write!(f, "<div class=\"custom-paragraph-wrapper\">")?;
            para.to_html(f, context)?;
            write!(f, "</div>")?;
            Ok(())
        } else {
            // Use default implementation for other paragraph types
            para.to_html(f, context)
        }
    }

    // All other methods use default implementations from the trait
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_custom_html_wrapper() {
        let document = Parser::new(r#"\c 1\p \v 1 Test verse."#)
            .parse(&DEFAULT_STYLESHEET)
            .document;

        let custom_html = CustomHtmlWrapper::new(&document, &DEFAULT_STYLESHEET)
            .with_custom_verse_style()
            .to_string();

        assert!(custom_html.contains("<div class=\"custom-bible-text\">"));
        assert!(custom_html.contains("</div>"));
    }

    #[test]
    fn test_basic_to_html() {
        let document = Parser::new(r#"\c 1\p \v 1 Test verse."#)
            .parse(&DEFAULT_STYLESHEET)
            .document;

        let html = to_html_string(&document, &DEFAULT_STYLESHEET);
        assert!(html.contains("<span class=\"v\">1</span>"));
        assert!(html.contains("Test verse."));
    }

    #[test]
    fn test_custom_html_serializer() {
        let document = Parser::new(r#"\c 1\p \v 1 Test verse."#)
            .parse(&DEFAULT_STYLESHEET)
            .document;

        let custom_serializer = CustomHtmlSerializer::new();
        let html = serialize_html(&document, &DEFAULT_STYLESHEET, custom_serializer);

        // Should contain custom verse styling
        assert!(html.contains("enhanced-verse"));
        assert!(html.contains("custom-paragraph-wrapper"));
        println!("Custom serializer output: {}", html);
    }
}
