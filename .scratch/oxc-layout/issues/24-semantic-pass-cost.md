# 24. The semantic pass costs 11% on top of the parse; make it near-free

Status: ready-for-agent
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
