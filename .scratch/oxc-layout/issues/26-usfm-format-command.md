# 26. `usfm format` and `--format usfm`

Status: resolved
Milestone: M5
Blocked by: 25

The spec's second M5 exit criterion: a `format` subcommand in the CLI.

- `usfm format <FILES>...`: parse (facade, so semantic diagnostics print
  too), write with `usfm_codegen`, and either print to stdout, `--write`
  in place (one file at a time; refuse when the parse has an Error
  diagnostic unless `--force`, since a repaired tree is not the author's
  text), or `--check` (exit 1 when the output differs from the input, print
  the file names; for CI). `--diagnostics text|json` as `parse` has.
- `usfm parse --format usfm` writes the same thing (useful for a document
  assembled from several files).
- Tests in `apps/usfm_cli/tests/cli.rs`: `format` on a tcdocs input equals
  `to_usfm_string`; `--check` exit codes on a clean and a dirty file;
  `--write` refuses on an Error without `--force` and rewrites with it.
- `usfm_pipeline::OutputFormat::Usfm` for the dispatch; `usfm` facade gets a
  `codegen` feature like the other outputs (on by default; the pipeline pulls
  it in).
- CLAUDE.md's CLI paragraph and the corpus README's tool list.

Done when the gate is green and `usfm format --check` passes on the whole
benchmark corpus (which is the parser's own output shape by construction:
if it does not, the corpus converter and codegen disagree on a whitespace
rule; fix codegen, never the corpus).

## Answer

Landed via PR #31 (2026-09-20). `usfm format <FILES>...` (per file, through
the facade; stdout by default, `--write` in place refusing a file with an
Error diagnostic unless `--force`, `--check` exiting 1 with `would reformat
<path>`; `--write`/`--check` conflict), `usfm parse --format usfm`, and
`OutputFormat::Usfm` in the pipeline (the facade's `pipeline` feature pulls
`codegen`). Six CLI tests and two unit tests.

The corpus `--check` criterion as written does not hold: 78 of 86 books
differ, all because the writer closes note-internal styles explicitly
(`\ft text\ft*\f*`) and puts `\cp` on the `\c` line; both are legal
spellings ticket 25 chose, the corpus round-trips, and `--check` passes on
all 86 after one `format --write` pass (idempotence). Making the writer
idiomatic is ticket 34, not a whitespace fix; the corpus was not touched.
