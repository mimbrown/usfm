# HTML Serialization in `usfm_html`

`usfm_html` provides an extensible HTML serialization system with two complementary approaches: the `ToHtml` trait for direct node conversion and the `SerializeHtml` trait for customizable serialization.

## Overview

The system provides multiple approaches for generating HTML from parsed USFM documents:

1. **ToHtml Trait**: Direct conversion of AST nodes to HTML
2. **SerializeHtml Trait**: Extensible serialization with default implementations
3. **Helper Functions**: Convenient functions for common use cases

### ToHtml Trait

```rust
pub trait ToHtml {
    fn to_html(&self, f: &mut Formatter<'_>, context: &mut Context) -> Result;
}
```

### SerializeHtml Trait

```rust
pub trait SerializeHtml {
    fn serialize_document(&self, f: &mut Formatter<'_>, context: &mut Context, document: &Document) -> Result;
    fn serialize_verse_start(&self, f: &mut Formatter<'_>, context: &mut Context, verse: &VerseStart) -> Result;
    // ... other methods with default implementations
}
```

## Key Benefits

1. **Extensibility**: Override only the methods you need to customize
2. **Type Safety**: Compile-time guarantees about HTML generation
3. **Performance**: Direct formatting without intermediate representations
4. **Flexibility**: Multiple approaches for different use cases
5. **Simplicity**: Clean API without wrapper types

## Basic Usage

### Simple ToHtml Conversion

```rust
use usfm_html::to_html_string;
use usfm_parser::{DEFAULT_STYLESHEET, parser::Parser};

let document = Parser::new(r#"\c 1\p \v 1 Hello world!"#)
    .parse(&DEFAULT_STYLESHEET)?;

let html = to_html_string(&document, &DEFAULT_STYLESHEET);
println!("{}", html);
```

## Advanced Customization

### Custom Wrapper

You can create custom wrappers that modify the HTML output:

```rust
use std::fmt::{Display, Formatter, Result};
use usfm_ast::Document;
use usfm_html::{context::Context, serialize_html::ToHtml};

struct CustomHtmlWrapper<'a> {
    document: &'a Document<'a>,
    style_sheet: &'a usfm_style::StyleSheet,
    add_wrapper_div: bool,
}

impl<'a> Display for CustomHtmlWrapper<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        if self.add_wrapper_div {
            write!(f, "<div class=\"bible-text\">")?;
        }
        
        let mut context = Context::new(self.style_sheet);
        self.document.to_html(f, &mut context)?;
        
        if self.add_wrapper_div {
            write!(f, "</div>")?;
        }
        
        Ok(())
    }
}
```

### Extending Individual Node Types

For more advanced customization, you can create new types that wrap existing AST nodes:

```rust
use usfm_ast::VerseStart;
use usfm_html::{context::Context, serialize_html::ToHtml};
use std::fmt::{Formatter, Result};

struct CustomVerse<'a>(&'a VerseStart<'a>);

impl<'a> ToHtml for CustomVerse<'a> {
    fn to_html(&self, f: &mut Formatter<'_>, _context: &mut Context) -> Result {
        write!(
            f,
            "<span class=\"verse-number enhanced\" data-verse=\"{}\">{}</span>",
            self.0.number, self.0.number
        )
    }
}
```

## Implementation Details

### AST Node Implementations

All major AST node types implement the `ToHtml` trait:

- `Document` - Iterates through blocks
- `Block` - Dispatches to specific block types (Book, Chapter, Para, Table)
- `Inline` - Dispatches to specific inline types (Text, Verse, Char, Note, Chunk)
- `Para` - Renders as appropriate HTML elements (h1-h6, p) based on style
- `Char` - Renders as span with style classes, handles special cases like flags
- `Note` - Renders as popover buttons with note content
- `Table` - Renders as HTML table with proper structure
- And more...

### Context and State Management

The `Context` object maintains serialization state:

- Current book, chapter, and verse information
- Style sheet for looking up formatting rules
- Metadata for cross-node communication
- Counters for generating unique IDs

### Error Handling

All `ToHtml` implementations return `std::fmt::Result`, allowing for proper error propagation during HTML generation.

## Examples

See `usfm_html/examples/custom_html_serialization.rs` for complete working examples of:

- Basic usage with `to_html_string()`
- Custom wrappers for specialized formatting
- Integration with existing USFM parsing workflows
- Testing custom HTML implementations

## Performance Considerations

The `ToHtml` trait is designed for performance:

- Direct formatting without intermediate allocations
- Efficient delegation between parent and child nodes
- Lazy evaluation of HTML generation

This makes it suitable for both small documents and large-scale batch processing scenarios.
