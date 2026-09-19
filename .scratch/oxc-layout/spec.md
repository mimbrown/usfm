# oxc-style layout

Decision: `docs/adr/0001-oxc-style-crate-layout.md`. This spec is the route; it
replaces Phase 4 and the Phase 0 leftovers in `docs/plans/hardening.md`.

Milestones are fixed and in order. Tickets are written for a milestone when the
one before it closes, because each one's findings shape the next. Milestones 1
and 2 are ticketed in `issues/`.

Invariant for every milestone: the unit and integration suites pass and the
tcdocs gate stays at 215 / 0 failed / empty baseline.

## M1. Workspace builds clean

Remove what blocks `cargo build --workspace` and gate lint.

Exit: `cargo build --workspace` and `cargo clippy --workspace --all-targets -- -D
warnings` pass locally and in CI, with no `--exclude usfm_language_server`.

## M2. Measured

Benchmarks and safety checks exist before anything moves.

Exit: `cargo bench` reports parse, parse + USX and parse + HTML throughput (MB/s)
on a committed corpus; the numbers are recorded in `docs/benchmarks.md`; Miri runs
the lexer and `string_parser` tests clean in CI; a fuzz target asserts no panic and
the span invariants, and has run 10 minutes without a finding.

## M3. Split into the ADR's layout

Order: `usfm_span` and `usfm_diagnostics` out first (leaf crates), then
`usfm_usx` and `usfm_html` as `Visit`/`Fold` with no shared `Context`, then
`usfm_pipeline` and `apps/usfm_cli` (clap, `--strict`, `--deny-warnings`, JSON
diagnostics), then `usfm_json`, then the `usfm` facade, then the move to `crates/`,
`apps/`, `tasks/`.

Exit: `usfm_parser` has no binary and depends only on span, style, ast and
diagnostics; `main.rs` logic has tests; benchmarks within 3% of M2.

## M4. Semantic pass

Move non-syntactic checks (`OccursUnder` placement, table columns, leading zeros,
attribute validation that needs the stylesheet) and `ReferenceIndex` into
`usfm_semantic`.

Exit: the parser reports only what it needs to recover; every moved `Code` still
has its test; the facade's `parse()` returns the union, so tcdocs is unchanged.

## M5. Codegen and round trip

`usfm_codegen` writes USFM from the AST.

Exit: property test parse -> codegen -> parse yields an equal tree on the tcdocs
`pass` corpus; a `format` subcommand in the CLI.

## M6. Language server (hardening Phase 5)

Rebuilt in `apps/` on `ParseResult`, `usfm_semantic` and `usfm_codegen`
(formatting).

## Open, to settle when reached

- M2: which corpus. Needs a full Bible with a licence that allows committing it
  (WEB is public domain). tcdocs files are too small to benchmark.
- M3: whether `Context` (note numbering, counters) survives as HTML-only state.
- M4: whether verse-end emission is syntax or semantics. It is in the parser now
  (plan D4) and USX needs it; default is to leave it.
