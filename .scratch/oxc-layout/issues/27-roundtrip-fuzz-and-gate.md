# 27. Round trip as an invariant: fuzz target and conformance step

Status: resolved
Milestone: M5
Blocked by: 26

Ticket 25 proves the round trip on the `pass` corpus. The stronger property
is idempotence on any input: the tree of `parse(codegen(parse(x)))` equals
the tree of `parse(x)` ignoring spans, and its diagnostics are a subset (a
repair the parser made is not repeated because codegen wrote the repaired
tree).

- `tasks/fuzz/fuzz_targets/roundtrip.rs` (lossy UTF-8): parse, codegen,
  parse, assert tree equality ignoring spans and that the second parse has no
  Error diagnostic; ten minutes clean. Every finding is a codegen or parser
  bug with a test in `crates/usfm_codegen/tests/roundtrip.rs` or
  `recovery.rs` first, then a fix (ticket 06's rules).
- `tasks/conformance`: a `--roundtrip` flag on the runner that runs the
  property over every case of every root (pass and fail) and reports
  failures by name; `scripts/gate.sh` runs it after the baseline step, so the
  invariant is gated. Any `fail` case that cannot be idempotent (the parser
  drops content it cannot place, so the second parse differs) is listed in a
  `roundtrip-known.txt` beside the baseline with a reason per line, and the
  gate fails on a stale entry as the baseline does.
- Record the M5 exit in the spec at the boundary: the property test on the
  `pass` corpus (25), the `format` subcommand (26), and this invariant.

Done when the fuzz target has run ten minutes clean and the gate runs the
round trip over both roots.

## Answer

Landed in `eab58a3` (PR #32, 2026-09-20). The property is defined once, in
`tasks/conformance/src/roundtrip.rs` (`usfm_tests::roundtrip::check`): the
trees are equal ignoring spans, the second parse gains no diagnostic code the
first did not report, and writing the second tree gives the same bytes. It is
asserted in three places: `crates/usfm_codegen/tests/roundtrip.rs` (the
`pass` corpus, the 86 benchmark books, the seven machine.py fixtures), the
gate's new `cargo run -p usfm_tests -- --roundtrip
tasks/conformance/roundtrip-known.txt` step over all 275 conformance cases of
both roots (`roundtrip-known.txt` is empty and has the baseline's semantics),
and the `roundtrip` fuzz target (`usfm_fuzz::check_roundtrip`, restated there
because the fuzz crate depends only on the facade).

One change to the ticket's wording, measured rather than chosen: "the second
parse has no Error diagnostic" does not hold — 21 of the 275 cases keep one
(`missing-id`, `verse-outside-chapter`, `empty-book`, …), each an error about
the document rather than its spelling, which a faithful writer reports again.
"No code gained" is what is asserted, and the fixed-point check keeps it
honest.

Twenty bugs came out of the target: nineteen findings (four in `usfm_ast`,
fourteen in the parser, one in `usfm_codegen`; the table is in
`tasks/fuzz/README.md`) plus ticket 29. Tickets 28 and 29 are fixed here.
Nearly all are one shape — a marker the parser drops leaves a tree no USFM
spells — and the fix is that the parser now reads the absorbed spelling the
way the writer's output reads (a milestone after a `Para` goes inside it, a
`\cp` after `\c` is its published number, consecutive tables are one table,
whitespace across a dropped marker obeys rules 1 and 6).

**The ten-minutes-clean criterion is not met.** The twenty-first run, from
the pruned seeds, found `\esb\c\sh\*` at 35 101 execs: a `Block::Milestone`
after a `Sidebar`, a `Table` or a `\periph` line, which the line before takes
back on re-parse. It is a rule about which blocks may precede a block
milestone, so it is written up in `tasks/fuzz/findings/` and is ticket 35
rather than a repair here. The gate runs the round trip over both roots, so
the second "done" clause holds; the first is ticket 35's.
