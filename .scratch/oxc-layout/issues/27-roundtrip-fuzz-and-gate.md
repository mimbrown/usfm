# 27. Round trip as an invariant: fuzz target and conformance step

Status: ready-for-agent
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
