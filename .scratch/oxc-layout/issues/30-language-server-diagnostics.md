# 30. `apps/usfm_language_server`: rebuilt on `usfm::parse`, diagnostics first

Status: resolved
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

## Answer

Landed in `c258619` (PR #38, 2026-09-20). `apps/usfm_language_server` (binary
`usfm-language-server`) on `tower-lsp-server` 0.23 and `tokio` (workspace
dependencies again, tokio with the four features an stdio loop needs), on
the facade with `default-features = false`. Four modules: `main.rs`
(`Backend`, `initialize` advertising full sync and UTF-16 positions only,
`did_open`/`did_change`/`did_close`, `publishDiagnostics` after every
open/change and an empty list on close), `convert.rs` (`Span` -> `Range`,
`Diagnostic` -> LSP diagnostic with the kebab-case `Code` as `code`,
`source = "usfm"`, severity one to one), `documents.rs` (a `HashMap<Uri,
String>` behind a `tokio::sync::RwLock`; the text is read back from the
store so what is published is what the server believes the file to be),
`stylesheet.rs` (`initializationOptions.stylesheet`, absolute or relative
to the workspace root, else a `custom.sty` beside the file, else the
default; a found sheet *extends* the default as the machine-py test does;
each path read once, failures included, so a broken sheet warns once via
`window/showMessage`). Positions: `LineIndex::line_col_utf16` in
`usfm_span`, sharing `line_col`'s clamping and boundary snapping, tested
with `é`, an emoji and `中`. No debounce: a book parses in milliseconds.

Tests: 11 unit tests in the crate, 2 in `usfm_span`, and
`tests/lsp.rs::diagnostics_are_published_on_open_and_cleared_on_a_clean_change`
speaking hand-framed JSON-RPC to the built binary (a 30 s reader timeout).
The ticket's `\zzz` is `unknown-custom-marker` (a `z` marker is the user
extension space), so the test uses `\qqq` -> `unknown-marker`.

`vscode/`: no client-side regex diagnostics existed, but the
`LanguageClient` construction was commented out, so the extension shipped
no server at all; restored. `server:build:*` build `-p
usfm_language_server`, `extension.ts` spawns `usfm-language-server` and
sends `{stylesheet, workspaces}` as initialization options, a
`usfm.stylesheet` setting, `.vscode/launch.json` fixed (it also pointed
the preview at the pre-ticket-15 `usfm_parser` binary). `npm run compile`
bundles; the manual check is written up in `vscode/README.md`. Gate exit
0; `scripts/miri.sh` leaves the server out and says why. The lexicon
machinery in the extension is untouched (ticket 33).
