# 48. `usfm::parse_usx` and `usfm parse --from usx`

Status: resolved
Milestone: M7
Blocked by: 45

- The facade: `usfm::parse_usx(&str)` and `parse_usx_with(&str,
  &Arc<StyleSheet>)`, returning the reader's diagnostics plus
  `usfm_semantic`'s, as `parse` / `parse_with` do for USFM, behind the `usx`
  feature.
- The CLI: `usfm parse --from usfm|usx` (default `usfm`; a `.usx` or `.xml`
  extension does not switch it silently), so `usfm parse book.usx --format
  usfm` converts and `--format usx` normalises. `usfm format` stays
  USFM-only. Several `.usx` files concatenate the way several `.usfm` files
  do today, or the ticket says why they cannot.
- Tests in `apps/usfm_cli/tests/cli.rs`: one conversion each way, the
  diagnostics of a USX file with a known problem, and the usage error for
  `--from usx` with `format`.

The language server stays USFM-only: a `.usx` file is not a document it
opens.

Done when the gate is green and CLAUDE.md's CLI description names the flag.

## Answer

Landed 2026-09-27 (PR below).

- **Facade:** `usfm::parse_usx` / `parse_usx_with` behind `usx`, the
  reader's diagnostics plus `usfm_semantic`'s. The merge is one private
  `with_semantic` that `parse_with_options` now shares, so both paths order
  findings the same way. No `parse_usx_with_options`: nothing needs the ends
  switched off.
- **CLI:** `usfm parse --from usfm|usx` (default `usfm`; an extension never
  switches it — a `.usx` read without the flag is USFM and says
  `missing-id`). It applies to `--diglot` inputs too. Several USX files are
  combined **as trees, not text**: two USX files are not one XML document, so
  each is read on its own, reports under its own path with positions in its
  own file, and the documents are joined block by block, with derived style
  ids remapped onto the combined sheet (a style one file derived may share
  an id with a different one another file derived). Thresholds apply after
  every file has reported. `usfm format` has no `--from`; the usage error is
  clap's.
- Tests (`apps/usfm_cli/tests/cli.rs`): USX -> USFM equals `usfm_codegen` of
  `parse_usx`; USFM -> USX -> `--from usx` back to byte-identical USX;
  diagnostics of Tes `MAT.usx` in text and JSON; the extension rule; several
  files; `format --from`.
- Left as they are: with several USX files a combined document's `span` is
  `SPAN` and JSON spans are offsets into each node's own file; in watch
  mode an unreadable USX file reads as empty and reports
  `usx-not-well-formed`.
