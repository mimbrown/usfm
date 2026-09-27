# 49. Fuzz, Miri and a benchmark for the reader; M7's close

Status: resolved
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

## Answer

Landed 2026-09-27 (PR below); **M7 is closed** (spec, "M7. USX reader").

- **Fuzz:** `read_usx` and `usx_roundtrip` in `tasks/fuzz`, seeded with every
  reference of both roots and machine.py's USX (NIV excluded). Seven bugs,
  each a failing test first: an `<para style="esbe">` read silently (now
  `unmatched-sidebar-end`); a verse starting inside `\w` in a non-verse
  paragraph got its end before its start, and a second waiting verse end
  overwrote the first (both fixed in the parser *and* the reader, the waiting
  slot now a stack); the reader refused `\thc0` / `\tc3-2`; the USX writer
  left a tab raw in attribute values, which xml-rs does not escape; the
  nesting look-ahead took `\+xt*` for an unplussed `\xt`'s closer (the
  existing `roundtrip` target fails on it too); `\usfm` swallowed a
  milestone or verse after it (now read like `\id`). Then ten clean minutes
  each: `read_usx` 442 912 runs, `usx_roundtrip` 24 233, and `roundtrip`
  rerun 69 158. Table in `tasks/fuzz/README.md`.
- **Miri:** 8 of `reader`'s 25 tests (the byte-handling ones), 31 s; the
  rest would add 140 s and 1 John does not finish in 15 minutes.
  `scripts/miri.sh` is about 5 minutes now; `usx_text` alone drifted from
  ~15 s to 46 s, as it does at the M6 close too, so the drift is the VM or
  toolchain, not the code.
- **Benchmark:** `read_usx` reads 84.1 MiB/s of USX (1.49x the bytes), so a
  book from USX costs about 1.2x its parse. The M7 boundary rerun against
  the M6 close is within noise for every group; no regression ticket.
