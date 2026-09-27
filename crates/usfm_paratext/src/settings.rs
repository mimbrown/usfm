//! `Settings.xml`: a `<ScriptureText>` root with one element per setting.

use std::collections::BTreeMap;

use usfm_ast::BookCode;
use usfm_semantic::citation::CitationFormat;

use crate::canon;

/// A project's `Settings.xml`. Every setting is kept as text under its
/// element name ([`Settings::get`]); the ones this crate acts on have typed
/// accessors.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Settings {
    values: BTreeMap<String, String>,
    naming: Naming,
}

impl Settings {
    /// Read the text of a `Settings.xml`.
    pub fn from_xml(xml: &str) -> Result<Self, String> {
        let document = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
        let mut values = BTreeMap::new();
        let mut naming_element = None;
        for element in document
            .root_element()
            .children()
            .filter(|n| n.is_element())
        {
            let name = element.tag_name().name();
            if name == "Naming" {
                naming_element = Some(element);
                continue;
            }
            values.insert(name.to_string(), element.text().unwrap_or("").to_string());
        }

        // Paratext 7 wrote the naming rule as three elements; later versions
        // write one `<Naming>` element with three attributes, which wins when
        // both are there.
        let attribute = |name: &str, fallback: &str| {
            naming_element
                .and_then(|e| e.attribute(name))
                .or_else(|| values.get(fallback).map(String::as_str))
                .unwrap_or("")
                .to_string()
        };
        let naming = Naming {
            pre_part: attribute("PrePart", "FileNamePrePart"),
            post_part: attribute("PostPart", "FileNamePostPart"),
            form: BookNameForm::from_setting(&attribute("BookNameForm", "FileNameBookNameForm")),
        };
        Ok(Self { values, naming })
    }

    /// The text of the setting named `key` (`FullName`, `Versification`,
    /// `ChapterVerseSeparator`, …), as written.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    /// Every setting but `<Naming>`, by element name.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.values.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// How the project's book files are named.
    pub fn naming(&self) -> &Naming {
        &self.naming
    }

    /// The project's short name (`Name`).
    pub fn name(&self) -> Option<&str> {
        self.get("Name")
    }

    /// The project's full name (`FullName`).
    pub fn full_name(&self) -> Option<&str> {
        self.get("FullName")
    }

    /// The stylesheet file the project names (`StyleSheet`), normally
    /// `usfm.sty`.
    pub fn style_sheet(&self) -> Option<&str> {
        self.get("StyleSheet").filter(|s| !s.is_empty())
    }

    /// The books' encoding, as the Windows code page number Paratext writes
    /// (`65001` is UTF-8).
    pub fn encoding(&self) -> Option<&str> {
        self.get("Encoding")
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }

    /// The punctuation the project writes references with, for
    /// `usfm_semantic::citation`: Paratext's reference settings, each split on
    /// `|` where a project lists several spellings, and the default for a
    /// setting the file leaves out.
    pub fn citation_format(&self) -> CitationFormat {
        let default = CitationFormat::default();
        let read = |key: &str, fallback: Vec<String>| match self.get(key) {
            Some(value) if !value.trim().is_empty() => value
                .split('|')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            _ => fallback,
        };
        CitationFormat {
            chapter_verse: read("ChapterVerseSeparator", default.chapter_verse),
            range: read("RangeIndicator", default.range),
            chapter_range: read("ChapterRangeSeparator", default.chapter_range),
            sequence: read("SequenceIndicator", default.sequence),
            chapter_number: read("ChapterNumberSeparator", default.chapter_number),
            book_sequence: read("BookSequenceSeparator", default.book_sequence),
        }
    }

    /// The books `BooksPresent` marks with a `1`, in canon order.
    pub fn books_present(&self) -> Vec<BookCode> {
        self.get("BooksPresent")
            .unwrap_or("")
            .bytes()
            .enumerate()
            .filter(|(_, flag)| *flag == b'1')
            .filter_map(|(index, _)| canon::book(index))
            .collect()
    }
}

/// How a project names its book files: `PrePart`, then the book in
/// `BookNameForm`, then `PostPart` (`41MATSSV.SFM`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Naming {
    pub pre_part: String,
    pub post_part: String,
    pub form: BookNameForm,
}

impl Naming {
    /// The file name of the book, if the form is known and the book is in
    /// Paratext's canon (the `MAT` form names any book).
    pub fn file_name(&self, code: BookCode) -> Option<String> {
        let stem = match &self.form {
            BookNameForm::NumberCode => format!("{}{code}", canon::file_number(code)?),
            BookNameForm::Code => code.to_string(),
            BookNameForm::Number => canon::file_number(code)?,
            BookNameForm::Other(_) => return None,
        };
        Some(format!("{}{stem}{}", self.pre_part, self.post_part))
    }
}

/// The middle of a book's file name.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum BookNameForm {
    /// `41MAT`, Paratext's default.
    #[default]
    NumberCode,
    /// `MAT`.
    Code,
    /// `41`.
    Number,
    /// A form this crate does not know, as written.
    Other(String),
}

impl BookNameForm {
    fn from_setting(value: &str) -> Self {
        match value.trim() {
            "" | "41MAT" => BookNameForm::NumberCode,
            "MAT" => BookNameForm::Code,
            "41" => BookNameForm::Number,
            other => BookNameForm::Other(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(s: &str) -> BookCode {
        s.parse().unwrap()
    }

    #[test]
    fn the_naming_element_gives_the_file_name() {
        let settings = Settings::from_xml(
            r#"<ScriptureText><Name>SSV</Name><Naming PrePart="" PostPart="SSV.SFM" BookNameForm="41MAT" /></ScriptureText>"#,
        )
        .unwrap();
        assert_eq!(settings.name(), Some("SSV"));
        assert_eq!(
            settings.naming().file_name(code("MAT")).as_deref(),
            Some("41MATSSV.SFM")
        );
        assert_eq!(
            settings.naming().file_name(code("INT")).as_deref(),
            Some("A7INTSSV.SFM")
        );
    }

    #[test]
    fn paratext_7_naming_elements_are_read_too() {
        let settings = Settings::from_xml(
            "<ScriptureText><FileNamePrePart>x-</FileNamePrePart>\
             <FileNamePostPart>.usfm</FileNamePostPart>\
             <FileNameBookNameForm>MAT</FileNameBookNameForm></ScriptureText>",
        )
        .unwrap();
        assert_eq!(
            settings.naming().file_name(code("GEN")).as_deref(),
            Some("x-GEN.usfm")
        );
    }

    #[test]
    fn the_number_form_and_an_unknown_form() {
        let number = Settings::from_xml(
            r#"<ScriptureText><Naming PostPart=".SFM" BookNameForm="41" /></ScriptureText>"#,
        )
        .unwrap();
        assert_eq!(
            number.naming().file_name(code("MRK")).as_deref(),
            Some("42.SFM")
        );
        // The code form names a book outside the canon; the number forms
        // cannot.
        assert_eq!(number.naming().file_name(code("TST")), None);

        let unknown =
            Settings::from_xml(r#"<ScriptureText><Naming BookNameForm="MAT41" /></ScriptureText>"#)
                .unwrap();
        assert_eq!(
            unknown.naming().form,
            BookNameForm::Other("MAT41".to_string())
        );
        assert_eq!(unknown.naming().file_name(code("MAT")), None);
    }

    #[test]
    fn books_present_follows_the_canon() {
        let mut flags = "0".repeat(123).into_bytes();
        flags[0] = b'1'; // GEN
        flags[39] = b'1'; // MAT
        flags[106] = b'1'; // INT
        let xml = format!(
            "<ScriptureText><BooksPresent>{}</BooksPresent></ScriptureText>",
            String::from_utf8(flags).unwrap()
        );
        let books: Vec<String> = Settings::from_xml(&xml)
            .unwrap()
            .books_present()
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(books, ["GEN", "MAT", "INT"]);
    }

    #[test]
    fn the_citation_format_reads_the_settings_and_defaults_the_rest() {
        let settings = Settings::from_xml(
            "<ScriptureText><SequenceIndicator>،</SequenceIndicator>\
             <ChapterNumberSeparator>؛|;</ChapterNumberSeparator></ScriptureText>",
        )
        .unwrap();
        let format = settings.citation_format();
        assert_eq!(format.sequence, ["،"]);
        assert_eq!(format.chapter_number, ["؛", ";"]);
        assert_eq!(format.chapter_verse, [":"]);
    }

    #[test]
    fn not_xml_is_an_error() {
        assert!(Settings::from_xml("<ScriptureText>").is_err());
    }
}
