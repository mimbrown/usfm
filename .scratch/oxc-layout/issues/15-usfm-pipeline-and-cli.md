# 15. `usfm_pipeline` and `apps/usfm_cli`: the binary leaves the parser

Status: resolved
Milestone: M3
Blocked by: 14

`usfm_parser/src/main.rs` (903 lines) holds a hand-rolled argument parser,
watch mode, diglot weaving, document sectioning, a "prompt" output and the
`ParseAndTransform` driver; `text_replacements.rs` (292 lines) is the
regex-driven text replacement pass. M3's exit says `usfm_parser` has no binary
and `main.rs` logic has tests.

- New crate `usfm_pipeline` depending on the parser, `usfm_usx`, `usfm_html`,
  `regex`, `regex-automata`: `text_replacements` (with its tests), the diglot
  weaver (`DocumentWeaver`, `serialize_html_diglot`), `document_sections` /
  `IterSections`, and the prompt format, each as a function over `Document`s
  with a unit test on a small input. `todo!()`/`unimplemented!()` arms in the
  driver become `Err` with a message.
- Two things ticket 14 found in `main.rs`: `DocumentSectionHtmlSerializer`
  (diglot) writes text sections into HTML raw, bypassing the writer's
  escaping; route it through `usfm_html::write_escaped` when it moves. And
  `usfm_html::serialize::Serialize` has no implementor anywhere (the SILE
  output goes through USX since ticket 06): delete the trait and the
  `to_sile_string` path's dead half, or give `sile` a real implementor if one
  is cheap; say which.
- New binary crate `apps/usfm_cli` (package `usfm_cli`, binary `usfm`) on
  `clap` (derive): `usfm parse <files>` with `--format usx|html|sile|prompt`,
  `--stylesheet`, `--replace <file>` (repeatable), `--diglot …`, `--watch`,
  `--strict` (exit non-zero on an error diagnostic, as today), a new
  `--deny-warnings`, and `--diagnostics text|json` using
  `usfm_diagnostics::Diagnostic::render` / `to_json_line`. `notify` moves
  here from the parser's dependencies.
- CLI tests: `apps/usfm_cli/tests/cli.rs` runs the binary (`std::process::
  Command` on `env!("CARGO_BIN_EXE_usfm")`) over two tcdocs inputs and checks
  exit codes for `--strict`, the JSON diagnostics shape, and that USX output
  equals `usfm_usx::to_usx_string`. No new test dependency needed.
- Remove `[[bin]]`/`main.rs` from `usfm_parser`; `cargo run -p usfm_tests --
  --show` and the corpus README's CLI instructions point at the new binary.
  `vscode/` scripts that mention `cargo build -p usfm_parser` change to the
  CLI (they never built the server).
- `CLAUDE.md`: update the CLI paragraphs ("The CLI writes USX through
  `usx.rs`…") and the project structure.

Done when `usfm_parser` has no binary, the CLI tests pass, and the gate is
green.

## Answer

Landed in `7e9d531` (PR #18, 2026-09-19). `usfm_pipeline` holds `text_replacements`,
`sections`, `diglot` (its raw text writes now go through
`usfm_html::write_escaped`), `sile` and `render` (`OutputFormat`,
`RenderError`; the `todo!()`/`unimplemented!()` arms are errors with
messages), each with unit tests; `usfm_parser` is only its dev-dependency.
`apps/usfm_cli` (binary `usfm`, clap 4) has `parse <FILES>... --format
usx|html|sile|prompt --stylesheet --output --replace --diglot
--diglot-stylesheet --diglot-replace --watch --strict --deny-warnings
--diagnostics text|json`; `-ds`/`-dr` are long-only now. Six CLI tests run the
binary. `usfm_parser` has no binary and depends only on `usfm_ast`,
`usfm_diagnostics` and `usfm_style` (the M3 exit criterion, reached early);
its `usx`/`xml_document`/`serialize_html`/`serialize`/`context`/
`text_replacements` modules are gone, and `usfm_html::Serialize` (no
implementor) is deleted. `parse`, `parse_usx`, `parse_html` interleaved
against `5d376e8`: −0.8%, 0.0%, +0.3%. The VS Code extension's build scripts
and preview service point at the new binary and flags.

Left as found: the prompt weave writes section text raw (it is prompt text,
not markup); the diagnostic label is `input`/`diglot`, not a filename, since
inputs are concatenated before parsing.
