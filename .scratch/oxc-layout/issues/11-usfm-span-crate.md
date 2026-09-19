# 11. `usfm_span`: the first leaf crate

Status: ready-for-agent
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
