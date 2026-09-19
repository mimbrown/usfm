# 04. Criterion benches

Status: ready-for-agent
Milestone: M2
Blocked by: 01, 03

`tasks/benchmark` crate with criterion: `lex`, `parse`, `parse + USX string`,
`parse + HTML string`, `reference_index`, per file class and whole corpus, reported
as throughput. Record a baseline table in `docs/benchmarks.md` with machine and
commit. Not gated in CI yet (noisy runners); run on demand.

Done when `cargo bench -p usfm_benchmark` runs and the baseline is committed.
