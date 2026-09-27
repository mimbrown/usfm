//! `GlossaryIndex` over parsed documents (ticket 55): the glossary is its
//! own book, the words are in another.

use usfm_semantic::GlossaryIndex;

const GLOSSARY: &str = "\\id GLO\n\\c 1\n\\p \\k Grace\\k* God's favour.\n\
                        \\c 2\n\\p \\k Holy  Spirit\\k* The Spirit of God.\n\
                        \\p \\k grace\\k* Defined twice.\n";

const TEXT: &str = "\\id MAT\n\\c 1\n\\p \\v 1 By \\w grace\\w*, by the \\w Spirit|Holy Spirit\\w*,\
                    and by \\w favour|lemma=\"grace\"\\w* and \\w faith\\w*.\n";

#[test]
fn a_word_finds_the_keyword_that_defines_it() {
    let glossary = usfm::parse(GLOSSARY).document;
    let text = usfm::parse(TEXT).document;
    let index = GlossaryIndex::new(&[&glossary]);

    let words: Vec<(String, Option<Option<usize>>)> = index
        .words(&text)
        .into_iter()
        .map(|word| {
            let chapter = word.entry.map(|at| index.entries()[at].chapter);
            (word.term, chapter)
        })
        .collect();
    assert_eq!(
        words,
        [
            ("grace".to_string(), Some(Some(1))),
            ("Holy Spirit".to_string(), Some(Some(2))),
            ("grace".to_string(), Some(Some(1))),
            ("faith".to_string(), None),
        ]
    );

    let unresolved: Vec<String> = index
        .unresolved(&text)
        .into_iter()
        .map(|w| w.term)
        .collect();
    assert_eq!(unresolved, ["faith"]);
}

#[test]
fn an_entry_knows_where_it_is_and_a_repeat_is_a_duplicate() {
    let glossary = usfm::parse(GLOSSARY).document;
    let index = GlossaryIndex::new(&[&glossary]);

    let spirit = index.get("holy spirit").unwrap();
    assert_eq!(spirit.term, "Holy Spirit");
    assert_eq!(spirit.book.map(|b| b.to_string()).as_deref(), Some("GLO"));
    assert_eq!(spirit.chapter, Some(2));
    assert_eq!(spirit.document, 0);
    assert!(GLOSSARY[spirit.span.start as usize..].starts_with("\\k Holy"));

    let duplicates: Vec<&str> = index.duplicates().map(|e| e.term.as_str()).collect();
    assert_eq!(duplicates, ["grace"]);
    // The first definition wins.
    assert_eq!(index.get("GRACE").unwrap().chapter, Some(1));
}
