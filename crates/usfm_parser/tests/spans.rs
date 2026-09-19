//! Spans point at the source they were read from.
//!
//! The snapshot corpus records span values, but a recorded value is only as
//! good as the eye that accepted it. These tests check the invariants
//! mechanically, so a span cannot be silently wrong in a way that still looks
//! plausible in a snapshot.
//!
//! The walk and the invariants themselves live in `usfm_parser::span_check`
//! (behind the `testing` feature), because `tasks/fuzz` asserts the same ones
//! over inputs nobody wrote by hand.

mod common;

use usfm_parser::DEFAULT_STYLESHEET;
use usfm_parser::ast::*;
use usfm_parser::diagnostics::Code;
use usfm_parser::parser::Parser;
use usfm_parser::span_check::check;

#[test]
fn simple_paragraph() {
    check("\\id GEN\n\\c 1\n\\p \\v 1 In the beginning.\n");
}

#[test]
fn nested_character_styles() {
    check("\\id GEN\n\\c 1\n\\p \\v 1 a \\nd Lord\\+add ly\\+add*\\nd* b\n");
}

#[test]
fn implicitly_closed_character_style() {
    check("\\id GEN\n\\c 1\n\\p \\v 1 a \\em b\n\\p \\v 2 c\n");
}

#[test]
fn optional_line_breaks() {
    check("\\id GEN\n\\c 1\n\\s1 Jesus Heals a Man // Who Could Not Walk\n\\p \\v 1 a//b\n");
}

#[test]
fn notes_and_attributes() {
    check("\\id GEN\n\\c 1\n\\p \\v 1 \\w grace|lemma=\"grace\"\\w* \\f + \\ft note\\f* end\n");
}

#[test]
fn milestones_inline_and_between_blocks() {
    check("\\id GEN\n\\c 1\n\\ts\\*\n\\p \\v 1 a \\qt-s |who=\"Jesus\"\\* b \\qt-e\\* c\n");
}

#[test]
fn chapter_with_alternate_and_published_numbers() {
    check("\\id MAT\n\\c 1\n\\ca 2\\ca*\n\\cp M\n\\p \\v 1 text\n");
}

#[test]
fn table() {
    check("\\id GEN\n\\c 1\n\\tr \\th1 Header \\th2 Second\n\\tr \\tc1 One \\tc2 Two\n");
}

#[test]
fn malformed_input_still_has_sane_spans() {
    check("\\id GEN\n\\c 1\n\\p \\v 1 a \\em b \\foo c \\* d \\bk* e\n");
}

#[test]
fn normalized_whitespace_and_escapes() {
    check("\\id GEN\n\\c 1\n\\p \\v 1 a\\\\b   c\nd \\em e\\em*\n");
}

/// The three nodes that can carry an attribute list — a character style, a
/// milestone (inline and between blocks) and a `\periph` line — each with one,
/// so the checker sees the `|` and every pair in every position it can appear
/// in. `Attributes::pipe` and `Attribute::span` arrived with ticket 20 and are
/// checked from ticket 21 on: the `|` is one byte of `|`, and a pair's span is
/// the name it was read from, or the value run for a bare one.
#[test]
fn attribute_lists_on_every_node_that_can_carry_one() {
    check(
        "\\id FRT\n\\periph Title Page|id=\"title\"\n\\p a\n\
         \\id GEN\n\\c 1\n\\ts-s |x-a=\"1\" x-b=\"2\"\\*\n\
         \\p \\v 1 \\w grace|lemma=\"grace\" strong=\"H1\"\\w* \
         \\w plain|grace\\w* \\qt-s |Jesus\\* b \\qt-e\\*\n",
    );
    // A bare value the parser borrows verbatim, quotes included, and an
    // unquoted one it reads as a single word: both spans are the run read.
    check("\\id GEN\n\\c 1\n\\p \\v 1 \\rb b|\"h=c\"\\rb* \\w x|lemma=y\\w*\n");
    // Lists the parser had to recover in: no attribute at all, a name that is
    // not an identifier, a value with no closing quote, a line break inside
    // the list.
    check("\\id GEN\n\\c 1\n\\p \\v 1 \\w a| \\w* \\w b|c<d=\"1\"\\w* \\w e|f=\"g\n\\w* h\n");
    check("\\id GEN\n\\c 1\n\\p \\v 1 \\w a|lemma=\"b\"\n   strong=\"c\"\\w*\n");
}

/// `into_owned` must detach the document from the source (plan D5): the
/// document has to still be usable, and unchanged, once the input is gone.
#[test]
fn into_owned_outlives_the_source() {
    fn parse_detached(source: &str) -> Document<'static> {
        // `source` is borrowed only inside this function, so the returned
        // document cannot be borrowing it — the signature is the assertion.
        Parser::new(source)
            .parse(&DEFAULT_STYLESHEET)
            .document
            .into_owned()
    }

    let source = String::from(
        "\\id GEN Genesis\n\\c 1\n\\ca 2\\ca*\n\\cp M\n\\p \\v 1 \\w grace|lemma=\"grace\"\\w* \
         \\f + \\ft note\\f* \\zaln-s |x-strong=\"G1\"\\* a \\zaln-e\\*\n\
         \\tr \\tc1 one \\tc2 two\n",
    );
    let borrowed = Parser::new(&source).parse(&DEFAULT_STYLESHEET).document;
    let rendered = format!("{borrowed:?}");

    let owned = parse_detached(&source);
    drop(source);

    // Same tree, and still resolvable: the stylesheet is shared, not copied.
    assert_eq!(format!("{owned:?}"), rendered);
    let Block::Para(para) = &owned.blocks[2] else {
        panic!("expected a paragraph, got {:?}", owned.blocks[2]);
    };
    assert_eq!(owned.marker(para.style), "p");
}

/// Content with no paragraph marker of its own is kept in an implicit `\p`
/// (`content-outside-paragraph`), and `\tr` with no cell marker opens an
/// implicit `\tc1` (`expected-table-cell`). Neither has a marker in the
/// source, so the node's span starts at the content it holds.
///
/// The fuzzer found this on a source of one byte, `\`: a stray backslash is
/// text, and text outside a paragraph opens an implicit one.
#[test]
fn implicit_nodes_span_the_content_they_hold() {
    for source in [
        "\\",
        "\\id GEN\n\\c 1\nloose text\n",
        // `\c in` is not a chapter number, so `x` is left outside a paragraph.
        "\\id GEN\n\\c in x\n",
    ] {
        assert!(
            common::codes(source).contains(&Code::ContentOutsideParagraph),
            "{source:?} was expected to open an implicit paragraph"
        );
        check(source);
    }

    // The paragraph is opened at the `\v`, which is then dropped for want of
    // a number: it starts before the content that is left. Also the fuzzer's.
    check("\\v\\");

    let table = "\\id GEN\n\\c 1\n\\tr cell without a marker\n";
    assert!(common::codes(table).contains(&Code::ExpectedTableCell));
    check(table);
}

/// A `\v` on a `\periph` title line leaves a verse end and a synthesized
/// space among the title's children. A synthesized node carries `SPAN`, so
/// taking the title's end from the last text child put the end at offset 0 and
/// inverted the span. The fuzzer found it; only text read from the source
/// counts now.
#[test]
fn periph_title_span_ignores_synthesized_text() {
    check("\\id GEN\n\\c 1\n\\periph T \\v 2 b \\v 3\n");
}

/// A `\periph` title is a `Text`, so its span is the source it was read from
/// and its content is that source normalised: `\periph\* n` has the title
/// "n" and the span of " n". The fuzzer found this input; the checker used to
/// expect the span to start at the title's first character.
#[test]
fn periph_title_span_is_the_source_it_was_read_from() {
    check("\\periph\\* n");
    check("\\id GEN\n\\periph \\nd Lord\\nd* of hosts|id=\"x\"\nbody\n");
}
