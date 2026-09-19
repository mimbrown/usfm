# Benchmarks

The M2 baseline (`.scratch/oxc-layout/spec.md`, ticket 04). M3's exit criterion
is "benchmarks within 3% of M2", so this file records not only the numbers but
the machine, the corpus commit and how much the numbers move when nothing
changes.

## What is measured

`tasks/benchmark` (`usfm_benchmark`) is a criterion bench over the committed
corpus in `tasks/benchmark/corpus/` (see its README for provenance and
licence). One command runs everything:

```bash
cargo bench -p usfm_benchmark
```

Five groups, each reporting throughput over the bytes of USFM it was given:

| Group | One iteration does |
| --- | --- |
| `lex` | `Lexer::new(&text, …)` driven to exhaustion, every token black-boxed |
| `parse` | `Parser::new(&text).parse(&sheet)` |
| `parse_usx` | `parse`, then `usx::to_usx_string(&document)` |
| `parse_html` | `parse`, then `to_html_string(&document, document.style_sheet())` |
| `reference_index` | `document.reference_index()`; the parse is done once **outside** the timed loop |

Five benchmark ids per group, one per file class. **The input of one id is the
whole class**: an iteration lexes or parses every file of the class, one after
another, so the number is an aggregate over the class and not one book. The classes are
the ones `corpus/README.md` defines; they overlap (`note-heavy` is one of the
86 `plain` files, and `whole-corpus` is `plain` plus the two synthetic classes
with nothing counted twice).

| Class | Files | Bytes | MiB |
| --- | ---: | ---: | ---: |
| plain | 86 | 5 483 072 | 5.23 |
| attributes-heavy | 3 | 4 034 752 | 3.85 |
| alignment-heavy | 1 | 3 883 060 | 3.70 |
| note-heavy | 1 | 80 219 | 0.08 |
| whole-corpus | 90 | 13 400 884 | 12.78 |

The default stylesheet (`usfm_parser::DEFAULT_STYLESHEET`) is built once, and
every file is read into a `String` before the timed loop, so no group is
measuring the disk or the stylesheet build. The CLI binary is not benchmarked:
the groups call the library directly.

Criterion is configured in `benches/corpus.rs` at `warm_up_time` 1 s,
`measurement_time` 5 s and `sample_size` 10 — the smallest settings that still
give criterion its ten samples when one whole-corpus iteration takes a quarter
of a second. The defaults (100 samples over 5 s) would take hours. A full run
of all five groups is about **3.5 minutes**.

Criterion's own output directory, `target/criterion`, is git-ignored through
the `/target` entry in `.gitignore`, so nothing a run writes is committed.
Only this file is.

**Units.** The tables below are **MiB/s**, which is what criterion prints
(`thrpt: [... MiB/s ...]`), copied without conversion so a number here can be
compared with a run's output by eye. MB/s (10⁶ bytes) is 1.048576× the MiB/s
figure.

## Baseline

| | |
| --- | --- |
| Date | 2026-09-19 |
| Commit | `10cb9dd` (the commit the corpus landed in; this bench crate is the only change on top) |
| Machine | 4 vCPU Intel(R) Xeon(R) @ 2.80GHz, 15 GB RAM — the cloud VM the agent loop runs on |
| OS | Linux 6.18.44 |
| Toolchain | `rustc 1.98.0 (88d9e12ae 2026-08-18)`, `cargo 1.98.0` |
| Profile | `bench` (release, default workspace profile) |
| Runs | three consecutive `cargo bench -p usfm_benchmark`, nothing else running |

Each cell is criterion's point estimate for that run. **Median** is the median
of the three; **spread** is (max − min) as a percentage of that median.

### `lex`

The lexer alone: `Lexer::new` through the parser's `benchmarking` feature,
which is what re-exports `UniquePromise` so another crate can build a `Lexer`.
These three runs were taken with `cargo bench -p usfm_benchmark --bench corpus
-- lex`; every other table comes from a full run.

| Class | Run 1 | Run 2 | Run 3 | Median | Spread |
| --- | ---: | ---: | ---: | ---: | ---: |
| plain | 137.1 | 137.5 | 136.2 | **137.1** | 0.9% |
| attributes-heavy | 152.1 | 154.2 | 155.7 | **154.2** | 2.4% |
| alignment-heavy | 179.6 | 181.4 | 180.3 | **180.3** | 1.0% |
| note-heavy | 140.4 | 140.1 | 140.2 | **140.2** | 0.3% |
| whole-corpus | 153.0 | 153.0 | 153.8 | **153.0** | 0.5% |

Lexing is about three times the speed of a full parse, and the ordering across
classes inverts: the markup-dense classes lex *faster* per byte (alignment-heavy
180 MiB/s against plain's 137) because their extra bytes are long marker and
attribute runs the lexer classifies in one pass, while the same bytes are the
most expensive thing the parser does.

### `parse`

| Class | Run 1 | Run 2 | Run 3 | Median | Spread |
| --- | ---: | ---: | ---: | ---: | ---: |
| plain | 66.2 | 65.5 | 65.9 | **65.9** | 1.1% |
| attributes-heavy | 32.1 | 32.7 | 33.1 | **32.7** | 2.9% |
| alignment-heavy | 63.3 | 65.2 | 66.0 | **65.2** | 4.1% |
| note-heavy | 47.2 | 48.0 | 47.9 | **47.9** | 1.6% |
| whole-corpus | 49.0 | 50.0 | 50.4 | **50.0** | 2.9% |

### `parse_usx`

| Class | Run 1 | Run 2 | Run 3 | Median | Spread |
| --- | ---: | ---: | ---: | ---: | ---: |
| plain | 28.5 | 28.4 | 28.7 | **28.5** | 0.9% |
| attributes-heavy | 12.3 | 12.1 | 12.4 | **12.3** | 2.3% |
| alignment-heavy | 23.6 | 23.3 | 23.5 | **23.5** | 1.4% |
| note-heavy | 19.0 | 19.1 | 19.0 | **19.0** | 0.8% |
| whole-corpus | 19.3 | 19.6 | 19.7 | **19.6** | 2.2% |

### `parse_html`

| Class | Run 1 | Run 2 | Run 3 | Median | Spread |
| --- | ---: | ---: | ---: | ---: | ---: |
| plain | 55.5 | 55.9 | 55.5 | **55.5** | 0.7% |
| attributes-heavy | 26.4 | 26.3 | 26.5 | **26.4** | 0.5% |
| alignment-heavy | 51.5 | 51.3 | 51.2 | **51.3** | 0.5% |
| note-heavy | 37.9 | 37.7 | 38.0 | **37.9** | 0.7% |
| whole-corpus | 40.8 | 41.1 | 40.8 | **40.8** | 0.9% |

### `reference_index`

The parse is outside the timed loop, so these are the cost of walking a built
tree, expressed against the bytes of source the tree came from. They are the
noisiest numbers here and the least comparable across classes: alignment-heavy
is fast per source byte only because its 3.7 MB of source holds one ordinary
book's worth of chapters and verses wrapped in milestone markup.

| Class | Run 1 | Run 2 | Run 3 | Median | Spread |
| --- | ---: | ---: | ---: | ---: | ---: |
| plain | 232.2 | 229.7 | 233.9 | **232.2** | 1.8% |
| attributes-heavy | 209.8 | 192.6 | 207.8 | **207.8** | 8.3% |
| alignment-heavy | 1391.8 | 1508.9 | 1569.1 | **1508.9** | 11.7% |
| note-heavy | 548.5 | 566.0 | 545.6 | **548.5** | 3.7% |
| whole-corpus | 227.7 | 249.0 | 248.0 | **248.0** | 8.6% |

## Reading a regression

The VM is a shared 4-vCPU cloud instance, so the numbers move on their own.
Over these three back-to-back runs the spread is **under 3% for every `lex`,
`parse`, `parse_usx` and `parse_html` id except `parse/alignment-heavy`
(4.1%)**, and 4–12% for `reference_index`, whose timed work is small enough
that scheduler noise dominates.

The spec's M3 exit criterion — "benchmarks within 3% of M2" — should therefore
be read as:

- compare **the median of three runs** against the median recorded above, not
  a single run against a single run;
- on the **same machine class** (4 vCPU Xeon @ 2.80 GHz), with nothing else
  running, and ideally back to back with a rerun of the M2 commit rather than
  against these absolute numbers months later;
- apply the 3% threshold to `lex`, `parse`, `parse_usx` and `parse_html`, where the
  measurement is steady enough to support it. `reference_index` cannot resolve
  3% on this hardware; treat a change there as real only past ~15%, or rerun it
  somewhere quieter.

Criterion prints its own change-since-last-run line when `target/criterion`
holds a previous run, which is the cheapest way to see a regression: run the
old commit, then the new one, on the same machine in the same sitting.
