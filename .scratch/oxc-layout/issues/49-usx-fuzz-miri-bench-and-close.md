# 49. Fuzz, Miri and a benchmark for the reader; M7's close

Status: ready-for-agent
Milestone: M7
Blocked by: 46, 47, 48

- `tasks/fuzz`: `read_usx` (arbitrary bytes: no panic, the span invariants,
  and a read tree writes well-formed USX) and `usx_roundtrip` (arbitrary
  USFM: parse, write USX, read, and the tree equals the parse's modulo the
  list ticket 45 recorded; then write USFM and parse again as `roundtrip`
  does). Seeds: every reference of both roots and the vendored machine.py
  USX. Each target ten minutes clean from the seeds; every finding a test
  first, as ticket 27's were.
- `scripts/miri.sh`: `usfm_usx`'s `reader` tests (`roxmltree` has no
  `unsafe`, so this is cheap); keep the script's total in CLAUDE.md honest.
- `tasks/benchmark`: a `read_usx` group over the benchmark corpus written as
  USX once at setup, its number in `docs/benchmarks.md` beside `parse`.
- M7's close in the spec: each exit criterion checked on `main` with the
  commit, as M5's and M6's were, plus the boundary benchmark rerun and the
  invariant.

Done when the spec records M7 closed.
