//! `BookNames.xml`: `<BookNames><book code="MAT" abbr="Mt" short="Matthew"
//! long="The Gospel of Matthew" />…</BookNames>`.

use usfm_ast::BookCode;
use usfm_semantic::citation::BookNameTable;

/// The names a project gives one book. An empty attribute is `None`:
/// Paratext writes every attribute, filled in or not.
#[derive(Debug, Clone, PartialEq)]
pub struct BookName {
    pub code: BookCode,
    pub abbreviation: Option<String>,
    pub short: Option<String>,
    pub long: Option<String>,
}

/// Every book `BookNames.xml` lists, in its order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BookNames {
    names: Vec<BookName>,
}

impl BookNames {
    /// Read the text of a `BookNames.xml`. A `<book>` whose `code` is not a
    /// book code is skipped.
    pub fn from_xml(xml: &str) -> Result<Self, String> {
        let document = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
        let names = document
            .root_element()
            .children()
            .filter(|n| n.has_tag_name("book"))
            .filter_map(|book| {
                let code = book.attribute("code")?.trim().parse().ok()?;
                let name = |attribute: &str| {
                    book.attribute(attribute)
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                };
                Some(BookName {
                    code,
                    abbreviation: name("abbr"),
                    short: name("short"),
                    long: name("long"),
                })
            })
            .collect();
        Ok(Self { names })
    }

    /// The names of one book.
    pub fn get(&self, code: BookCode) -> Option<&BookName> {
        self.names.iter().find(|name| name.code == code)
    }

    pub fn iter(&self) -> impl Iterator<Item = &BookName> {
        self.names.iter()
    }

    /// The names to recognise in a reference: every book's abbreviation,
    /// short and long name, and the three-letter codes.
    ///
    /// A `~` in a name is USFM's no-break space, as it is in the text: a
    /// project that writes `\xt خلیفۃُ~اللہ 5:3` names the book `خلیفۃُ~اللہ`
    /// here, and the parser has made the text's `~` a U+00A0 by the time a
    /// reference is read from it. The fields themselves keep the `~`.
    pub fn table(&self) -> BookNameTable {
        let mut table = BookNameTable::new().with_codes();
        for name in &self.names {
            for text in [&name.abbreviation, &name.short, &name.long]
                .into_iter()
                .flatten()
            {
                table.add(&text.replace('~', "\u{00A0}"), name.code);
            }
        }
        table
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_read_and_empty_ones_are_none() {
        let names = BookNames::from_xml(
            r#"<?xml version="1.0" encoding="utf-8"?>
<BookNames>
  <book code="MAT" abbr="متی" short="متی" long="متی کی انجیل" />
  <book code="MRK" abbr="" short="مرقس" long="" />
  <book code="!!!" abbr="x" />
</BookNames>"#,
        )
        .unwrap();
        let mat = names.get("MAT".parse().unwrap()).unwrap();
        assert_eq!(mat.abbreviation.as_deref(), Some("متی"));
        assert_eq!(mat.long.as_deref(), Some("متی کی انجیل"));
        let mrk = names.get("MRK".parse().unwrap()).unwrap();
        assert_eq!(mrk.abbreviation, None);
        assert_eq!(mrk.short.as_deref(), Some("مرقس"));
        assert_eq!(names.iter().count(), 2);
    }

    /// Found in a real project (2026-10-06): its four Gospels are named with
    /// a `~`, so none of their names matched the text, and every reference
    /// to one of them was read as a reference to the book it stood in.
    #[test]
    fn a_tilde_in_a_name_is_the_texts_no_break_space() {
        use usfm_semantic::citation::{CitationFormat, Piece, parse_citations};
        let names = BookNames::from_xml(
            r#"<BookNames><book code="MAT" abbr="@خلیفۃُ~اللہ" short="" long="" /></BookNames>"#,
        )
        .unwrap();
        let mat = names.get("MAT".parse().unwrap()).unwrap();
        assert_eq!(mat.abbreviation.as_deref(), Some("@خلیفۃُ~اللہ"));

        let text = "@خلیفۃُ\u{00A0}اللہ 5\u{200F}:3";
        let pieces = parse_citations(
            text,
            &CitationFormat::default(),
            &names.table(),
            Some("JHN".parse().unwrap()),
        );
        let [Piece::Citation(citation)] = pieces.as_slice() else {
            panic!("{pieces:?}");
        };
        assert_eq!(citation.book.as_str(), "MAT");
        assert_eq!((citation.start.chapter, citation.start.verse), (5, Some(3)));
    }
}
