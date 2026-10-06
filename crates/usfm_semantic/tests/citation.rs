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

/// A bare number in an `\\xt` is a verse as often as a chapter, and the text
/// alone does not say which, so it is not read. A `link-href` does say, and
/// is believed over the text — found in two real projects that write
/// "see verse `\\xt 45|MAT 5:45\\xt*`" and "chapters `\\xt 21|1SA 21:1-15\\xt*`"
/// (2026-10-06).
#[test]
fn a_link_href_is_believed_over_the_text() {
    let source = "\\id MAT\n\\c 5\n\\p \\v 3 see verse \\xt 45|Mat 5\u{200F}:45\\xt*, chapters \\xt 21|link-href=\"1SA 21:1-15\"\\xt*, \
                  \\xt 7\\xt*, \\xt 6:1|https://example.org\\xt* and \\xt Lk 1:2|LUK 3:4\\xt*.\n";
    let document = usfm::parse(source).document;
    let mut names = BookNameTable::new();
    names.add("Lk", "LUK".parse().unwrap());

    let found = xt_citations(&document, &CitationFormat::default(), &names);
    let described: Vec<Vec<String>> = found
        .iter()
        .map(|xt| {
            xt.citations()
                .iter()
                .map(|c| format!("{} {}:{}", c.book, c.start.chapter, c.start.verse.unwrap()))
                .collect()
        })
        .collect();
    assert_eq!(
        described,
        [
            vec!["MAT 5:45"],
            vec!["1SA 21:1"],
            // No attribute: nothing says whether 7 is a verse or a chapter.
            vec![],
            // An attribute that is not Scripture: the text is all there is.
            vec!["MAT 6:1"],
            // Where the two disagree, the attribute is the author's word.
            vec!["LUK 3:4"],
        ]
    );
    assert!(found[0].pieces.iter().all(|p| matches!(p, Piece::Text(_))));
    assert_eq!(found[0].link.as_ref().unwrap().value, "Mat 5\u{200F}:45");
    assert!(found[2].link.is_none() && found[3].link.is_none());
}
