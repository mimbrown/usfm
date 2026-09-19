mod book;
mod chapter;
pub mod cursor;
pub mod fold;
mod inline_container;
mod into_owned;
mod number;
mod parse_error;
mod periph;
pub mod reference;
mod sidebar;
pub mod span;
pub mod string_parser;
mod styled;
mod table;
#[cfg(test)]
mod test_fixtures;
pub mod text;
mod verse;
#[macro_use]
pub mod visit;
pub mod visit_mut;

use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;

use usfm_style::{StyleRule, StyleSheet};

pub use book::*;
pub use chapter::*;
pub use cursor::*;
pub use inline_container::*;
pub use into_owned::*;
pub use number::*;
pub use parse_error::*;
pub use periph::*;
pub use reference::{ChapterRef, NodePath, ReferenceIndex, VerseRef};
pub use sidebar::*;
pub use span::{SPAN, Span};
pub use styled::*;
pub use table::*;
pub use verse::*;

/// A resolved style, as an index into the owning [`Document`]'s stylesheet.
///
/// Consumers never construct or arithmetic on the raw index: resolve it with
/// [`Document::style`] or [`Document::marker`]. The newtype exists so a style
/// cannot be confused with any other index (hardening plan D3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StyleId(u32);

impl StyleId {
    /// Wrap a stylesheet index. Only a parser (or a test building a tree by
    /// hand) should need this; it is unchecked against any stylesheet.
    pub const fn new(index: u32) -> Self {
        Self(index)
    }

    /// The underlying index. Use [`Document::style`] instead where possible.
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

impl fmt::Display for StyleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "style#{}", self.0)
    }
}

/// A parsed USFM document, together with the stylesheet its [`StyleId`]s
/// resolve against.
///
/// The document owns the stylesheet (`Arc`, so cloning a document's sheet is
/// cheap) because the parser may extend it while parsing: derived milestone
/// forms such as `\k-s` are registered on the fly, and indices into an
/// extended sheet would be meaningless to a caller holding only the base
/// sheet. Any serializer leaving the process writes the marker name, never
/// the index.
pub struct Document<'a> {
    pub blocks: Vec<Block<'a>>,
    style_sheet: Arc<StyleSheet>,
}

impl<'a> Document<'a> {
    pub fn new(blocks: Vec<Block<'a>>, style_sheet: Arc<StyleSheet>) -> Self {
        Self {
            blocks,
            style_sheet,
        }
    }

    /// A document with an empty stylesheet, for trees built by hand where no
    /// style is ever resolved. [`Document::style`] panics on any `StyleId`
    /// from such a document.
    pub fn without_styles(blocks: Vec<Block<'a>>) -> Self {
        Self {
            blocks,
            style_sheet: Arc::new(StyleSheet::new(Vec::new())),
        }
    }

    /// The stylesheet every [`StyleId`] in this document resolves against.
    pub fn style_sheet(&self) -> &Arc<StyleSheet> {
        &self.style_sheet
    }

    /// Resolve a style to its rule.
    ///
    /// # Panics
    /// If `id` did not come from this document.
    pub fn style(&self, id: StyleId) -> &StyleRule {
        self.style_sheet.get_rule(id.index())
    }

    /// The marker name for a style, e.g. `"p"`, without the leading `\`.
    pub fn marker(&self, id: StyleId) -> &str {
        &self.style(id).marker
    }

    /// Index the chapters and verses (plan D4). The index borrows the
    /// document, so it cannot go stale; build a new one after mutating.
    pub fn reference_index(&self) -> ReferenceIndex<'_> {
        ReferenceIndex::build(self)
    }

    /// The version declared by a `\usfm` marker, e.g. `"3.1"`, or `None` when
    /// the document has none.
    ///
    /// `\usfm` is kept in the tree as an ordinary paragraph so the AST stays
    /// faithful to the source; serializers that emit a version attribute (USX
    /// `<usx version>`) read it from here and skip the paragraph itself, see
    /// [`Para::is_usfm_version`].
    pub fn usfm_version(&self) -> Option<String> {
        self.blocks.iter().find_map(|block| match block {
            Block::Para(para) if para.is_usfm_version(self.style_sheet()) => {
                let version: String = para
                    .children
                    .iter()
                    .filter_map(|inline| match inline {
                        Inline::Text(text) => Some(text.content.as_ref()),
                        _ => None,
                    })
                    .collect();
                let version = version.trim();
                (!version.is_empty()).then(|| version.to_string())
            }
            _ => None,
        })
    }
}

// Hand-written so that comparing or printing a document does not drag in its
// entire stylesheet: two documents are equal when their trees are, and the
// debug output of a 1000-rule sheet is noise in a test failure.
impl PartialEq for Document<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.blocks == other.blocks
    }
}

impl fmt::Debug for Document<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Document")
            .field("blocks", &self.blocks)
            .field(
                "style_sheet",
                &format_args!("<{} rules>", self.style_sheet.rules.len()),
            )
            .finish()
    }
}

impl<'a> IntoIterator for Document<'a> {
    type Item = Block<'a>;
    type IntoIter = std::vec::IntoIter<Block<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.blocks.into_iter()
    }
}

#[derive(Debug, PartialEq)]
pub enum Block<'a> {
    Book(Book<'a>),
    ChapterStart(ChapterStart<'a>),
    ChapterEnd(ChapterEnd),
    Para(Para<'a>),
    Table(Table<'a>),
    /// A milestone between blocks rather than inside a paragraph, such as the
    /// `\\ts\\*` translator section marker that unfoldingWord writes between
    /// `\\c` and the first `\\p`. USX puts these at the same level as `<para>`.
    Milestone(Milestone<'a>),
    /// `\esb` … `\esbe`: a block that contains blocks.
    Sidebar(Sidebar<'a>),
    /// `\periph Title|id="x"` up to the next `\periph`: a division of
    /// peripheral matter, also a block that contains blocks.
    Periph(Periph<'a>),
}

#[derive(Debug, PartialEq)]
pub enum Inline<'a> {
    Text(Text<'a>),
    VerseStart(VerseStart<'a>),
    VerseEnd(VerseEnd),
    Char(Char<'a>),
    Note(Note<'a>),
    Milestone(Milestone<'a>),
    OptBreak(OptBreak),
}

/// The default (unnamed) attribute for a marker, as the USFM 3 spec assigns
/// them: `\w word|lemma\w*` means `lemma="lemma"`. `None` for markers without
/// one (`\fig` has several required attributes and no default).
pub fn default_attribute_name(marker: &str) -> Option<&'static str> {
    match marker {
        "w" => Some("lemma"),
        "rb" => Some("gloss"),
        "xt" | "jmp" => Some("link-href"),
        "ref" => Some("loc"),
        "periph" => Some("id"),
        // Quotation milestones: `\qt-s |Speaker\*`, also `\qt1-s` … `\qt5-s`.
        m if m.starts_with("qt") && (m.ends_with("-s") || m.ends_with("-e")) => Some("who"),
        _ => None,
    }
}

/// A run of text, with the source range it came from.
///
/// `content` is not the source verbatim and `&source[span]` is not expected to
/// equal it. The parser applies these rules, and only these, to text:
///
/// 1. A run of ASCII whitespace (space, tab, CR, LF) becomes one space. A
///    newline is whitespace like any other, so a line break inside a
///    paragraph is a space.
/// 2. Only ASCII whitespace counts. A no-break space (U+00A0), an
///    ideographic space (U+3000) and every other Unicode space is content,
///    never collapsed or trimmed.
/// 3. `~` becomes a no-break space (U+00A0), which is what it denotes.
/// 4. Escapes (`\\`, `\|`, `\"`) resolve to the literal character.
/// 5. Trailing whitespace is dropped where a run ends at a paragraph-level
///    boundary: before a paragraph, chapter or book marker, before the next
///    table cell, or at end of input. Before an inline marker — a closing
///    marker, a sibling character style, a note, a verse, a milestone — it
///    is kept: `\add foo \add*` holds `"foo "`.
/// 6. Leading whitespace after a marker is part of the marker, not the text.
///
/// The span answers "where did this text come from", which is what a language
/// server needs to map a position; it is deliberately not narrowed when the
/// content is rewritten, because the source offsets of normalised text cannot
/// be recovered from the content alone.
#[derive(Debug, PartialEq, Clone)]
pub struct Text<'a> {
    pub content: Cow<'a, str>,
    pub span: Span,
}

impl<'a> Text<'a> {
    pub fn new(content: impl Into<Cow<'a, str>>, span: Span) -> Self {
        Self {
            content: content.into(),
            span,
        }
    }

    /// A run with no source position, for synthesized text such as the space
    /// inserted before a verse-end milestone.
    pub fn synthesized(content: impl Into<Cow<'a, str>>) -> Self {
        Self::new(content, SPAN)
    }
}

impl std::ops::Deref for Text<'_> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.content
    }
}

impl fmt::Display for Text<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.content)
    }
}

/// An optional line break, `//` in the source: a point where a renderer
/// may break the line if it needs to. USX `<optbreak/>`. The whitespace on
/// either side stays in the neighbouring text (`Man // Who` is `"Man "`,
/// the break, `" Who"`); `//` inside a word splits it.
#[derive(Debug, PartialEq)]
pub struct OptBreak {
    /// Source range of the two slashes.
    pub span: Span,
}

/// A milestone marker - self-closing markers that denote spans without nesting.
/// Examples: \qt-s (quotation start), \qt-e (quotation end), \ts (translator section)
#[derive(Debug, PartialEq)]
pub struct Milestone<'a> {
    /// The milestone's style, resolved against the document's stylesheet.
    pub style: StyleId,
    /// Attributes (sid, eid, who, etc.)
    pub attributes: Attributes<'a>,
    /// Source range from the marker to the closing `\*`.
    pub span: Span,
}

#[derive(Debug, PartialEq)]
pub struct Para<'a> {
    pub style: StyleId,
    pub children: Vec<Inline<'a>>,
    /// Source range from the paragraph marker to the end of its content.
    pub span: Span,
}

impl Para<'_> {
    /// Whether this is the `\usfm` version declaration. It is metadata, not
    /// content: see [`Document::usfm_version`].
    pub fn is_usfm_version(&self, style_sheet: &StyleSheet) -> bool {
        style_sheet.get_rule(self.style.index()).marker == "usfm"
    }
}

#[derive(Debug, PartialEq)]
pub struct Char<'a> {
    pub style: StyleId,
    pub children: Vec<Inline<'a>>,
    /// Word-level attributes, when the source had a `|`. `Some` with no pairs
    /// is a bare `|`, which is not the same as no `|` at all.
    pub attributes: Option<Attributes<'a>>,
    /// Source range from the opening marker to the closing marker, or to
    /// wherever the style was implicitly closed.
    pub span: Span,
}

/// Word-level attributes, separated from text by `|`.
/// Example: `\w gracious|lemma="grace" strong="H1234"\w*`
#[derive(Debug, PartialEq, Clone)]
pub struct Attributes<'a> {
    /// Attribute key-value pairs. Key may be empty for default attribute.
    pub pairs: Vec<Attribute<'a>>,
}

/// A single attribute, either named or default (unnamed).
#[derive(Debug, PartialEq, Clone)]
pub struct Attribute<'a> {
    /// Attribute name (e.g., "lemma", "strong"). Empty string for default attribute.
    pub name: Cow<'a, str>,
    /// Attribute value.
    pub value: Cow<'a, str>,
}

#[derive(Debug, PartialEq)]
pub struct Note<'a> {
    pub style: StyleId,
    pub caller: Caller<'a>,
    /// The `\cat` category, when one directly follows the caller. Its span
    /// is the whole `\cat …\cat*`. A `\cat` anywhere else stays a `Char`.
    pub category: Option<Text<'a>>,
    pub children: Vec<Inline<'a>>,
    /// Source range from the opening marker to the closing marker, or to
    /// wherever the note was implicitly closed.
    pub span: Span,
}

#[derive(Debug, PartialEq)]
pub enum Caller<'a> {
    Plus,
    Minus,
    Custom(Cow<'a, str>),
}

impl std::fmt::Display for Caller<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Caller::Plus => "+",
                Caller::Minus => "-",
                Caller::Custom(s) => s,
            }
        )
    }
}

impl<'a> Caller<'a> {
    pub fn from_str(s: &'a str) -> Caller<'a> {
        match s {
            "+" => Caller::Plus,
            "-" => Caller::Minus,
            _ => Caller::Custom(Cow::Borrowed(s)),
        }
    }
}
