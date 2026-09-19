# oxc-style layout

Decision: `docs/adr/0001-oxc-style-crate-layout.md`. This spec is the route; it
replaces Phase 4 and the Phase 0 leftovers in `docs/plans/hardening.md`.

Milestones are fixed and in order. Tickets are written for a milestone when the
one before it closes, because each one's findings shape the next. Milestones 1
and 2 are ticketed in `issues/`.

Invariant for every milestone: the unit and integration suites pass and the
conformance gate stays at tcdocs 215 / 0 failed plus the vendored
usfm-grammar root at 16 / 0 (ticket 08), empty baseline.

## M1. Workspace builds clean

Remove what blocks `cargo build --workspace` and gate lint.

Exit: `cargo build --workspace` and `cargo clippy --workspace --all-targets -- -D
warnings` pass locally and in CI, with no `--exclude usfm_language_server`.

Closed 2026-09-19 (tickets 01, 02; `fac106b`, `6c68d6e`). Checked on `main`:
`cargo build --workspace` passes; `cargo clippy --workspace --all-targets -- -D
warnings` passes locally on the pinned 1.98.0 toolchain and in CI through
`scripts/gate.sh`; no `--exclude` remains in the gate, CI or CLAUDE.md. tcdocs
215 / 0 / 44 expected failures, baseline empty. Found on the way: the tree is
not rustfmt-clean (58 hunks), left as a follow-up before M3.

## M2. Measured

Benchmarks and safety checks exist before anything moves.

Exit: `cargo bench` reports parse, parse + USX and parse + HTML throughput (MB/s)
on a committed corpus; the numbers are recorded in `docs/benchmarks.md`; Miri runs
the lexer and `string_parser` tests clean in CI; a fuzz target asserts no panic and
the span invariants, and has run 10 minutes without a finding.

Closed 2026-09-19 (tickets 03–06; `10cb9dd`, `991d902`, `84ee98e`, `bfaa57f`).
Checked on `main` at `bfaa57f`:
- `cargo bench -p usfm_benchmark` reports lex, parse, parse + USX, parse + HTML
  and reference_index throughput in MiB/s over `tasks/benchmark/corpus/` (WEB,
  12.8 MB, plus synthetic attribute and alignment classes). Yes.
- Recorded in `docs/benchmarks.md`: the M2 baseline, the ticket 05 before/after,
  and the "M2 close" table that M3 compares against. Yes.
- Miri runs the lexer and `string_parser` tests clean in CI: `scripts/miri.sh`
  is the gate's last step, CI installs nightly Miri. Yes. On the way, 26 of 27
  `unsafe` uses became safe code at no measured cost.
- A fuzz target asserts no panic and the span invariants (and USX
  well-formedness), and has run 10 minutes without a finding: `tasks/fuzz`,
  both targets, 10 minutes clean on `bfaa57f` after seven findings were fixed.
  Yes.
Follow-ups ticketed: 07 (`*` note caller), 08–09 (public fixtures), 10
(licence call, `ready-for-human`); the HTML writer's control-character hazard is
folded into ticket 14.

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

- M2: which corpus. Settled: WEB (public domain) via its USFX rendering, since
  ebible.org is blocked from the cloud environment; see
  `tasks/benchmark/corpus/README.md`.
- M3: whether `Context` (note numbering, counters) survives as HTML-only state.
- M4: whether verse-end emission is syntax or semantics. It is in the parser now
  (plan D4) and USX needs it; default is to leave it.
