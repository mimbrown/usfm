use bitflags::bitflags;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Error, Read};
use std::mem::take;
use std::path::Path;
use std::str::FromStr;

#[derive(Debug, PartialEq)]
pub enum StyleParseError {
    StyleTypeRequired,
    UnknownStyleType(String),
    UnknownTextType,
    UnknownTextProperty,
}

#[derive(Debug, PartialEq, Clone)]
pub enum StyleType {
    Paragraph,
    Character,
    Note,
    Milestone,
}

impl FromStr for StyleType {
    type Err = StyleParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "paragraph" => StyleType::Paragraph,
            "character" => StyleType::Character,
            "note" => StyleType::Note,
            "milestone" => StyleType::Milestone,
            _ => return Err(StyleParseError::UnknownStyleType(s.to_string())),
        })
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum TextType {
    ChapterNumber,
    VerseNumber,
    VerseText,
    Title,
    Section,
    NoteText,
    Other,
}

impl FromStr for TextType {
    type Err = StyleParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "chapternumber" => TextType::ChapterNumber,
            "versenumber" => TextType::VerseNumber,
            "versetext" => TextType::VerseText,
            "title" => TextType::Title,
            "section" => TextType::Section,
            "notetext" => TextType::NoteText,
            "other" => TextType::Other,
            _ => return Err(StyleParseError::UnknownTextType),
        })
    }
}

bitflags! {
    #[derive(Debug, Default, Clone)]
    pub struct TextProperties: u16 {
        const LEVEL_1 =         0b000000000000001;
        const LEVEL_2 =         0b000000000000010;
        const LEVEL_3 =         0b000000000000011;
        const LEVEL_4 =         0b000000000000100;
        const LEVEL_5 =         0b000000000000101;
        const PARAGRAPH =       0b000000000001000;
        const PUBLISHABLE =     0b000000000010000;
        const CHAPTER =         0b000000000100000;
        const VERSE =           0b000000001000000;
        const VERNACULAR =      0b000000010000000;
        const BOOK =            0b000000100000000;
        const POETIC =          0b000001000000000;
        const NOTE =            0b000010000000000;
        const CROSS_REFERENCE = 0b000100000000000;
    }
}

impl TextProperties {
    pub fn level(&self) -> u16 {
        self.bits() & 0b000000000000111
    }
}

impl FromStr for TextProperties {
    type Err = StyleParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut properties = TextProperties::default();
        for property in s.split(|c: char| c.is_ascii_whitespace()) {
            match property.to_lowercase().as_str() {
                "paragraph" => properties.insert(TextProperties::PARAGRAPH),
                "publishable" => properties.insert(TextProperties::PUBLISHABLE),
                "chapter" => properties.insert(TextProperties::CHAPTER),
                "verse" => properties.insert(TextProperties::VERSE),
                "vernacular" => properties.insert(TextProperties::VERNACULAR),
                "level_1" => properties.insert(TextProperties::LEVEL_1),
                "level_2" => properties.insert(TextProperties::LEVEL_2),
                "level_3" => properties.insert(TextProperties::LEVEL_3),
                "level_4" => properties.insert(TextProperties::LEVEL_4),
                "level_5" => properties.insert(TextProperties::LEVEL_5),
                "book" => properties.insert(TextProperties::BOOK),
                "poetic" => properties.insert(TextProperties::POETIC),
                "note" => properties.insert(TextProperties::NOTE),
                "crossreference" => properties.insert(TextProperties::CROSS_REFERENCE),
                "nonpublishable" => properties.remove(TextProperties::PUBLISHABLE),
                "nonvernacular" => properties.remove(TextProperties::VERNACULAR),
                _ => return Err(StyleParseError::UnknownTextProperty),
            }
        }
        Ok(properties)
    }
}

pub trait Marker {
    fn style_type(&self) -> StyleType;

    fn text_type(&self) -> TextType;

    fn is_paragraph(&self) -> bool {
        self.style_type() == StyleType::Paragraph
    }

    fn is_character(&self) -> bool {
        self.style_type() == StyleType::Character
    }

    fn is_note(&self) -> bool {
        self.style_type() == StyleType::Note
    }

    fn is_publishable(&self) -> bool;

    fn is_nonpublishable(&self) -> bool;
}

#[derive(Debug, Default)]
pub struct StyleRuleBuilder {
    name: Option<String>,
    description: Option<String>,
    style_type: Option<StyleType>,
    text_type: Option<TextType>,
    text_properties: Option<TextProperties>,
    nest: bool,
    occurs_under: Vec<String>,
}

impl StyleRuleBuilder {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone)]
pub struct StyleRule {
    pub marker: String,
    /// `\Name`: the marker's title, as Paratext's sheet writes it
    /// (`"p - Paragraph - Normal - First Line Indent"`). `None` for a rule
    /// from a sheet that does not say, and for one the parser derived
    /// (`\k-s` from `\k`, an unknown milestone). Nothing in the toolchain
    /// decides anything by it: it is documentation, which is what the
    /// language server shows on hover (ticket 31).
    pub name: Option<String>,
    /// `\Description`: one sentence on what the marker is for.
    pub description: Option<String>,
    pub style_type: StyleType,
    pub text_type: TextType,
    pub text_properties: TextProperties,
    /// `NEST` in `\OccursUnder`: the style may nest inside a character style.
    pub nest: bool,
    /// The markers this one may occur under (`\OccursUnder`, minus `NEST`).
    /// Empty means unrestricted.
    pub occurs_under: Vec<String>,
}

impl StyleRule {
    fn from_builder(
        marker: String,
        builder: &mut StyleRuleBuilder,
    ) -> Result<Self, StyleParseError> {
        Ok(Self {
            marker,
            name: builder.name.take(),
            description: builder.description.take(),
            style_type: builder
                .style_type
                .take()
                .ok_or(StyleParseError::StyleTypeRequired)?,
            text_type: builder.text_type.take().unwrap_or(TextType::Other),
            text_properties: builder.text_properties.take().unwrap_or_default(),
            nest: builder.nest,
            occurs_under: std::mem::take(&mut builder.occurs_under),
        })
    }

    fn apply_builder(&mut self, builder: &mut StyleRuleBuilder) {
        if let Some(name) = builder.name.take() {
            self.name = Some(name);
        }
        if let Some(description) = builder.description.take() {
            self.description = Some(description);
        }
        if let Some(style_type) = builder.style_type.take() {
            self.style_type = style_type;
        }
        if let Some(text_type) = builder.text_type.take() {
            self.text_type = text_type;
        }
        if let Some(text_properties) = builder.text_properties.take() {
            self.text_properties = text_properties;
        }
        if !builder.occurs_under.is_empty() {
            self.occurs_under = std::mem::take(&mut builder.occurs_under);
            self.nest = builder.nest;
        }
    }

    pub fn is_paragraph(&self) -> bool {
        self.style_type == StyleType::Paragraph
    }

    pub fn is_character(&self) -> bool {
        self.style_type == StyleType::Character
    }

    pub fn is_note(&self) -> bool {
        self.style_type == StyleType::Note
    }

    pub fn is_publishable(&self) -> bool {
        self.text_properties.contains(TextProperties::PUBLISHABLE)
    }

    pub fn is_nonpublishable(&self) -> bool {
        !self.is_publishable()
    }

    pub fn is_vernacular(&self) -> bool {
        self.text_properties.contains(TextProperties::VERNACULAR)
    }

    pub fn is_nonvernacular(&self) -> bool {
        !self.is_vernacular()
    }

    pub fn is_chapter(&self) -> bool {
        self.text_properties.contains(TextProperties::CHAPTER)
    }

    pub fn is_verse(&self) -> bool {
        self.text_properties.contains(TextProperties::VERSE)
    }

    pub fn is_verse_text(&self) -> bool {
        self.text_type == TextType::VerseText
    }

    pub fn is_milestone(&self) -> bool {
        self.style_type == StyleType::Milestone
    }
}

pub struct StyleSheetBuilder {
    rules: Vec<StyleRule>,
    rule_by_marker_map: HashMap<String, usize>,
    current_marker: String,
    current_marker_builder: StyleRuleBuilder,
}

impl Default for StyleSheetBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl StyleSheetBuilder {
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            rule_by_marker_map: HashMap::new(),
            current_marker: String::default(),
            current_marker_builder: StyleRuleBuilder::default(),
        }
    }

    pub fn cut_rule(&mut self) -> Result<(), StyleParseError> {
        if self.current_marker.is_empty() {
            return Ok(());
        }
        let marker = take(&mut self.current_marker);

        if let Some(existing_rule) = self.rule_by_marker_map.get(&marker) {
            self.rules[*existing_rule].apply_builder(&mut self.current_marker_builder);
        } else {
            let key = marker.clone();
            let rule = StyleRule::from_builder(marker, &mut self.current_marker_builder)?;
            if rule.is_milestone() && key.ends_with("-s") {
                let mut end_rule = rule.clone();
                let end_rule_marker = format!("{}e", &key[0..key.len() - 1]);
                let end_rule_key = end_rule_marker.clone();
                end_rule.marker = end_rule_marker;
                self.rules.push(end_rule);
                self.rule_by_marker_map
                    .insert(end_rule_key, self.rules.len() - 1);
            }
            self.rules.push(rule);
            self.rule_by_marker_map.insert(key, self.rules.len() - 1);
        }
        Ok(())
    }
}

impl FromStr for StyleSheetBuilder {
    type Err = StyleParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut builder = StyleSheetBuilder::new();
        builder.read(s)?;
        Ok(builder)
    }
}

impl StyleSheetBuilder {
    /// Read a `.sty` text into this builder: a `\Marker` already here amends
    /// that rule, a new one adds a rule (and needs a `\StyleType`).
    fn read(&mut self, s: &str) -> Result<(), StyleParseError> {
        let builder = self;
        // A byte-order mark (Paratext and Windows editors write one) would
        // hide the first line's `\`, and with it the first `\Marker`: every
        // field after it would then belong to no rule, silently.
        let s = s.strip_prefix('\u{feff}').unwrap_or(s);
        for line in s.lines() {
            let line = line.trim_start_matches("#!").trim();
            if line.starts_with('\\') {
                let len = line.len();
                let space = line.find(char::is_whitespace).unwrap_or(len);
                let key = &line[1..space];
                let value = if space < len {
                    &line[space + 1..]
                } else {
                    &line[0..0]
                };
                // println!("key: {}, value: {}", key, value);
                match key {
                    "Marker" => {
                        builder.cut_rule()?;
                        builder.current_marker = value.trim().to_string();
                    }
                    // Documentation, kept so that a tool can show it: the
                    // language server's hover is the stylesheet's own words
                    // about the marker under the cursor (ticket 31). An empty
                    // value is no value, the way a missing line is.
                    "Name" => {
                        let name = value.trim();
                        if !name.is_empty() {
                            builder.current_marker_builder.name = Some(name.to_string());
                        }
                    }
                    "Description" => {
                        let description = value.trim();
                        if !description.is_empty() {
                            builder.current_marker_builder.description =
                                Some(description.to_string());
                        }
                    }
                    "StyleType" => {
                        builder.current_marker_builder.style_type =
                            Some(StyleType::from_str(value)?);
                    }
                    "TextType" => {
                        builder.current_marker_builder.text_type = Some(TextType::from_str(value)?);
                    }
                    "TextProperties" => {
                        builder.current_marker_builder.text_properties =
                            Some(TextProperties::from_str(value)?);
                    }
                    "OccursUnder" => {
                        let names: Vec<&str> = value.split_whitespace().collect();
                        builder.current_marker_builder.nest = names.contains(&"NEST");
                        builder.current_marker_builder.occurs_under = names
                            .into_iter()
                            .filter(|name| *name != "NEST")
                            .map(String::from)
                            .collect();
                    }
                    _ => {}
                }
            }
        }
        builder.cut_rule()
    }
}

#[derive(Clone)]
pub struct StyleSheet {
    pub rules: Vec<StyleRule>,
    rule_by_marker_map: HashMap<String, usize>,
}

impl StyleSheet {
    pub fn new(rules: Vec<StyleRule>) -> Self {
        let mut rule_by_marker_map = HashMap::new();
        for (index, rule) in rules.iter().enumerate() {
            rule_by_marker_map.insert(rule.marker.clone(), index);
        }
        Self {
            rules,
            rule_by_marker_map,
        }
    }

    pub fn from_file<P>(path: P) -> Result<Self, Error>
    where
        P: AsRef<Path>,
    {
        let file = File::open(path)?;
        let mut lines = String::new();
        BufReader::new(file).read_to_string(&mut lines)?;

        StyleSheet::from_str(&lines)
            .map_err(|e| Error::other(format!("failed to parse usfm.sty: {:?}", e)))
    }

    pub fn from_builder(builder: StyleSheetBuilder) -> Self {
        Self {
            rules: builder.rules,
            rule_by_marker_map: builder.rule_by_marker_map,
        }
    }

    pub fn get_marker_index(&self, marker: &str) -> Option<&usize> {
        self.rule_by_marker_map.get(marker)
    }

    pub fn get_rule_by_marker(&self, marker: &str) -> Option<&StyleRule> {
        self.rule_by_marker_map
            .get(marker)
            .map(|index| &self.rules[*index])
    }

    pub fn get_rule(&self, marker: usize) -> &StyleRule {
        &self.rules[marker]
    }

    /// Read a project's `.sty` (Paratext's `custom.sty`) over this sheet, the
    /// way Paratext does: an entry for a marker the sheet has **amends** that
    /// rule — `\Marker p` with only a `\FirstLineIndent` changes nothing we
    /// read and keeps `p` a paragraph — and an entry for a new marker adds a
    /// rule, which then needs its `\StyleType`. Existing rules keep their
    /// index, so a `StyleId` into the sheet stays valid.
    pub fn extend_from_str(&mut self, s: &str) -> Result<(), StyleParseError> {
        let mut builder = StyleSheetBuilder {
            rules: self.rules.clone(),
            rule_by_marker_map: self.rule_by_marker_map.clone(),
            ..StyleSheetBuilder::new()
        };
        builder.read(s)?;
        *self = StyleSheet::from_builder(builder);
        Ok(())
    }

    pub fn add_rule(&mut self, rule: StyleRule) -> usize {
        let insert = self.rules.len();
        self.rule_by_marker_map.insert(rule.marker.clone(), insert);
        self.rules.push(rule);
        insert
    }
}

impl FromStr for StyleSheet {
    type Err = StyleParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let builder = StyleSheetBuilder::from_str(s)?;
        Ok(StyleSheet::from_builder(builder))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `\Name` and `\Description` are kept as the sheet writes them, and a
    /// repeated `\Marker` amends them like every other field (which is how
    /// `usfm-extra.sty` corrects an entry of Paratext's sheet).
    #[test]
    fn a_rule_keeps_its_name_and_description() {
        let sheet = StyleSheet::from_str(
            "\\Marker p\n\
             \\Name p - Paragraph - Normal - First Line Indent\n\
             \\Description Paragraph text, with first line indent\n\
             \\StyleType Paragraph\n\
             \\OccursUnder id c\n\
             \n\
             \\Marker zx\n\
             \\StyleType Character\n\
             \n\
             \\Marker p\n\
             \\Description Amended\n",
        )
        .expect("a well-formed sheet");

        let p = sheet.get_rule_by_marker("p").expect("the `p` rule");
        assert_eq!(
            p.name.as_deref(),
            Some("p - Paragraph - Normal - First Line Indent")
        );
        assert_eq!(p.description.as_deref(), Some("Amended"));
        assert_eq!(p.occurs_under, ["id", "c"]);

        // A sheet that says neither leaves both unset rather than empty.
        let zx = sheet.get_rule_by_marker("zx").expect("the `zx` rule");
        assert_eq!(zx.name, None);
        assert_eq!(zx.description, None);
    }

    /// A `\Name` with nothing after it is no name: the hover has nothing to
    /// show either way, and `Some("")` would print an empty line.
    #[test]
    fn an_empty_name_is_no_name() {
        let sheet = StyleSheet::from_str("\\Marker p\n\\Name\n\\StyleType Paragraph\n")
            .expect("a well-formed sheet");
        assert_eq!(sheet.get_rule_by_marker("p").unwrap().name, None);
    }

    /// A project sheet over a base sheet: an entry for a marker the base has
    /// amends that rule in place (no `\\StyleType` needed, nothing it does
    /// not mention changed, same index), and a new marker adds a rule.
    #[test]
    fn extending_amends_known_markers_and_adds_new_ones() {
        let mut sheet = StyleSheet::from_str(
            "\\Marker p\n\\Name Paragraph\n\\StyleType Paragraph\n\\OccursUnder c\n\
             \n\\Marker nd\n\\StyleType Character\n",
        )
        .expect("a well-formed sheet");
        let p_index = *sheet.get_marker_index("p").unwrap();

        sheet
            .extend_from_str(
                "\\Marker p\n\\FirstLineIndent 0.5\n\\OccursUnder c id\n\
                 \n\\Marker zgrk\n\\StyleType character\n",
            )
            .expect("a well-formed project sheet");

        assert_eq!(*sheet.get_marker_index("p").unwrap(), p_index);
        let p = sheet.get_rule_by_marker("p").unwrap();
        assert!(p.is_paragraph());
        assert_eq!(p.name.as_deref(), Some("Paragraph"));
        assert_eq!(p.occurs_under, ["c", "id"]);
        assert!(sheet.get_rule_by_marker("nd").unwrap().is_character());
        assert!(sheet.get_rule_by_marker("zgrk").unwrap().is_character());

        // A new marker still needs to say what it is.
        assert!(matches!(
            sheet.extend_from_str("\\Marker zz\n\\Bold\n"),
            Err(StyleParseError::StyleTypeRequired)
        ));
    }

    /// A sheet saved with a UTF-8 byte-order mark keeps its first rule.
    #[test]
    fn a_byte_order_mark_does_not_hide_the_first_marker() {
        let sheet = StyleSheet::from_str(
            "\u{feff}\\Marker zgrk\n\\Endmarker zgrk*\n\\StyleType character\n",
        )
        .expect("a well-formed sheet");
        assert!(sheet.get_rule_by_marker("zgrk").unwrap().is_character());
    }
}
