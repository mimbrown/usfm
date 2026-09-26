# 48. `usfm::parse_usx` and `usfm parse --from usx`

Status: ready-for-agent
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
