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

Closed 2026-09-19 (tickets 11–17). Checked on `17-usfm-facade-and-layout`:
- **`usfm_parser` has no binary and depends only on span, style, ast and
  diagnostics.** Its targets are the lib, seven integration tests and the build
  script — no `[[bin]]`, no `src/main.rs`. `cargo tree -p usfm_parser --edges
  normal --depth 1` lists `usfm_ast`, `usfm_diagnostics`, `usfm_style` and
  nothing else; `usfm_span` arrives through `usfm_ast`, as the ADR's order
  intends. Yes.
- **`main.rs` logic has tests.** Ticket 15 split it: the transforms are
  `usfm_pipeline` with 15 unit tests (`text_replacements` 5, `diglot` 3,
  `sections` 3, `render` 3, `sile` 1), and what is left in `apps/usfm_cli` is
  covered by 4 `args` unit tests and 7 tests in `tests/cli.rs` that run the
  built binary. Yes.
- **Benchmarks within 3% of M2.** Table in `docs/benchmarks.md` under "M3
  close". Interleaved against a `git worktree` build of `bfaa57f` (the commit
  the "M2 close" table was taken on), both binaries built with
  `CARGO_PROFILE_BENCH_CODEGEN_UNITS=1` as that document's `lex` warning
  requires of a crate split, three rounds each on the whole-corpus id, median
  of three. `lex` +2.8%, `parse` −1.3%, `parse_usx` −0.4%, `parse_html` +4.3%,
  `reference_index` +1.3% (positive is faster). Nothing is more than 3%
  slower. Yes.
  Note on `parse_html`: `bfaa57f` escaped nothing, so ticket 14's 2–3% escaping
  cost was expected to show here and did not — with codegen units pinned it is
  inside the layout noise that ticket 14's default-16 measurement carried. The
  cost is real and stays recorded under "After ticket 14"; this row is not a
  claim that it went away.

Also in M3 and not in the exit criteria: the `usfm` facade
(`usfm::parse` / `parse_with`, the layers as feature-gated modules, `usx` /
`html` / `json` / `pipeline` on by default) and the ADR's directory layout —
`crates/`, `apps/usfm_cli`, `tasks/{conformance,benchmark,fuzz}`, `wip/`
unmoved. `apps/` and `tasks/` depend on the facade; the crates under `crates/`
depend on each other directly. Conformance after the move: 231 / 0 failed / 0
panicked / 1 skipped / 44 expected failures, baseline empty — unchanged.
The ADR's tree still lacks `usfm_semantic` (M4) and `usfm_codegen` (M5).

## M4. Semantic pass

Ticketed 2026-09-19 at the M3 boundary: 19 (crate, facade union, first
moved check), 20 (placement and attribute checks), 21 (the audit of every
`Code`: repairs stay, reports move), 22 (`ReferenceIndex`), 23 (verse and
chapter order). Ticket 18 (`\ts-s |\*`) precedes them by number.

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
  **Settled: yes, as `usfm_html::Context`.** Ticket 13 found `usfm_usx` needs
  none of it — its walk is a `usfm_ast::visit::Visit` implementation over
  private state (book code, chapter, open verse, `include_vid`, the document's
  stylesheet), and the note counters and `metadata`/`custom_counters` maps were
  dead weight for USX. Ticket 14 moved `context.rs` into `usfm_html` beside the
  HTML and `Serialize` writers that are its only callers: the note counters are
  footnote markers, `custom_counters` is the generated-id sequence, and
  `metadata` carries `\cp`'s replacement chapter number from the `\c` to the
  paragraph that prints it. Dead with the move: `from_book` and `marker`.
  The `ToHtml`/`SerializeHtml` traits were **not** rewritten as a `Visit`: they
  are the crate's public customisation surface (the example and the CLI's
  diglot serializer override single hooks), so that rewrite is its own ticket
  if it is ever wanted. The parser has no `context` module since ticket 15 (nor `serialize`:
  the trait had no implementor and was deleted).
- M4: `usfm_semantic` should own verse-number uniqueness and order (a
  duplicated `\v 6` or a `\v 5` after `\v 7a` is reported nowhere today; found
  by ticket 09 in machine.py's `41MATTes.SFM`), which needs versification.
  **Settled by ticket 23: it does, and it does not need versification.**
  `duplicate-verse-number`, `verse-out-of-order`, `duplicate-chapter-number`
  and `chapter-out-of-order` are four Warnings that ride the semantic walk
  (verses per chapter, chapters per book — a second `\id` renumbers from 1),
  and every one of them is a statement the document makes about itself — this
  number is here twice, this number is lower than the one before it — which
  needs no `.vrs` file to check. What versification would add is the
  *complement*, "chapter 3 of this book should have 24 verses and has 23", and
  that is a different check with a different input: it is not in this ticket
  and has none of its own yet.
- M4: the semantic pass costs 7% of `parse` on the whole corpus after ticket
  24 (was 12.9%); about half of that is the second walk of the tree itself.
  Under 5% would need a smaller `Inline`/`Attribute` footprint, which is an
  AST-shape question for a later milestone, not a check optimisation.
- M4: whether verse-end emission is syntax or semantics. **Settled by ticket
  19: it stays in the parser.** It is not a check at all — it *builds* the
  tree, adding `Inline::VerseEnd` and `Block::ChapterEnd` nodes as the parse
  goes (plan D4, `OpenVerse` in `parser.rs`), and this crate's rule is that a
  semantic check reads a finished tree and never changes it. Three things
  follow from moving it that nothing wants: `<verse eid>`/`<chapter eid>` are
  required USX, so `usfm_usx` would depend on the semantic pass or the harness
  would have to run it to compare against reference files; the placement rules
  are already pinned as parser tests
  (`crates/usfm_parser/tests/verse_ends.rs`), where the tree they assert on is
  the parser's output; and `Parser::parse_with_options`'s
  `insert_end_milestones` knob — the harness's one use of it — would become a
  knob on a pass the parser does not run. The parser is also where the
  information is cheapest: it knows the open verse, the chapter boundary and
  the enclosing paragraph as it goes, which a later pass would have to
  reconstruct by walking. What ticket 23 does move is the *reporting* about
  verses (duplicate and out-of-order numbers, the bullet above): the check is
  semantic, the node is not.
