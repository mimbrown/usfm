# 19. `usfm_semantic`: the crate, the union in the facade, the first moved check

Status: resolved
Milestone: M4

The parser should report only what it needs to recover (spec, M4). Everything
else it reports today is a document check that reads the finished tree and
the stylesheet, and belongs in `usfm_semantic` (ADR: `ReferenceIndex`,
placement and numbering checks). This ticket builds the crate and the plumbing
with one check, so the later moves are mechanical.

- New crate `crates/usfm_semantic` depending on `usfm_ast`, `usfm_style`,
  `usfm_diagnostics`, `usfm_span`. `pub fn analyze(&Document) -> Vec<Diagnostic>`:
  a `usfm_ast::visit::Visit` over the document holding
  `document.style_sheet()`, one method per check, diagnostics collected then
  sorted by span start (stable, so two at one offset keep emission order).
- The facade: `usfm::parse` / `parse_with` return a `ParseResult` whose
  diagnostics are the parser's plus `analyze`'s, merged and sorted by span.
  `usfm_parser::Parser::parse` keeps returning the parser's own only. The CLI,
  the conformance harness (`tasks/conformance`), the fuzz targets and
  `tasks/benchmark`'s `parse` group call the facade, so tcdocs is unchanged
  (231 / 0 / 44) and the fuzz seeds still exercise the checks. Add a
  `parse_semantic` bench group (facade parse) beside `parse`, recorded in
  `docs/benchmarks.md` as a new row.
- First moved check, to prove the plumbing: `unlisted-book-code` (Warning;
  recovery none, the book is kept). The `Code` stays in `usfm_diagnostics`;
  the parser stops emitting it; `analyze` emits it. Its tests move from
  `crates/usfm_parser/tests/recovery.rs` to
  `crates/usfm_semantic/tests/checks.rs` with their snapshots (`git mv` the
  `.snap` files, keep names), through a `check(code, source)` helper that
  parses with the facade. The rule "every `Code` has a test" now spans the
  two files; say so at the top of both.
- Record the D4 decision in the spec's open list: verse-end emission stays in
  the parser (USX needs it and it shapes the tree), unless this ticket finds a
  reason not to.

Done when the gate is green, tcdocs unchanged, and `unlisted-book-code` is
reported by `usfm::parse` and not by `usfm_parser::Parser::parse`.

## Answer

Landed in `bb33209` (PR #23, 2026-09-19). `crates/usfm_semantic`: `analyze(&Document)
-> Vec<Diagnostic>`, an `Analyzer: Visit` holding the document's stylesheet,
diagnostics stably sorted by span start; never depends on the parser. The
facade's `parse`/`parse_with`/new `parse_with_options` return the union
(parser first, then semantic, stable sort). The harness, the fuzz targets,
the CLI driver and a new `parse_semantic` bench group go through the facade
(whole-corpus 47.5 vs `parse` 49.0 MiB/s: about 8 ms of walk). First moved
check: `unlisted-book-code`, now reported over the whole `\id` line (the
`Book` node's span; the parser had the code word's). `Code::is_semantic()`
splits the one-snapshot-per-code rule between `recovery.rs` and
`usfm_semantic/tests/checks.rs`. D4 settled in the spec: verse-end emission
stays in the parser. Not under Miri (no byte handling).
