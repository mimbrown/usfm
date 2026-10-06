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
    ///
    /// A setting the file does give also keeps its **standard spelling**
    /// (`:` between chapter and verse, `-`, `,`, `;`) unless the project
    /// declares that spelling for a *different* setting. A real project sets
    /// its chapter–verse separator to `.` and writes `:` in every reference;
    /// nothing else in its settings claims `:`, so `3:16` there has one
    /// reading. A project that writes `3,16` and declares `,` for it does not
    /// get `,` back as a sequence mark. This is silent today; a settings
    /// mismatch like that is worth a warning once there is somewhere to
    /// report one (the spec's "Work with no plan yet").
    pub fn citation_format(&self) -> CitationFormat {
        const KEYS: [&str; 6] = [
            "ChapterVerseSeparator",
            "RangeIndicator",
            "ChapterRangeSeparator",
            "SequenceIndicator",
            "ChapterNumberSeparator",
            "BookSequenceSeparator",
        ];
        let default = CitationFormat::default();
        let standard = [
            default.chapter_verse,
            default.range,
            default.chapter_range,
            default.sequence,
            default.chapter_number,
            default.book_sequence,
        ];
        let declared = KEYS.map(|key| {
            let spellings: Vec<String> = self
                .get(key)
                .unwrap_or("")
                .split('|')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            (!spellings.is_empty()).then_some(spellings)
        });
        let mut roles = declared
            .iter()
            .zip(standard)
            .enumerate()
            .map(|(role, (own, standard))| {
                let Some(own) = own else {
                    return standard;
                };
                let mut spellings = own.clone();
                for spelling in standard {
                    let claimed = declared.iter().enumerate().any(|(other, list)| {
                        other != role && list.iter().flatten().any(|s| *s == spelling)
                    });
                    if !claimed && !spellings.contains(&spelling) {
                        spellings.push(spelling);
                    }
                }
                spellings
            });
        let mut next = || roles.next().unwrap_or_default();
        CitationFormat {
            chapter_verse: next(),
            range: next(),
            chapter_range: next(),
            sequence: next(),
            chapter_number: next(),
            book_sequence: next(),
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
        // `,` is nobody else's, so it is still a sequence mark.
        assert_eq!(format.sequence, ["،", ","]);
        assert_eq!(format.chapter_number, ["؛", ";"]);
        assert_eq!(format.chapter_verse, [":"]);
    }

    /// A declared setting keeps its standard spelling unless another setting
    /// claims it: `.` declared for chapter and verse still reads `3:16`, and
    /// a project that writes `3,16; 4,1.5` gets neither `,` nor `:` wrong.
    #[test]
    fn a_standard_spelling_is_kept_unless_another_setting_claims_it() {
        let format = Settings::from_xml(
            "<ScriptureText><ChapterVerseSeparator>.</ChapterVerseSeparator></ScriptureText>",
        )
        .unwrap()
        .citation_format();
        assert_eq!(format.chapter_verse, [".", ":"]);
        assert_eq!(
            format,
            CitationFormat {
                chapter_verse: vec![".".into(), ":".into()],
                ..CitationFormat::default()
            }
        );

        let format = Settings::from_xml(
            "<ScriptureText><ChapterVerseSeparator>,</ChapterVerseSeparator>\
             <SequenceIndicator>.</SequenceIndicator>\
             <RangeIndicator>-</RangeIndicator>\
             <ChapterRangeSeparator>—</ChapterRangeSeparator></ScriptureText>",
        )
        .unwrap()
        .citation_format();
        assert_eq!(format.chapter_verse, [",", ":"]);
        assert_eq!(format.sequence, ["."]);
        // `-` is the project's verse range, so it is not its chapter range.
        assert_eq!(format.chapter_range, ["—"]);
    }

    #[test]
    fn not_xml_is_an_error() {
        assert!(Settings::from_xml("<ScriptureText>").is_err());
    }
}
