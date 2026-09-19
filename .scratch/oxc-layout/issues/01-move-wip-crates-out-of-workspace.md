# 01. Move `usfm_language_server` and `data_layer` out of the workspace

Status: claimed
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
