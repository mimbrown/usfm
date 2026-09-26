# 45. `usfm_usx::read_usx`: USX into a `Document`

Status: ready-for-agent
Milestone: M7

The reader itself, over the vocabulary the conformance references use. The
decisions it rests on are in the spec's M7 section; read them first.

- `roxmltree` as a workspace dependency, `crates/usfm_usx/src/read.rs`, and
  `usfm_usx::read_usx(&str) -> ParseResult<Document<'_>>` plus
  `read_usx_with(&str, &Arc<StyleSheet>)`. `usfm_usx` gains a dependency on
  `usfm_diagnostics`.
- Every element the references use (tcdocs counts: `usx`, `book`, `chapter`,
  `para`, `verse`, `char`, `note`, `ms`, `optbreak`, `table`/`row`/`cell`,
  `sidebar`, `periph`, `figure`, `ref`, `unmatched`) maps onto the AST node the
  writer in `usx.rs` takes it from, attribute for attribute: `altnumber`,
  `pubnumber`, `caller`, `category`, `align`, `colspan`, the
  `<usx version>` as the `\usfm` paragraph (`Document::usfm_version()`).
  `vid` is derived data and is read only to check it, not stored.
- Spans are byte ranges from `roxmltree`; `crates/usfm_parser`'s span
  invariants (`usfm_parser::span_check`) hold for a read tree too, or the
  ticket says which one cannot and why.
- The `usx-…` codes the spec names, their tests in
  `crates/usfm_usx/tests/reader.rs`, and the coverage rule extended to three
  origins (`Code::origin()`, or `is_usx()` beside `is_semantic()`). Unknown
  styles use the parser's codes.
- Whitespace as the spec's decision says, with a test on DBL's
  pretty-printed shape (`<verse … />\n    <char …>`).
- A test that reads every reference of both roots and asserts no Error on the
  `pass` cases, and one that compares the read tree with `usfm::parse` of
  `origin.usfm` by `eq_ignoring_spans` after the normalisations USX forces
  (the spec's "What USX does not carry"). Measure that list rather than
  guessing it; each entry names the construct and a case that shows it.

Done when the gate is green, both tests pass over every case they cover, and
CLAUDE.md's Project Structure and `usfm_usx`'s module doc name the reader.
