//! Paratext's book canon: the order `Settings.xml`'s `BooksPresent` flags
//! follow, and the number each book's file name carries.
//!
//! Paratext numbers the books from 1 (`GEN`) to 39 (`MAL`), skips 40, and
//! goes on from 41 (`MAT`); from 101 the number is a letter and a digit
//! (`A0`, `A1`, …, `B0`, …), so `INT` is `A7` and its file is `A7INT….SFM`.

use usfm_ast::BookCode;

/// Every book code Paratext knows, in canon order.
pub const BOOKS: [&str; 123] = [
    "GEN", "EXO", "LEV", "NUM", "DEU", "JOS", "JDG", "RUT", "1SA", "2SA", "1KI", "2KI", "1CH",
    "2CH", "EZR", "NEH", "EST", "JOB", "PSA", "PRO", "ECC", "SNG", "ISA", "JER", "LAM", "EZK",
    "DAN", "HOS", "JOL", "AMO", "OBA", "JON", "MIC", "NAM", "HAB", "ZEP", "HAG", "ZEC", "MAL",
    "MAT", "MRK", "LUK", "JHN", "ACT", "ROM", "1CO", "2CO", "GAL", "EPH", "PHP", "COL", "1TH",
    "2TH", "1TI", "2TI", "TIT", "PHM", "HEB", "JAS", "1PE", "2PE", "1JN", "2JN", "3JN", "JUD",
    "REV", "TOB", "JDT", "ESG", "WIS", "SIR", "BAR", "LJE", "S3Y", "SUS", "BEL", "1MA", "2MA",
    "3MA", "4MA", "1ES", "2ES", "MAN", "PS2", "ODA", "PSS", "JSA", "JSB", "TBS", "SST", "DNT",
    "BLT", "XXA", "XXB", "XXC", "XXD", "XXE", "XXF", "XXG", "FRT", "BAK", "OTH", "3ES", "EZA",
    "5EZ", "6EZ", "INT", "CNC", "GLO", "TDX", "NDX", "DAG", "PS3", "2BA", "LBA", "JUB", "ENO",
    "1MQ", "2MQ", "3MQ", "REP", "4BA", "LAO",
];

/// The book's position in [`BOOKS`], from 0.
pub fn index(code: BookCode) -> Option<usize> {
    let code = code.to_string();
    BOOKS.iter().position(|book| *book == code)
}

/// The book at a position in [`BOOKS`].
pub fn book(index: usize) -> Option<BookCode> {
    BOOKS.get(index).and_then(|code| code.parse().ok())
}

/// The number Paratext puts in the book's file name: `01` for `GEN`, `41`
/// for `MAT`, `A7` for `INT`.
pub fn file_number(code: BookCode) -> Option<String> {
    let index = index(code)?;
    let number = if index < 39 { index + 1 } else { index + 2 };
    if number <= 100 {
        return Some(format!("{number:02}"));
    }
    let offset = number - 101;
    let letter = char::from(b'A' + u8::try_from(offset / 10).ok()?);
    Some(format!("{letter}{}", offset % 10))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(s: &str) -> BookCode {
        s.parse().unwrap()
    }

    #[test]
    fn file_numbers_skip_forty_and_turn_to_letters_after_a_hundred() {
        assert_eq!(file_number(code("GEN")).as_deref(), Some("01"));
        assert_eq!(file_number(code("MAL")).as_deref(), Some("39"));
        assert_eq!(file_number(code("MAT")).as_deref(), Some("41"));
        assert_eq!(file_number(code("REV")).as_deref(), Some("67"));
        assert_eq!(file_number(code("FRT")).as_deref(), Some("A0"));
        assert_eq!(file_number(code("INT")).as_deref(), Some("A7"));
        assert_eq!(file_number(code("GLO")).as_deref(), Some("A9"));
        assert_eq!(file_number(code("LAO")).as_deref(), Some("C3"));
    }

    #[test]
    fn a_code_outside_the_canon_has_no_number() {
        assert_eq!(file_number(code("TST")), None);
    }

    /// Every code of the canon is a `BookCode` and round-trips through its
    /// index, but one: `3ES`, which Paratext lists and USX's `book@code`
    /// neither lists nor matches (`[0-9][A-Z]{2}` is not one of its
    /// patterns), so a `\id 3ES` book cannot be a `Document` either.
    #[test]
    fn every_code_but_3es_is_a_book_code() {
        let unreadable: Vec<&str> = BOOKS
            .iter()
            .enumerate()
            .filter(|(index, name)| {
                let read = book(*index);
                if let Some(read) = read {
                    assert_eq!(read.to_string(), **name);
                    assert_eq!(super::index(read), Some(*index));
                }
                read.is_none()
            })
            .map(|(_, name)| *name)
            .collect();
        assert_eq!(unreadable, ["3ES"]);
    }
}
