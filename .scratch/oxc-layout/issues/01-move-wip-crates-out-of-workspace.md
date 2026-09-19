# 01. Move `usfm_language_server` and `data_layer` out of the workspace

Status: resolved
Milestone: M1

`usfm_language_server` has 8 compile errors and shares nothing with the parser;
`data_layer` exists only for its lexicon feature. Phase 5 rebuilds the server.

- `git mv` both to `wip/`, remove them from `[workspace] members`, add
  `exclude = ["wip"]`.
- Drop workspace dependencies nothing else uses (`rusqlite`, `rusqlite_regex`,
  `ropey`, `tokio`, `tower-lsp-server`, `serde*` if unused).
- Remove `--exclude usfm_language_server` from CI and CLAUDE.md.
- Check the VS Code extension's build does not reference the old path.

Done when `cargo build --workspace` and `cargo test --workspace` pass.

## Answer

Landed in `fac106b` (PR #4, 2026-09-19). Both crates are under `wip/` with
`exclude = ["wip"]`; `ropey`, `rusqlite`, `rusqlite_regex`, `tokio`,
`tower-lsp-server` and `unic` left the workspace dependencies (`serde*` stayed:
`usfm_tests` uses them). The gate, CI and CLAUDE.md have no exclude.

Follow-ups, not done here:
- The VS Code extension (`vscode/`) looks for a `target/*/usfm_language_server`
  binary that nothing builds; its `server:build:*` scripts build `usfm_parser`.
  It was already broken and is M6's to fix.
