# 54. Parse the references written in `\xt` (and the like), in the project's own punctuation

Status: ready-for-agent
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
