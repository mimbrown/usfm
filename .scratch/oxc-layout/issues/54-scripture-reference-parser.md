# 54. Parse the references written in `\xt` (and the like), in the project's own punctuation

Status: resolved
Milestone: after M7 (render gaps)
Blocked by: 53

Found by mapping Shahkar-Urdu-Apps/render (gap 3). render turns the text of
each `\xt` into structured links (`render:lib/usfm/parse/makeReferenceParser.ts`),
using the project's reference punctuation from `Settings.xml` and the
abbreviations from `BookNames.xml`; its app makes them tappable. We keep
`\xt` as a `Char` with `link-href` and never read the text.

- `usfm_semantic::references`: a `ReferenceFormat` (chapter–verse
  separator, range indicator, sequence indicator, chapter-number separator,
  book-sequence separator, final punctuation; Paratext's defaults by
  default) and book names; `parse_references(text, format, names,
  default_book)` returns the text cut into plain runs and references, each a
  book, a chapter and a verse range with its segment, and the byte range it
  was read from.
- Digits are any Unicode decimal digits (Arabic-Indic, Extended
  Arabic-Indic, Devanagari…), not ASCII only — render's parser misses Urdu
  numerals because JavaScript's `\d` is ASCII.
- A reference with no book takes the one before it (or the default); a verse
  with no chapter takes the chapter before it (`5:3, 7` is 5:3 and 5:7).
- A range keeps its end, where render's keeps only the start.
- `usfm_paratext` (ticket 53) builds the `ReferenceFormat` and names from a
  project.
- Not in this ticket: a semantic check that the target exists (it needs
  the other books), and rewriting `link-href` from the parse.

## Answer

`usfm_semantic::citation` (named apart from `reference`, which is the
`ReferenceIndex` of a document's own `\c`/`\v`): `CitationFormat`
(Paratext's defaults; `link_href()` for `link-href` values),
`BookNameTable` (names longest first; `with_codes()` / `codes()` also take a
listed book's three-letter code), `parse_citations(text, format, names,
default_book) -> Vec<Piece>` where a `Piece` is `Text(range)` or
`Citation { book, start, end, range }`, and `xt_citations(&document, …)`,
which reads every `\xt` with the book of the `\id` before it as the
default. `usfm_paratext` fills both from a project:
`Settings::citation_format()` (each setting split on `|`, defaults for the
rest) and `BookNames::table()` (abbreviation, short and long names, and the
codes).

The rules are in the module doc; the ones that go past render's parser:
any Unicode decimal digits, a range keeps its end, a segment letter is
kept, a one-chapter book reads a bare number as a verse, directional marks
inside a reference are skipped, and a bare number with no book name and no
reference before it is not a reference.

Tests: `citation.rs`'s unit tests (English and an Urdu reference with RLMs,
the Arabic comma and semicolon), `tests/citation.rs` over a parsed
document, and the fixture project's own spelling in `usfm_paratext`.

Not done, as the ticket said: checking that the target exists, and
rewriting `link-href` from the parse.
