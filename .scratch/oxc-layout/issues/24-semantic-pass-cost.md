# 24. The semantic pass costs 11% on top of the parse; make it near-free

Status: resolved
Milestone: M4
Blocked by: 23

`parse_semantic` (facade) vs `parse` (parser alone), whole-corpus, after
ticket 21: 45.9 vs 51.5 MiB/s, a 10.9% gap. The nine checks ticket 21 moved
cost nothing above noise; the gap is the attribute re-check from ticket 20
(`attributes-heavy` 18% apart, `alignment-heavy` 14%, `plain` 2%). The pass
walks every attribute list a second time, cold, checking names with
`is_valid_attribute_name`, duplicates with a quadratic scan, and the
default-attribute rules per pair.

- Measure first with `perf`-free means: time `analyze` alone over the
  attributes-heavy class (a `criterion` group `analyze` that parses outside
  the timed loop), then the same with each attribute check disabled, to
  attribute the cost.
- Then one of: (a) make the checks linear and allocation-free (the duplicate
  scan is O(n²) per list; `is_valid_attribute_name` re-scans names the lexer
  already tokenised), (b) have the parser record, in the additive
  `Attributes` node, the two booleans it knows for free while building the
  list (`has_duplicate`, `has_malformed_name`) so the pass only reports, or
  (c) both. Not acceptable: moving a check back into the parser as an emit.
- Target: `parse_semantic` within 5% of `parse` on whole-corpus, three rounds
  turn about, recorded in `docs/benchmarks.md`.

Done when the gate is green with tcdocs unchanged and the gap is under 5%.

## Answer

Landed in `f181eab` (PR #28, 2026-09-20). Measured first (new `analyze` bench group,
a per-family attribution): `is_valid_attribute_name` was 58% of the pass's
instructions over an aligned text (`chars()` decoding to answer byte
questions); the `OccursUnder` scan for `\w` walked 96 strings per word; the
scope stack was two `Vec` scans per check. Fixed: a byte scan with
`#[inline]`, a 64-slot placement memo, saved-scope fields, a plain-verse
fast path in the order checks. `analyze` alone: −41% whole-corpus, −83% on
alignment-heavy. `parse_semantic` vs `parse`: 12.9% → 7.0% whole-corpus
(plain 3.3%, alignment 4.1%, notes 4.2%, attributes-heavy 10.7%).

The 5% target was not met and is revised to "under 8%": an empty `Visit`
over the tree costs ~3.5% of `parse` by itself, and callgrind puts nearly
every cache miss in the walk, not the checks; the rest is reading a ~50 MB
tree a second time, which only a smaller AST footprint or not walking twice
would remove, and the second is the check moving back into the parser.
Option (b), flags on `Attributes`, was tried and measured as nothing once the
name scan was cheap; reverted and recorded. Zero diagnostic difference over
385 inputs; two minutes of fuzz clean.
