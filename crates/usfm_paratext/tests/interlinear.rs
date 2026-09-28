//! Paratext 9 interlinear glosses read and placed (ticket 56), over
//! `tests/fixtures/pt9`: SIL's interlinearizer extension's four test
//! projects (MIT, `LICENSE` beside them), each with an invented Philippians
//! and the files Paratext 9 keeps its glosses in.
//!
//! - PIA: two gloss languages, words, parses, phrases, repeats.
//! - PIB: every way a cluster fails to land, and a book with no text.
//! - PIC: a canonical file beside an older top-level twin, and files that
//!   leave out their language or book.
//! - PID: a lexicon and no interlinear file.

use std::path::{Path, PathBuf};

use usfm_ast::BookCode;
use usfm_paratext::anchor::{self, Anchoring, DropReason};
use usfm_paratext::{ClusterKind, InterlinearBook, LexemeKey, LexemeType, Lexicon, Project};
use usfm_semantic::ReferenceIndex;

fn project(name: &str) -> Project {
    let dir: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/pt9")
        .join(name);
    Project::open(dir).unwrap()
}

fn php() -> BookCode {
    "PHP".parse().unwrap()
}

/// The verse's text as a `Document` holds it, notes left out.
fn verse_text(project: &Project, chapter: usize, verse: usize) -> String {
    let source = project.read_book(php()).unwrap();
    let sheet = project.style_sheet().unwrap();
    let document = usfm_parser::parser::Parser::new(&source)
        .parse(&sheet)
        .document;
    let index = ReferenceIndex::new(&document);
    index.verse(chapter, verse).unwrap().text()
}

/// Each anchored word or phrase as `words = gloss`, in `language`.
fn glossed(
    text: &str,
    book: &InterlinearBook,
    lexicon: &Lexicon,
    chapter: usize,
    verse: usize,
    language: &str,
) -> (Vec<String>, Anchoring) {
    let clusters = &book.verse(chapter, verse).unwrap().clusters;
    let anchoring = anchor::anchor(text, clusters);
    let word = |w: usize| &text[anchoring.words[w].clone()];
    let gloss = |cluster: usize| {
        clusters[cluster]
            .lexemes
            .iter()
            .map(|lexeme| lexicon.gloss(lexeme, language).unwrap_or("?"))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let mut out = Vec::new();
    for anchored in &anchoring.anchored_words {
        for cluster in [anchored.gloss, anchored.parse].into_iter().flatten() {
            out.push(format!("{} = {}", word(anchored.word), gloss(cluster)));
        }
    }
    for phrase in &anchoring.phrases {
        let words: Vec<&str> = phrase.words.clone().map(word).collect();
        out.push(format!("{} = {}", words.join(" "), gloss(phrase.cluster)));
    }
    (out, anchoring)
}

#[test]
fn a_book_file_and_the_lexicon_are_read() {
    let pia = project("PIA");
    assert_eq!(pia.interlinear_languages().unwrap(), ["es", "fr"]);
    let book = pia.interlinear("es", php()).unwrap().unwrap();
    assert_eq!(book.gloss_language.as_deref(), Some("es"));
    assert_eq!(book.book(), Some(php()));
    assert_eq!(book.verses.len(), 8);

    let first = book.verse(1, 1).unwrap();
    assert_eq!(
        first.approved_hash.as_deref(),
        Some("pia-words-approved-hash")
    );
    let phrase = &first.clusters[3];
    assert_eq!((phrase.range.index, phrase.range.length), (16, 7));
    assert_eq!(phrase.kind(), ClusterKind::Phrase);
    assert_eq!(phrase.lexemes[0].sense.as_deref(), Some("s-daxfep"));
    assert!(!book.verse(1, 8).unwrap().is_approved());

    let second = book.verse(1, 2).unwrap();
    assert!(second.clusters[0].excluded);
    let punctuation = &second.punctuation[0];
    assert_eq!(punctuation.before.as_deref(), Some("."));
    assert_eq!(punctuation.after.as_deref(), Some(";"));

    let parse = &book.verse(1, 3).unwrap().clusters[0];
    assert_eq!(parse.kind(), ClusterKind::Parse);
    assert_eq!(parse.surface().as_deref(), Some("wugs"));

    let lexicon = pia.lexicon().unwrap().unwrap();
    let wug = lexicon
        .entry(&LexemeKey::parse("Word:wug").unwrap())
        .unwrap();
    assert_eq!(wug.senses.len(), 2);
    assert_eq!(lexicon.sense("s-wug1").unwrap().gloss("fr"), Some("un"));
    assert_eq!(lexicon.sense("s-wug2").unwrap().gloss("fr"), None);
    let analysis = &lexicon.analyses()[0];
    assert_eq!(analysis.word, "flibs");
    assert_eq!(analysis.lexemes[1].kind, LexemeType::Suffix);
}

#[test]
fn words_parses_and_phrases_land_on_the_verse_text() {
    let pia = project("PIA");
    let book = pia.interlinear("es", php()).unwrap().unwrap();
    let lexicon = pia.lexicon().unwrap().unwrap();

    let text = verse_text(&pia, 1, 1);
    assert_eq!(text, "Wug blicket wug dax fep gorp.");
    let (glosses, anchoring) = glossed(&text, &book, &lexicon, 1, 1, "es");
    // The second `wug` chose no sense, so it shows the entry's first; `gorp`
    // names a sense the lexicon does not have.
    assert_eq!(
        glosses,
        [
            "Wug = uno",
            "blicket = dos",
            "wug = uno",
            "gorp = ?",
            "dax fep = tres cuatro"
        ]
    );
    assert!(anchoring.dropped.is_empty());

    // A word and its parse over one range land together.
    let text = verse_text(&pia, 1, 3);
    let (glosses, anchoring) = glossed(&text, &book, &lexicon, 1, 3, "es");
    assert_eq!(
        glosses,
        [
            "Wugs = wug -s",
            "daxes = dax -es",
            "glorp = ocho",
            "glorp = glorp"
        ]
    );
    assert_eq!(anchoring.anchored_words[2].gloss, Some(2));
    assert_eq!(anchoring.anchored_words[2].parse, Some(3));

    // Phrases may overlap; the first `zim zam` is ambiguous and the range
    // puts it at the start.
    let text = verse_text(&pia, 1, 4);
    let (glosses, anchoring) = glossed(&text, &book, &lexicon, 1, 4, "es");
    assert_eq!(
        glosses,
        [
            "Zim zam = nueve diez",
            "zam zim = diez nueve",
            "zim zam = nueve diez"
        ]
    );
    let words: Vec<_> = anchoring.phrases.iter().map(|p| p.words.clone()).collect();
    assert_eq!(words, [0..2, 1..3, 2..4]);
    assert!(anchoring.phrases[0].ambiguous);

    // A repeated form: each cluster's range picks its own `plovs`.
    let text = verse_text(&pia, 1, 6);
    let (_, anchoring) = glossed(&text, &book, &lexicon, 1, 6, "es");
    let placed: Vec<_> = anchoring
        .anchored_words
        .iter()
        .map(|w| (w.word, w.gloss, w.parse, w.ambiguous))
        .collect();
    assert_eq!(
        placed,
        [(1, Some(0), Some(1), true), (3, Some(2), Some(3), false)]
    );
}

#[test]
fn a_cluster_that_cannot_land_is_dropped_with_its_reason() {
    let pib = project("PIB");
    let book = pib.interlinear("es", php()).unwrap().unwrap();
    let text = verse_text(&pib, 1, 1);
    assert_eq!(text, "Alpha beta gamma delta.");
    let anchoring = anchor::anchor(&text, &book.verse(1, 1).unwrap().clusters);
    assert_eq!(
        anchoring.dropped,
        [
            (2, DropReason::Duplicate),
            (3, DropReason::FormMismatch),
            (4, DropReason::NoSurface),
            (5, DropReason::NoSurface),
            (6, DropReason::Unparseable),
            (7, DropReason::Unparseable),
            (9, DropReason::FormMismatch),
            (10, DropReason::FormMismatch),
        ]
    );
    let words: Vec<_> = anchoring.anchored_words.iter().map(|w| w.word).collect();
    assert_eq!(words, [0, 1]);
    assert_eq!(anchoring.phrases[0].words, 2..4);

    // Glosses for a verse, and a book, the text does not have.
    assert!(book.verse(2, 5).is_some());
    let james: BookCode = "JAS".parse().unwrap();
    assert!(pib.interlinear("es", james).unwrap().is_some());
    assert!(!pib.books().contains(&james));
}

#[test]
fn the_canonical_file_wins_over_a_top_level_twin() {
    let pic = project("PIC");
    assert_eq!(
        pic.interlinear_languages().unwrap(),
        ["English", "es", "es-MX"]
    );
    let canonical = pic.interlinear("es", php()).unwrap().unwrap();
    let cluster = &canonical.verse(1, 1).unwrap().clusters[0];
    assert_eq!(cluster.lexemes[0].id.as_deref(), Some("Word:uno"));

    // A file that leaves out its book or its language still reads.
    let dir = pic.dir();
    let no_book = InterlinearBook::from_xml(
        &std::fs::read_to_string(dir.join("Interlinear_nobookid.xml")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        (no_book.book(), no_book.gloss_language.as_deref()),
        (None, Some("es"))
    );
    let no_language = InterlinearBook::from_xml(
        &std::fs::read_to_string(dir.join("Interlinear_noglosslang.xml")).unwrap(),
    )
    .unwrap();
    assert_eq!(no_language.gloss_language, None);
}

#[test]
fn a_project_may_have_a_lexicon_and_no_interlinear() {
    let pid = project("PID");
    assert!(pid.interlinear_languages().unwrap().is_empty());
    assert!(pid.interlinear("es", php()).unwrap().is_none());
    assert!(!pid.lexicon().unwrap().unwrap().is_empty());
    // And one with neither has no lexicon rather than an error.
    let ssvx =
        Project::open(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/SSVx")).unwrap();
    assert!(ssvx.lexicon().unwrap().is_none());
}

#[test]
fn a_malformed_file_is_an_error() {
    let bad_range = r#"<InterlinearData><Verses><item><string>PHP 1:1</string>
        <VerseData><Cluster><Range Index="x" Length="1" /></Cluster></VerseData>
        </item></Verses></InterlinearData>"#;
    assert!(InterlinearBook::from_xml(bad_range).is_err());
    let bad_type =
        r#"<Lexicon><Entries><item><Lexeme Type="Nope" Form="x" /></item></Entries></Lexicon>"#;
    assert!(Lexicon::from_xml(bad_type).is_err());
    assert!(Lexicon::from_xml("<Other />").is_err());
    // A repeated verse keeps the last occurrence.
    let repeated = r#"<InterlinearData><Verses>
        <item><string>PHP 1:1</string><VerseData Hash="a" /></item>
        <item><string>PHP 1:1</string><VerseData Hash="b" /></item>
        </Verses></InterlinearData>"#;
    let book = InterlinearBook::from_xml(repeated).unwrap();
    assert_eq!(book.verses.len(), 1);
    assert_eq!(book.verses[0].approved_hash.as_deref(), Some("b"));
}
