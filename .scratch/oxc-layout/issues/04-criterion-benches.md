# 04. Criterion benches

Status: resolved
Milestone: M2
Blocked by: 01, 03

`tasks/benchmark` crate with criterion: `lex`, `parse`, `parse + USX string`,
`parse + HTML string`, `reference_index`, per file class and whole corpus, reported
as throughput. Record a baseline table in `docs/benchmarks.md` with machine and
commit. Not gated in CI yet (noisy runners); run on demand.

Done when `cargo bench -p usfm_benchmark` runs and the baseline is committed.

## Answer

Landed via PR #7 (2026-09-19). `tasks/benchmark` (`usfm_benchmark`, in the
workspace) with `benches/corpus.rs`: `lex`, `parse`, `parse_usx`, `parse_html`,
`reference_index`, each over the five file classes (plain, attributes-heavy,
alignment-heavy, note-heavy, whole-corpus) with `Throughput::Bytes`. Baseline
in `docs/benchmarks.md`: three runs each, medians and spread, machine and
commit recorded. Whole-corpus medians: lex 153, parse 50.0, parse_usx 19.6,
parse_html 40.8 MiB/s; `reference_index` is too noisy on this VM to resolve
3% (treat under ~15% as noise). Full run is about 3.5 minutes.

`usfm_parser` re-exports `UniquePromise` under the `benchmarking` feature only,
so the bench can construct a `Lexer`; the default build is unchanged.
