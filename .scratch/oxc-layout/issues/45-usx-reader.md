# 45. `usfm_usx::read_usx`: USX into a `Document`

Status: resolved
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

## Answer

Landed 2026-09-27 (PR below). `usfm_usx::read_usx` / `read_usx_with` in
`crates/usfm_usx/src/read.rs`, on `roxmltree` 0.21.

- **`DEFAULT_STYLESHEET` moved to `usfm_style`** (with `build.rs`, `usfm.sty`
  and `usfm-extra.sty`): the reader resolves styles through it and an output
  crate may not depend on the parser. `usfm_parser` re-exports it under the
  old name, so no caller changed.
- Five codes, `Origin::Usx`: `usx-not-well-formed`, `usx-unknown-element`,
  `usx-unmatched`, `usx-verse-end-mismatch`, and `usx-reference-mismatch`
  (a `sid`/`vid` that does not name what the numbers make; Warning, since
  both are derived). `Code::origin() -> Origin { Parser, Semantic, Usx }`
  replaces `is_semantic()` as the coverage rule (`is_semantic` stays, as
  `origin() == Semantic`); the third coverage test is
  `crates/usfm_usx/tests/reader.rs::usx_codes_are_covered`. Where a USX file
  is wrong the way its USFM would be, the parser's code is reused.
- `tasks/conformance/tests/usx_reader.rs` (it needs the harness's patching
  and comparison), in the gate by name since the gate's `cargo test`
  excludes `usfm_tests`: all 271 references read without panic and no `pass`
  case has an Error; every case the harness passes writes back to its
  reference; the read tree equals `usfm::parse(origin.usfm)` modulo the
  measured list; span invariants. No known list.
- **What USX cannot say, measured:** a default attribute gains its name
  (`specExamples/cross-ref`); a document with no `\usfm` gains `\usfm 3.0`
  after `\id` (USX always declares a version); a note's trailing whitespace
  and the usfm-grammar root's whitespace are not compared (the harness's own
  rules). The empty `|` list the spec expected occurs in no compared case
  (the reader reads it as no list, pinned in `reader.rs`); `\+` is not in the
  tree at all.
- Decisions beyond the spec: whitespace-only text containing a line break is
  formatting even inside mixed content (Paratext indents a note's `<char>`s
  where there is no space; DBL's line breaks sit only after `<para>` and
  `<verse/>`); a verse end goes before the whitespace in front of it, as the
  parser places it; verse and chapter ends carry `SPAN`; a node's span starts
  at `<`, not at a marker; an unknown element's content is read in its place;
  `<ms>` between blocks is always `Block::Milestone` (ticket 46's USX -> USFM
  -> USX step would catch a shape USFM cannot spell).
- Left for later tickets: 46 (gate steps), 47 (USX 2 `eid` synthesis,
  machine.py USX), 48 (facade, CLI), 49 (fuzz, Miri, bench).
  `tasks/fuzz/Cargo.lock` updates when the fuzz crate is next built.
