# 30. `apps/usfm_language_server`: rebuilt on `usfm::parse`, diagnostics first

Status: claimed
Milestone: M6
Blocked by: 37

The parked server in `wip/usfm_language_server` (tower-lsp-server, 8 compile
errors, its own regex-based checks and a SQLite lexicon) is replaced, not
fixed: M6 rebuilds it in `apps/` on `ParseResult`, `usfm_semantic` and
`usfm_codegen` (spec). This ticket is the skeleton with the one feature that
pays for it, diagnostics.

- New workspace member `apps/usfm_language_server` (binary
  `usfm-language-server`) on `tower-lsp-server` and `tokio` (workspace
  dependencies again) depending on the `usfm` facade: `initialize` (full text
  sync, the capabilities this ticket implements only), `did_open`/`did_change`
  /`did_close` keeping the text per document, and `publishDiagnostics` from
  `usfm::parse_with` (the union) after every change, debounced only if a
  measurement says it matters (the whole corpus parses at 50 MiB/s: a book is
  milliseconds). Positions through `usfm_span::LineIndex` (LSP wants UTF-16
  columns: add `LineIndex::line_col_utf16` or convert in the server; test
  with a line containing `é` and an emoji), `Diagnostic.code` = the kebab-case
  `Code` name, `source` = "usfm", severity mapped one to one, `tags` none.
- A project stylesheet: `initializationOptions.stylesheet` (a path) or
  `custom.sty` next to the file when present (Paratext projects have one),
  parsed once with `StyleSheet::from_str` and extended over the default sheet
  as ticket 09's test does; a failure to read it is a `window/showMessage`
  warning, never a crash.
- Tests: unit tests for the position conversion and the diagnostic mapping;
  one integration test that spawns the binary over stdio, speaks a minimal
  JSON-RPC by hand (no LSP client dependency: `initialize`, `initialized`,
  `didOpen` with a document carrying an `unknown-marker`, read the
  `publishDiagnostics` notification, `shutdown`, `exit`) and asserts the code
  and range.
- `vscode/`: `server:build:*` build `-p usfm_language_server`,
  `extension.ts` and `.vscode/launch.json` point at the new binary name; the
  extension's own regex diagnostics, if any, are removed in favour of the
  server's (check `vscode/client`).
- CLAUDE.md: the Language Server stream is no longer parked; Project
  Structure.

Done when the gate is green (the server's tests run in it) and the VS Code
extension shows the server's diagnostics on a file with an unknown marker
(describe the manual check; it cannot run in CI).
