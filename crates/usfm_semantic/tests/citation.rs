//! `xt_citations` over a parsed document (ticket 54): every `\xt`, with the
//! book of the `\id` before it as the default.

use usfm_semantic::citation::{BookNameTable, CitationFormat, Piece, xt_citations};

#[test]
fn every_xt_is_read_with_its_book_as_the_default() {
    let source = "\\id MAT\n\\c 5\n\\p \\v 3 Blessed\\x - \\xo 5:3 \\xt Lk 6:20; 4:18\\xt*\\x* are the poor.\n\
                  \\v 4 Blessed \\x - \\xt 11:28\\xt*\\x* are those who mourn.\n";
    let document = usfm::parse(source).document;
    let mut names = BookNameTable::new();
    names.add("Lk", "LUK".parse().unwrap());

    let found = xt_citations(&document, &CitationFormat::default(), &names);
    let described: Vec<Vec<String>> = found
        .iter()
        .map(|xt| {
            xt.pieces
                .iter()
                .filter_map(|p| match p {
                    Piece::Citation(c) => Some(format!(
                        "{} {}:{}",
                        c.book,
                        c.start.chapter,
                        c.start.verse.unwrap()
                    )),
                    Piece::Text(_) => None,
                })
                .collect()
        })
        .collect();
    assert_eq!(described, [vec!["LUK 6:20", "LUK 4:18"], vec!["MAT 11:28"]]);
    // The span is the `\xt` node's, in the source.
    assert!(source[found[0].span.start as usize..].starts_with("\\xt Lk"));
}
