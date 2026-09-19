# 17. The `usfm` facade and the move to `crates/`, `apps/`, `tasks/`

Status: resolved
Milestone: M3
Blocked by: 16

The last M3 step: one crate to depend on, and the directory layout from the
ADR.

- New crate `usfm` re-exporting `usfm_span`, `usfm_style`, `usfm_ast`,
  `usfm_diagnostics`, `usfm_parser`, and the outputs behind features (`usx`,
  `html`, `json`, `pipeline`; default = all), with one `usfm::parse(&str) ->
  ParseResult` and `usfm::parse_with(&str, &Arc<StyleSheet>)`. `apps/usfm_cli`
  and `tasks/*` depend on `usfm`, not on the pieces.
- Move directories: `usfm_*` crates to `crates/`, `tests/` to
  `tasks/conformance` (package name stays `usfm_tests` so `cargo run -p
  usfm_tests` keeps working; its `build.rs` path to `tcdocs` changes),
  `tests/tcdocs-baseline.txt` and `tests/tcdocs-patches/` with it. `git mv`
  only; then fix paths in `Cargo.toml` files, `.github/workflows/ci.yml`,
  `scripts/*.sh`, `CLAUDE.md`, `docs/plans/hardening.md`, `docs/benchmarks.md`,
  `tasks/benchmark/corpus/README.md`, `NOTICE.md`, `tasks/fuzz`.
- Already done by ticket 15: the re-exports left in `usfm_parser` by tickets
  13–14 are gone and `usfm_parser` depends only on `usfm_ast`,
  `usfm_diagnostics` and `usfm_style` (span through ast). Re-check after the
  move and record it as the M3 exit.
- M3 exit check, recorded in the spec: `usfm_parser` has no binary and the
  dependency list above; `main.rs` logic has tests (ticket 15); benchmarks
  within 3% of M2's "After ticket 05" numbers, all five groups, three runs,
  interleaved with a build of `84ee98e` as `docs/benchmarks.md` prescribes.

Done when the gate is green from the new paths, tcdocs 215 / 0 / 44, and the
spec records M3 closed.

## Answer

Landed via PR #20 (2026-09-19). `crates/usfm` re-exports `span`, `style`,
`ast`, `diagnostics`, `parser` unconditionally and `usx`, `html`, `json`,
`pipeline` behind features (all on by default; `benchmarking`/`testing`
forward to the parser), lifts the common types to its root, and adds
`usfm::parse` / `parse_with`. `apps/` and `tasks/` depend on it; the crates
under `crates/` depend on each other. Everything moved by `git mv`:
`crates/usfm_*`, `tasks/conformance` (still the `usfm_tests` package, with its
fixtures, patches and baseline). M3 exit recorded in the spec: parser has no
binary and depends only on ast, diagnostics and style; `main.rs` logic has
tests; benchmarks interleaved against `bfaa57f` with codegen-units pinned are
within 3% on every group (lex +2.8%, parse −1.3%, parse_usx −0.4%,
parse_html +4.3%, reference_index +1.3%). The orchestrator reran the gate,
the fuzz crate's check and clippy, `seed.sh` (0 written) and the facade's
`--no-default-features` build after a container restart interrupted the
subagent's final run.
