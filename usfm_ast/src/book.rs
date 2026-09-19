use std::{borrow::Cow, fmt, str::FromStr};

use super::Span;

#[derive(Debug, Clone, PartialEq)]
pub struct Book<'a> {
    pub code: BookCode,
    pub description: Cow<'a, str>,
    /// Source range of the whole `\id` line.
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BookCategory {
    OldTestament,
    NewTestament,
    Deuterocanon,
    NonScripture,
    /// A code that is well formed but not one of the books this type lists,
    /// so which of the four it belongs to is not knowable. See
    /// [`BookCode::Other`].
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BookCode {
    // Old Testament
    Gen,
    Exo,
    Lev,
    Num,
    Deu,
    Jos,
    Jdg,
    Rut,
    Sa1,
    Sa2,
    Ki1,
    Ki2,
    Ch1,
    Ch2,
    Ezr,
    Neh,
    Est,
    Job,
    Psa,
    Pro,
    Ecc,
    Sng,
    Isa,
    Jer,
    Lam,
    Ezk,
    Dan,
    Hos,
    Jol,
    Amo,
    Oba,
    Jon,
    Mic,
    Nam,
    Hab,
    Zep,
    Hag,
    Zec,
    Mal,
    // New Testament
    Mat,
    Mrk,
    Luk,
    Jhn,
    Act,
    Rom,
    Co1,
    Co2,
    Gal,
    Eph,
    Php,
    Col,
    Th1,
    Th2,
    Ti1,
    Ti2,
    Tit,
    Phm,
    Heb,
    Jas,
    Pe1,
    Pe2,
    Jn1,
    Jn2,
    Jn3,
    Jud,
    Rev,
    // Deuterocanon
    Tob,
    Jdt,
    Esg,
    Wis,
    Sir,
    Bar,
    Lje,
    S3y,
    Sus,
    Bel,
    Ma1,
    Ma2,
    Ma3,
    Ma4,
    Es1,
    Es2,
    Man,
    Ps2,
    Oda,
    Pss,
    Eza,
    Ez5,
    Ez6,
    Dag,
    Ps3,
    Ba2,
    Lba,
    Jub,
    Eno,
    Mq1,
    Mq2,
    Mq3,
    Rep,
    Ba4,
    Lao,
    // Non scripture
    Xxa,
    Xxb,
    Xxc,
    Xxd,
    Xxe,
    Xxf,
    Xxg,
    Frt,
    Bak,
    Oth,
    Int,
    Cnc,
    Glo,
    Tdx,
    Ndx,
    /// A code USX accepts that is not one of the books above. `book@code` in
    /// `tcdocs/grammar/usx.rnc` is the list of known books *or* the pattern
    /// `[A-Z][A-Z0-9]{2}|[0-9][A-Z][0-9]|[0-9]{2}[A-Z]`, so a project may use
    /// its own three-character code (`TST`, `ZZZ`) and the document is still
    /// valid. The three ASCII bytes are kept verbatim so the code round-trips;
    /// [`BookCode::from_str`] only ever builds one that matches that pattern
    /// and is not a listed variant.
    Other([u8; 3]),
}

use BookCode::*;

/// Whether `code` matches the catch-all `book@code` pattern in `usx.rnc`,
/// `[A-Z][A-Z0-9]{2}|[0-9][A-Z][0-9]|[0-9]{2}[A-Z]`.
pub fn is_well_formed_book_code(code: &str) -> bool {
    let bytes = code.as_bytes();
    let [a, b, c] = bytes else {
        return false;
    };
    let upper = |byte: &u8| byte.is_ascii_uppercase();
    let digit = |byte: &u8| byte.is_ascii_digit();
    (upper(a) && (upper(b) || digit(b)) && (upper(c) || digit(c)))
        || (digit(a) && upper(b) && digit(c))
        || (digit(a) && digit(b) && upper(c))
}

impl BookCode {
    pub fn category(&self) -> BookCategory {
        match self {
            Gen | Exo | Lev | Num | Deu | Jos | Jdg | Rut | Sa1 | Sa2 | Ki1 | Ki2 | Ch1 | Ch2
            | Ezr | Neh | Est | Job | Psa | Pro | Ecc | Sng | Isa | Jer | Lam | Ezk | Dan | Hos
            | Jol | Amo | Oba | Jon | Mic | Nam | Hab | Zep | Hag | Zec | Mal => {
                BookCategory::OldTestament
            }
            Mat | Mrk | Luk | Jhn | Act | Rom | Co1 | Co2 | Gal | Eph | Php | Col | Th1 | Th2
            | Ti1 | Ti2 | Tit | Phm | Heb | Jas | Pe1 | Pe2 | Jn1 | Jn2 | Jn3 | Jud | Rev => {
                BookCategory::NewTestament
            }
            Tob | Jdt | Esg | Wis | Sir | Bar | Lje | S3y | Sus | Bel | Ma1 | Ma2 | Ma3 | Ma4
            | Es1 | Es2 | Man | Ps2 | Oda | Pss | Eza | Ez5 | Ez6 | Dag | Ps3 | Ba2 | Lba | Jub
            | Eno | Mq1 | Mq2 | Mq3 | Rep | Ba4 | Lao => BookCategory::Deuterocanon,
            Xxa | Xxb | Xxc | Xxd | Xxe | Xxf | Xxg | Frt | Bak | Oth | Int | Cnc | Glo | Tdx
            | Ndx => BookCategory::NonScripture,
            Other(_) => BookCategory::Other,
        }
    }

    pub fn is_old_testament(&self) -> bool {
        self.category() == BookCategory::OldTestament
    }

    pub fn is_new_testament(&self) -> bool {
        self.category() == BookCategory::NewTestament
    }

    pub fn is_deuterocanon(&self) -> bool {
        self.category() == BookCategory::Deuterocanon
    }

    pub fn is_non_scripture(&self) -> bool {
        self.category() == BookCategory::NonScripture
    }

    /// Whether this code is one of the books listed above, rather than a
    /// well-formed code USX accepts that this type does not name. Callers that
    /// report on unusual input (`unlisted-book-code`) ask this; callers that
    /// only need the text use [`BookCode::as_str`].
    pub fn is_listed(&self) -> bool {
        !matches!(self, Other(_))
    }

    /// The three-character code, as USX writes it in `book@code`. Borrowed
    /// from `self` for [`BookCode::Other`], `'static` for every listed book.
    pub fn as_str(&self) -> &str {
        match self {
            Gen => "GEN",
            Exo => "EXO",
            Lev => "LEV",
            Num => "NUM",
            Deu => "DEU",
            Jos => "JOS",
            Jdg => "JDG",
            Rut => "RUT",
            Sa1 => "1SA",
            Sa2 => "2SA",
            Ki1 => "1KI",
            Ki2 => "2KI",
            Ch1 => "1CH",
            Ch2 => "2CH",
            Ezr => "EZR",
            Neh => "NEH",
            Est => "EST",
            Job => "JOB",
            Psa => "PSA",
            Pro => "PRO",
            Ecc => "ECC",
            Sng => "SNG",
            Isa => "ISA",
            Jer => "JER",
            Lam => "LAM",
            Ezk => "EZK",
            Dan => "DAN",
            Hos => "HOS",
            Jol => "JOL",
            Amo => "AMO",
            Oba => "OBA",
            Jon => "JON",
            Mic => "MIC",
            Nam => "NAM",
            Hab => "HAB",
            Zep => "ZEP",
            Hag => "HAG",
            Zec => "ZEC",
            Mal => "MAL",
            Mat => "MAT",
            Mrk => "MRK",
            Luk => "LUK",
            Jhn => "JHN",
            Act => "ACT",
            Rom => "ROM",
            Co1 => "1CO",
            Co2 => "2CO",
            Gal => "GAL",
            Eph => "EPH",
            Php => "PHP",
            Col => "COL",
            Th1 => "1TH",
            Th2 => "2TH",
            Ti1 => "1TI",
            Ti2 => "2TI",
            Tit => "TIT",
            Phm => "PHM",
            Heb => "HEB",
            Jas => "JAS",
            Pe1 => "1PE",
            Pe2 => "2PE",
            Jn1 => "1JN",
            Jn2 => "2JN",
            Jn3 => "3JN",
            Jud => "JUD",
            Rev => "REV",
            Tob => "TOB",
            Jdt => "JDT",
            Esg => "ESG",
            Wis => "WIS",
            Sir => "SIR",
            Bar => "BAR",
            Lje => "LJE",
            S3y => "S3Y",
            Sus => "SUS",
            Bel => "BEL",
            Ma1 => "1MA",
            Ma2 => "2MA",
            Ma3 => "3MA",
            Ma4 => "4MA",
            Es1 => "1ES",
            Es2 => "2ES",
            Man => "MAN",
            Ps2 => "PS2",
            Oda => "ODA",
            Pss => "PSS",
            Eza => "EZA",
            Ez5 => "5EZ",
            Ez6 => "6EZ",
            Dag => "DAG",
            Ps3 => "PS3",
            Ba2 => "2BA",
            Lba => "LBA",
            Jub => "JUB",
            Eno => "ENO",
            Mq1 => "1MQ",
            Mq2 => "2MQ",
            Mq3 => "3MQ",
            Rep => "REP",
            Ba4 => "4BA",
            Lao => "LAO",
            Xxa => "XXA",
            Xxb => "XXB",
            Xxc => "XXC",
            Xxd => "XXD",
            Xxe => "XXE",
            Xxf => "XXF",
            Xxg => "XXG",
            Frt => "FRT",
            Bak => "BAK",
            Oth => "OTH",
            Int => "INT",
            Cnc => "CNC",
            Glo => "GLO",
            Tdx => "TDX",
            Ndx => "NDX",
            // `Other` is built only by `from_str`, from three ASCII bytes.
            Other(code) => std::str::from_utf8(code).expect("a book code is three ASCII bytes"),
        }
    }
}

impl fmt::Display for BookCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnknownBookCodeError;

impl FromStr for BookCode {
    type Err = UnknownBookCodeError;

    /// A listed book by its code, or, failing that, [`BookCode::Other`] for
    /// any code matching `usx.rnc`'s catch-all pattern — `\id TST` is a valid
    /// USX document. Only a code that matches neither is an error.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let upper = s.to_uppercase();
        if let Some(listed) = Self::listed_from_str(&upper) {
            return Ok(listed);
        }
        if is_well_formed_book_code(&upper) {
            let bytes = upper.as_bytes();
            return Ok(Other([bytes[0], bytes[1], bytes[2]]));
        }
        Err(UnknownBookCodeError)
    }
}

impl BookCode {
    /// The listed book with this (already upper-cased) code, if there is one.
    fn listed_from_str(s: &str) -> Option<Self> {
        Some(match s {
            "GEN" => Gen,
            "EXO" => Exo,
            "LEV" => Lev,
            "NUM" => Num,
            "DEU" => Deu,
            "JOS" => Jos,
            "JDG" => Jdg,
            "RUT" => Rut,
            "1SA" => Sa1,
            "2SA" => Sa2,
            "1KI" => Ki1,
            "2KI" => Ki2,
            "1CH" => Ch1,
            "2CH" => Ch2,
            "EZR" => Ezr,
            "NEH" => Neh,
            "EST" => Est,
            "JOB" => Job,
            "PSA" => Psa,
            "PRO" => Pro,
            "ECC" => Ecc,
            "SNG" => Sng,
            "ISA" => Isa,
            "JER" => Jer,
            "LAM" => Lam,
            "EZK" => Ezk,
            "DAN" => Dan,
            "HOS" => Hos,
            "JOL" => Jol,
            "AMO" => Amo,
            "OBA" => Oba,
            "JON" => Jon,
            "MIC" => Mic,
            "NAM" => Nam,
            "HAB" => Hab,
            "ZEP" => Zep,
            "HAG" => Hag,
            "ZEC" => Zec,
            "MAL" => Mal,
            "MAT" => Mat,
            "MRK" => Mrk,
            "LUK" => Luk,
            "JHN" => Jhn,
            "ACT" => Act,
            "ROM" => Rom,
            "1CO" => Co1,
            "2CO" => Co2,
            "GAL" => Gal,
            "EPH" => Eph,
            "PHP" => Php,
            "COL" => Col,
            "1TH" => Th1,
            "2TH" => Th2,
            "1TI" => Ti1,
            "2TI" => Ti2,
            "TIT" => Tit,
            "PHM" => Phm,
            "HEB" => Heb,
            "JAS" => Jas,
            "1PE" => Pe1,
            "2PE" => Pe2,
            "1JN" => Jn1,
            "2JN" => Jn2,
            "3JN" => Jn3,
            "JUD" => Jud,
            "REV" => Rev,
            "TOB" => Tob,
            "JDT" => Jdt,
            "ESG" => Esg,
            "WIS" => Wis,
            "SIR" => Sir,
            "BAR" => Bar,
            "LJE" => Lje,
            "S3Y" => S3y,
            "SUS" => Sus,
            "BEL" => Bel,
            "1MA" => Ma1,
            "2MA" => Ma2,
            "3MA" => Ma3,
            "4MA" => Ma4,
            "1ES" => Es1,
            "2ES" => Es2,
            "MAN" => Man,
            "PS2" => Ps2,
            "ODA" => Oda,
            "PSS" => Pss,
            "EZA" => Eza,
            "5EZ" => Ez5,
            "6EZ" => Ez6,
            "DAG" => Dag,
            "PS3" => Ps3,
            "2BA" => Ba2,
            "LBA" => Lba,
            "JUB" => Jub,
            "ENO" => Eno,
            "1MQ" => Mq1,
            "2MQ" => Mq2,
            "3MQ" => Mq3,
            "REP" => Rep,
            "4BA" => Ba4,
            "LAO" => Lao,
            "XXA" => Xxa,
            "XXB" => Xxb,
            "XXC" => Xxc,
            "XXD" => Xxd,
            "XXE" => Xxe,
            "XXF" => Xxf,
            "XXG" => Xxg,
            "FRT" => Frt,
            "BAK" => Bak,
            "OTH" => Oth,
            "INT" => Int,
            "CNC" => Cnc,
            "GLO" => Glo,
            "TDX" => Tdx,
            "NDX" => Ndx,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_listed_code_wins_over_the_catch_all_pattern() {
        assert_eq!(BookCode::from_str("GEN"), Ok(Gen));
        assert_eq!(BookCode::from_str("1sa"), Ok(Sa1));
        assert!(Gen.is_listed());
    }

    /// `usx.rnc` ends `book@code` with
    /// `[A-Z][A-Z0-9]{2}|[0-9][A-Z][0-9]|[0-9]{2}[A-Z]`, so these are valid
    /// codes even though no book is named for them, and they round-trip.
    #[test]
    fn a_well_formed_code_becomes_other_and_round_trips() {
        for code in ["TST", "ZZZ", "XY9", "1A2", "12B"] {
            let parsed = BookCode::from_str(code).expect(code);
            assert!(!parsed.is_listed(), "{code} should not be listed");
            assert_eq!(parsed.as_str(), code);
            assert_eq!(parsed.to_string(), code);
            assert_eq!(parsed.category(), BookCategory::Other);
        }
        // Codes are upper-cased first, as they always have been: tcdocs has a
        // `\id php`, which is Philippians.
        assert_eq!(BookCode::from_str("g3n").unwrap().as_str(), "G3N");
    }

    #[test]
    fn a_code_matching_neither_is_an_error() {
        for code in ["", "GE", "GENESIS", "1A", "A-B", "A B", "123", "1B2C"] {
            assert_eq!(
                BookCode::from_str(code),
                Err(UnknownBookCodeError),
                "{code} is not a book code"
            );
        }
    }
}
