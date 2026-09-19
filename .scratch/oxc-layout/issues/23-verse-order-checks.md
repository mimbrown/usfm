# 23. Verse and chapter order checks

Status: ready-for-agent
Milestone: M4
Blocked by: 22

Found by ticket 09 in machine.py's `41MATTes.SFM`: a duplicated `\v 6` and a
`\v 5` after `\v 7a` are reported nowhere. The spec's open list has it.

- New codes in `usfm_diagnostics`: `duplicate-verse-number` (Warning: the
  same number twice in one chapter, segments `4a`/`4b` are distinct and a
  range `3-5` covers 3, 4, 5), `verse-out-of-order` (Warning: a number lower
  than the previous verse's end in the same chapter), `duplicate-chapter-number`
  and `chapter-out-of-order` (Warning). Ranges and segments follow
  `NumberList`. Versification (`.vrs`) is out of scope: no "missing verse"
  check here.
- Implemented in `usfm_semantic` over `ReferenceIndex`; tests in
  `crates/usfm_semantic/tests/checks.rs`; tcdocs must stay 231 / 0 / 44 (a
  reference file with a genuine duplicate would show up as an unexpected
  Warning only in a `fail` case, so check the numbers; report any that flips).
- The corpus (`tasks/benchmark/corpus`) must stay at zero errors; count the
  new warnings over it and record them in its README.

Done when the gate is green and `41MATTes.SFM`'s snapshot shows the two new
codes.
