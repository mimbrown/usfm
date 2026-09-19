# Parked crates

`usfm_language_server` and `data_layer` live outside the cargo workspace
(`exclude = ["wip"]`) and **do not build**: their `workspace = true`
dependencies have no workspace to read, and the server still has its original
compile errors.

They are kept for reference until M6 rebuilds the language server in `apps/` on
`ParseResult`, `usfm_semantic` and `usfm_codegen`. Nothing in the repo builds
them; the VS Code extension's `server:build:*` scripts build `usfm_cli` (`apps/usfm_cli`).
