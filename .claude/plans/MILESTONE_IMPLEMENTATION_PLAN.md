# Milestone Feature Implementation Plan

## Overview

This plan outlines the implementation of USFM milestone support for the usfm-tools parser. Milestones are self-closing markers used to denote spans of text (quotations, translator sections, etc.) without requiring nesting.

## USFM Milestone Syntax

```usfm
\qt-s |sid="qt_123" who="Pilate"\*"Are you the king of the Jews?"\qt-e |eid="qt_123"\*
\ts-s|sid="ts_JUD_5-6"\*
\ts\*
\zms\*
```

Key syntax rules:
- Milestones are self-closing, ending with `\*` (backslash-star)
- Attributes follow a pipe `|` character (e.g., `|sid="qt_123" who="Pilate"`)
- Start markers have `-s` suffix with `sid` attribute
- End markers have `-e` suffix with `eid` attribute
- Some milestones are standalone (no start/end, e.g., `\ts\*`, `\zms\*`)

## Expected USX Output

```xml
<ms style="qt-s" sid="qt_123" who="Pilate" />
<ms style="qt-e" eid="qt_123" />
<ms style="ts-s" sid="ts_JUD_5-6" />
<ms style="ts"/>
<ms style="zms"/>
```

---

## Implementation Tasks

### Task 1: Add Milestone AST Node

**File:** `usfm_ast/src/lib.rs`

Add a new `Milestone` struct and add it to the `Inline` enum:

```rust
/// A milestone marker - self-closing markers that denote spans without nesting.
/// Examples: \qt-s (quotation start), \qt-e (quotation end), \ts (translator section)
#[derive(Debug, PartialEq)]
pub struct Milestone<'a> {
    /// The style index referencing the marker in the stylesheet
    pub style: usize,
    /// Attributes (sid, eid, who, etc.)
    pub attributes: Attributes<'a>,
}
```

Update the `Inline` enum:
```rust
pub enum Inline<'a> {
    Text(Cow<'a, str>),
    VerseStart(VerseStart<'a>),
    VerseEnd(VerseEnd),
    Char(Char<'a>),
    Note(Note<'a>),
    Chunk(Chunk<'a>),
    Attributes(Attributes<'a>),
    Milestone(Milestone<'a>),  // NEW
}
```

---

### Task 2: Add Milestone Markers to Stylesheet

**File:** `usfm_parser/src/generated/mod.rs`

Add milestone marker definitions. These markers should be added:

| Marker | Description |
|--------|-------------|
| `qt-s` | Quotation start |
| `qt-e` | Quotation end |
| `qt1-s` through `qt5-s` | Nested quotation start (levels 1-5) |
| `qt1-e` through `qt5-e` | Nested quotation end (levels 1-5) |
| `ts-s` | Translator section start |
| `ts-e` | Translator section end |
| `ts` | Standalone translator section |
| `zms` | Custom/extension milestone |

Example entry:
```rust
StyleRule {
    marker: "qt-s".into(),
    style_type: StyleType::Milestone,
    text_type: TextType::Other,
    text_properties: TextProperties::from_bits(0).unwrap(),
},
```

---

### Task 3: Implement Milestone Parsing

**File:** `usfm_parser/src/parser.rs` (around line 505)

Replace `todo!("milestone style type")` with milestone parsing logic:

```rust
StyleType::Milestone => {
    // Milestones are always self-closing
    // They may have attributes after a pipe |
    // They end with \*

    // 1. Check for optional attributes (pipe followed by attributes)
    let attributes = if self.lexer.token.kind == Kind::Pipe {
        self.bump_any(); // consume '|'
        self.parse_attributes()?
    } else {
        Attributes { pairs: vec![] }
    };

    // 2. Expect the closing \*
    self.eat_whitespace();
    if self.lexer.token.kind == Kind::Backslash {
        self.bump_any();
        if self.lexer.token.kind == Kind::Star {
            self.bump_any(); // consume '*'
        }
    }

    // 3. Create and add the milestone node
    context.add_child(Inline::Milestone(Milestone {
        style: marker,
        attributes,
    }));
}
```

**Parser context considerations:**
- Milestones don't have children (self-closing)
- Milestones can appear in any inline context (paragraphs, table cells, etc.)
- No need to track open/close state in the parser (that's semantic, not syntactic)

---

### Task 4: Implement USX Serialization for Milestone

**File:** `usfm_parser/src/usx.rs`

Add `ToUsx` implementation for `Milestone`:

```rust
impl<'a> ToUsx for Milestone<'a> {
    fn to_usx(&self, context: &mut Context) -> XmlNode {
        let marker = &context.style_sheet.get_rule(self.style).marker;

        // Build attributes list, starting with style
        let mut attrs = vec![xml::attribute::OwnedAttribute::new(
            xml::name::OwnedName::local("style"),
            marker.to_string(),
        )];

        // Add milestone-specific attributes (sid, eid, who, etc.)
        for attr in &self.attributes.pairs {
            if !attr.name.is_empty() {
                attrs.push(xml::attribute::OwnedAttribute::new(
                    xml::name::OwnedName::local(attr.name.to_string()),
                    attr.value.to_string(),
                ));
            }
        }

        // Milestones are self-closing <ms> elements with no children
        XmlNode::Element(XmlElement {
            name: xml::name::OwnedName::local("ms"),
            attributes: attrs,
            namespace: xml::namespace::Namespace::empty(),
            children: vec![],
        })
    }
}
```

Update the `Inline::to_usx` match arm:
```rust
impl<'a> ToUsx for Inline<'a> {
    fn to_usx(&self, context: &mut Context) -> XmlNode {
        match self {
            // ... existing arms ...
            Inline::Milestone(milestone) => milestone.to_usx(context),
        }
    }
}
```

---

### Task 5: Update Visitor Pattern

**File:** `usfm_parser/src/visit_mut.rs`

Add milestone visitor methods:

```rust
// In VisitMut trait
fn visit_mut_milestone(&mut self, context: &Context, milestone: &mut Milestone) {}

// In WalkMut for Inline
impl<V> WalkMut<V> for Inline<'_>
where
    V: VisitMut,
{
    fn walk_mut(&mut self, context: &Context, visitor: &mut V) {
        match self {
            // ... existing arms ...
            Inline::Milestone(milestone) => visitor.visit_mut_milestone(context, milestone),
        }
    }
}
```

---

### Task 6: Handle Edge Cases

**Additional considerations:**

1. **Whitespace handling:** Milestones may have optional whitespace before `\*`
   - `\qt-s |sid="x"\*` (no space)
   - `\qt-s |sid="x" \*` (space before closing)

2. **Empty attribute handling:** Some milestones have no attributes
   - `\ts\*`
   - `\zms\*`

3. **Unknown milestone markers:** The parser should handle custom/extension milestones (z-prefixed markers like `\zms`)

---

## Testing

### Existing Test Cases

The implementation can be validated against existing tests:
- `tcdocs/tests/advanced/milestones/` - Basic milestone test
- `tcdocs/tests/specExamples/milestone/` - Comprehensive milestone examples

### Test Commands

```bash
# Run all tests
cargo run --package usfm_tests

# Run specific milestone tests
cargo run --package usfm_tests advanced
cargo run --package usfm_tests specExamples
```

### Expected Test Input/Output

**Input (from `tcdocs/tests/advanced/milestones/origin.usfm`):**
```usfm
\id GEN
\c 1
\p
\v 1 the first verse
\v 2 the second verse
\v 3
\qt-s |sid="qt_123" who="Pilate" \*"Are you the king of the Jews?"
\qt-e |eid="qt_123" \*
```

**Expected Output:**
```xml
<ms style="qt-s" sid="qt_123" who="Pilate" />"Are you the king of the Jews?" <ms style="qt-e" eid="qt_123" />
```

---

## Implementation Order

1. **Task 1:** Add AST node (foundational)
2. **Task 2:** Add stylesheet markers (required for parsing)
3. **Task 3:** Implement parsing (core functionality)
4. **Task 4:** Implement USX serialization (required for tests)
5. **Task 5:** Update visitor pattern (completeness)
6. **Task 6:** Handle edge cases (robustness)

---

## Files to Modify

| File | Changes |
|------|---------|
| `usfm_ast/src/lib.rs` | Add `Milestone` struct, update `Inline` enum |
| `usfm_parser/src/generated/mod.rs` | Add milestone marker definitions |
| `usfm_parser/src/parser.rs` | Replace `todo!()` with parsing logic |
| `usfm_parser/src/usx.rs` | Add `ToUsx` implementation |
| `usfm_parser/src/visit_mut.rs` | Add visitor method |

---

## Success Criteria

1. Parser correctly handles all milestone syntax variants
2. USX output matches expected format (`<ms>` elements)
3. Milestone tests in `tcdocs/tests/advanced/milestones/` pass
4. Milestone tests in `tcdocs/tests/specExamples/milestone/` pass
5. No regressions in existing tests
