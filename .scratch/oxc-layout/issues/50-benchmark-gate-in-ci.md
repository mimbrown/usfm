# 50. A benchmark regression gate in CI, by instruction count

Status: needs-triage
Milestone: after M7

The hardening plan's last testing box (`docs/plans/hardening.md`,
"Benchmarks") asks CI to fail on a 20% regression. The benches exist
(`tasks/benchmark`); what is missing is the gate. Today regressions are
caught by hand, interleaving against the previous commit at each milestone
boundary ("Reading a regression" in `docs/benchmarks.md`).

Wall-clock time is the wrong signal for a gate. On this VM the spread
between back-to-back runs is 1.4–4.3% per group and up to 12% for
`reference_index`; shared CI runners are likely no quieter. The real
regressions so far were smaller than that: M5's `parse/whole-corpus` was
−3.9%, and ticket 37 attributed it with callgrind because each suspect sat
under the noise floor. A 20% time threshold would have missed it, and a
tight one would fail on noise.

Logged on 2026-09-27 as an option for later (Michael: "log it as an option
for later on"), not scheduled.

- Count instructions rather than time: callgrind (or `iai-callgrind`) over a
  few representative inputs — `parse`, `parse_semantic` and `read_usx` over
  one WEB book and one aligned book — which is deterministic to well under
  1%, so a 5% threshold is meaningful.
- Decide where the baseline lives (a committed file, like
  `tcdocs-baseline.txt`, with the same regression-*and*-stale semantics) and
  how it is updated on purpose (`--write-…`, in the commit that explains why).
- Mind CI time: valgrind is slow; keep the inputs small enough that the step
  stays within the gate's current budget.

Done when CI fails on an instruction-count regression above the threshold,
passes on an unchanged tree, and the box in the hardening plan is ticked.
