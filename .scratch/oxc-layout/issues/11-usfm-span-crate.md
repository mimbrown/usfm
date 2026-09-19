# 11. `usfm_span`: the first leaf crate

Status: resolved
Milestone: M3

`usfm_ast/src/span.rs` (736 lines: `Span`, `SPAN`, the oxc-derived helpers,
19 `ignore`d doc examples) becomes its own crate, the root of the dependency
order in the ADR (`span <- style, ast <- diagnostics <- parser`).

- New crate `usfm_span` (workspace member, MIT, no dependencies). Move
  `span.rs` there; `usfm_ast` depends on it and keeps `pub use usfm_span::*`
  as `usfm_ast::span` so every existing path still resolves; `usfm_parser`'s
  `lexer::span` re-export stays.
- Add the line/column lookup the ADR names: `LineIndex::new(&str)` with
  `line_col(offset) -> (line, col)` (1-based, columns in chars), built once
  per source, and tests for CRLF, a multi-byte line and an offset at EOF.
  The CLI's `file:line:col` printing in `main.rs` uses it instead of its own
  arithmetic (grep `line` in `report_diagnostics`).
- The 19 `ignore`d doc examples on `Span` either run (`Span::new` is public,
  so most can) or are deleted; none stays `ignore`.
- NOTICE.md: the oxc attribution line now names `usfm_span/src/span.rs`.

Done when the workspace builds, `cargo test -p usfm_span` runs the doc tests,
the gate is green, and `docs/benchmarks.md` numbers are unaffected (the move is
type-level only; no bench rerun needed, say so in the answer).

## Answer

Landed in `fcb4b64` (PR #14, 2026-09-19). `usfm_span` is the leaf crate: `span.rs`
moved by `git mv` with its oxc leftovers deleted, and `LineIndex` (line
starts recorded once, binary search per lookup, columns in chars, CRLF is one
break, offsets clamp and never panic inside a multi-byte character) with an
oracle test against the old per-call arithmetic. `usfm_ast::span` re-exports
it so no caller changed; `diagnostics::line_col` delegates to it and the CLI
builds one index per file. All 19 `ignore`d doc examples now run (21 doc
tests with the two on `LineIndex`); no other ignored doc test exists in the
workspace. Type-level move, no bench rerun.
