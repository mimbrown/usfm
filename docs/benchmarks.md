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

## After ticket 05

Ticket 05 (Miri on the unsafe code, 2026-09-19) rewrote the lexer's `Source` and
took 26 of the workspace's 27 `unsafe` uses out, so the numbers above no longer
describe the code in the tree. Ticket 06 then changed the USX writer, so the
numbers M3 compares against are the ones under "M2 close" below, not these.

### `unsafe` before and after

| File | Before | After |
| --- | ---: | ---: |
| `usfm_parser/src/lexer/source.rs` | 21 (15 blocks, 6 `unsafe fn`) | 0 |
| `usfm_parser/src/lexer/mod.rs` | 3 | 0 |
| `usfm_ast/src/string_parser.rs` | 2 | 0 |
| `usfm_parser/src/cursor.rs` | 1 | **1** |
| **Total** | **27** | **1** |

`Source` now holds `&'a str` plus a `usize` offset instead of three raw
pointers, and `SourcePosition` is an offset instead of a pointer. The one that
stayed is `ParserImpl::src` — `str::get_unchecked` over a token span — with a
`debug_assert` of the invariant that every test build and `scripts/miri.sh`
check. Miri itself found nothing to fix.

### Before and after, whole binaries

Five rounds, alternating the two binaries within each round (`bench-unsafe`,
built from `991d902`, and `bench-final`, built from this tree) so that a drift
in the machine hits both equally. Same VM, toolchain and profile as the
baseline above. Cells are criterion's point estimate per round; **median** of
the five.

| Id | Before (median of 5) | After (median of 5) | Δ |
| --- | ---: | ---: | ---: |
| `lex/plain` | 136.9 | 127.0 | −7.3% |
| `lex/attributes-heavy` | 152.9 | 149.9 | −2.0% |
| `lex/alignment-heavy` | 181.2 | 178.2 | −1.7% |
| `lex/note-heavy` | 140.4 | 128.8 | −8.3% |
| **`lex/whole-corpus`** | **152.9** | **146.6** | **−4.2%** |
| `parse/plain` | 67.2 | 65.0 | −3.2% |
| `parse/attributes-heavy` | 31.2 | 31.4 | +0.4% |
| `parse/alignment-heavy` | 63.1 | 64.9 | +3.0% |
| `parse/note-heavy` | 48.5 | 47.1 | −2.8% |
| **`parse/whole-corpus`** | **49.1** | **50.1** | **+2.0%** |

### What the `lex` row does *not* mean

The `lex` loss is **not** attributable to the `Source` rewrite, and reading it
as one would send M3 chasing the wrong thing. `lex` never calls
`ParserImpl::src`, yet three binaries whose lexer sources are byte-identical —
the same offset-based `Source` — and which differ only in the body of `src` lex
`plain` at 138.7, 127.8 and 123.8 MiB/s, and the whole corpus at 153.7, 146.6
and 143.0. Those numbers are stable across rounds; it is the binaries that
differ, not the runs. The `usfm_parser` rlib is built with the default 16
codegen units and no LTO, so editing any function in the crate re-partitions and
re-places the rest, and the lexer's hot loop moves with it. **Whole-binary `lex`
numbers on this machine carry a stable ±5–8% that has nothing to do with the
code being measured.**

So: the pointer-based `Source` measured 152.4, 152.9 and 153.5 MiB/s on
`lex/whole-corpus` across the three interleaved sets, and the offset-based one
measured 153.7 in the binary that happened to be laid out well. The rewrite can
lex at least as fast as the pointers did, and the committed binary's 146.6 is
placement. That is the answer to "did removing the lexer's 21 `unsafe` uses cost
anything": no.

The lesson for M3, which moves every one of these functions into new crates:
**compare `lex` across a crate split only with LTO on or codegen-units set to
1**, or the split's own placement churn will read as a regression.

### Why `ParserImpl::src` kept its `unsafe`

Two comparisons where everything else is held fixed: same offset `Source`, same
everything but the body of `src`, binaries interleaved round by round.

**A. `&self.source_text[a..b]` against `get_unchecked`** — eight rounds each,
no `debug_assert` in either. Medians:

| Id | `get_unchecked` | safe indexing | Cost of safe |
| --- | ---: | ---: | ---: |
| `parse/plain` | 65.6 | 62.4 | −4.8% |
| `parse/attributes-heavy` | 32.9 | 32.3 | −1.9% |
| `parse/alignment-heavy` | 65.7 | 64.8 | −1.4% |
| `parse/note-heavy` | 47.4 | 46.1 | −2.6% |
| **`parse/whole-corpus`** | **50.5** | **49.4** | **−2.3%** |

**B. `self.source_text.get(a..b).unwrap_or("")` against `get_unchecked`** —
three rounds each, both with the `debug_assert` the committed version carries.
Medians:

| Id | `get_unchecked` | safe `get` | Cost of safe |
| --- | ---: | ---: | ---: |
| `parse/plain` | 64.8 | 62.3 | −3.8% |
| `parse/attributes-heavy` | 32.6 | 31.3 | −3.9% |
| `parse/alignment-heavy` | 65.2 | 62.9 | −3.5% |
| `parse/note-heavy` | 47.1 | 45.3 | −3.8% |
| **`parse/whole-corpus`** | **49.9** | **48.2** | **−3.5%** |

Every `parse` class is slower in both, `plain` — the 5.2 MB of real Bible text —
by 3.8–4.8%, and `whole-corpus` by 2.3–3.5%. `get` is no cheaper than indexing:
both pay the same two `is_char_boundary` checks per call, on a function called
for every word, every marker name and every attribute. That is over the 2% the
ADR asks of an `unsafe` block, so it stayed — with a `debug_assert` that makes
every test run and every Miri run check the invariant the `SAFETY` comment
claims.

## M2 close: the numbers M3 compares against

Taken on `bfaa57f` (2026-09-19, ticket 06 merged, the code M3 starts from),
same VM and toolchain as the baseline, `cargo bench -p usfm_benchmark` three
times in a row on an otherwise idle machine, all five groups. MiB/s; median of
three; spread = (max − min) / median.

| Id | R1 | R2 | R3 | Median | Spread |
| --- | ---: | ---: | ---: | ---: | ---: |
| `lex/plain` | 120.9 | 120.8 | 120.3 | **120.8** | 0.5% |
| `lex/attributes-heavy` | 151.4 | 148.5 | 149.3 | **149.3** | 1.9% |
| `lex/alignment-heavy` | 174.8 | 174.0 | 174.1 | **174.1** | 0.4% |
| `lex/note-heavy` | 123.5 | 122.3 | 123.3 | **123.3** | 1.0% |
| `lex/whole-corpus` | 142.8 | 140.1 | 141.9 | **141.9** | 1.9% |
| `parse/plain` | 65.0 | 63.9 | 64.8 | **64.8** | 1.7% |
| `parse/attributes-heavy` | 31.8 | 31.9 | 32.7 | **31.9** | 2.7% |
| `parse/alignment-heavy` | 61.3 | 61.1 | 61.8 | **61.3** | 1.2% |
| `parse/note-heavy` | 46.9 | 47.4 | 47.4 | **47.4** | 0.9% |
| `parse/whole-corpus` | 49.3 | 49.4 | 48.4 | **49.3** | 2.1% |
| `parse_usx/plain` | 28.5 | 28.3 | 28.1 | **28.3** | 1.3% |
| `parse_usx/attributes-heavy` | 11.9 | 11.5 | 11.7 | **11.7** | 3.1% |
| `parse_usx/alignment-heavy` | 21.6 | 22.1 | 22.1 | **22.1** | 2.1% |
| `parse_usx/note-heavy` | 18.4 | 19.1 | 18.9 | **18.9** | 3.4% |
| `parse_usx/whole-corpus` | 18.9 | 19.0 | 18.5 | **18.9** | 2.7% |
| `parse_html/plain` | 53.5 | 54.0 | 53.4 | **53.5** | 1.2% |
| `parse_html/attributes-heavy` | 25.3 | 25.9 | 25.5 | **25.5** | 2.5% |
| `parse_html/alignment-heavy` | 48.1 | 47.6 | 48.5 | **48.1** | 1.8% |
| `parse_html/note-heavy` | 37.0 | 36.8 | 37.5 | **37.0** | 1.8% |
| `parse_html/whole-corpus` | 39.4 | 38.0 | 39.7 | **39.4** | 4.3% |
| `reference_index/plain` | 238.3 | 228.5 | 240.6 | **238.3** | 5.1% |
| `reference_index/attributes-heavy` | 177.7 | 184.9 | 198.4 | **184.9** | 11.2% |
| `reference_index/alignment-heavy` | 1458.1 | 1483.1 | 942.5 | **1458.1** | 37.1% |
| `reference_index/note-heavy` | 526.9 | 551.9 | 551.7 | **551.7** | 4.5% |
| `reference_index/whole-corpus` | 246.2 | 239.6 | 239.9 | **239.9** | 2.7% |

Against the M2 baseline medians: `lex/whole-corpus` 153.0 → 141.9 (−7%, the
codegen-placement effect described above, not a code change: the lexer is
byte-identical to the "After ticket 05" build), `parse/whole-corpus` 50.0 →
49.3 (−1.4%), `parse_usx/whole-corpus` 19.6 → 18.9 (−3.6%: ticket 06's
control-character replacement in the writer, and the `duplicate-attribute`
check per attribute), `parse_html/whole-corpus` 40.8 → 39.4 (−3.4%, no HTML
code changed; placement again). The M3 exit test is the median of three runs
per group against **this** table, interleaved with a build of `bfaa57f` as
"Reading a regression" says, and `reference_index` is judged only past ~15%.

## After ticket 13

`usfm_usx` (2026-09-19): `usx.rs` and `xml_document.rs` moved out of
`usfm_parser` into their own crate, and the walk became a
`usfm_ast::visit::Visit` implementation over private state instead of the
`ToUsx` trait with a shared `Context`. Only `parse_usx` can move, so only
`parse_usx` was rerun.

Same VM, toolchain and profile as the baseline. Two binaries built first — one
from `fe63ec3` in a worktree, one from this tree — then run turn about, three
rounds each, `--bench corpus -- parse_usx`, as "Reading a regression" says.
MiB/s, criterion's point estimate per round; median of three.

| Id | base R1 | base R2 | base R3 | **base** | new R1 | new R2 | new R3 | **new** | Δ |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `parse_usx/plain` | 26.99 | 27.65 | 28.32 | **27.65** | 26.62 | 27.49 | 26.91 | **26.91** | −2.7% |
| `parse_usx/attributes-heavy` | 11.63 | 12.14 | 12.30 | **12.14** | 11.79 | 11.83 | 11.96 | **11.83** | −2.6% |
| `parse_usx/alignment-heavy` | 21.67 | 22.28 | 22.93 | **22.28** | 21.74 | 22.41 | 22.72 | **22.41** | +0.5% |
| `parse_usx/note-heavy` | 18.64 | 19.34 | 19.18 | **19.18** | 19.10 | 18.82 | 19.20 | **19.10** | −0.4% |
| **`parse_usx/whole-corpus`** | 18.92 | 19.27 | 19.93 | **19.27** | 19.02 | 19.44 | 19.24 | **19.24** | **−0.1%** |

Every class is inside the 3% the spec asks for. Against the "M2 close" absolutes
the new numbers are +1.1% to +1.8% on every class except `plain`, which reads
−4.9% — but the `fe63ec3` build measured 27.65 on `plain` in the same sitting
(−2.3% from its own recorded 28.3), so most of that is the machine, which is why
the comparison that counts is the interleaved one above.

### What it took to get there, and the two traps

The first two versions of the visitor were **5 to 7% slower on
`whole-corpus`**, and finding out why took two more interleaved runs. Both
causes are worth knowing before the next output crate moves:

1. **An allocation that the old code did not make.** `visit_char` works out
   each attribute's name as a `String` (the marker's default name, `file` for
   `\fig`'s `src`, or the name as written). The helper took `&str`, so
   `OwnedName::local` copied that `String` into a second one — two extra
   allocations per `\w`. Taking `impl Into<String>` lets the `String` move in.
   That alone was most of `attributes-heavy`'s −4.4%, and it is the only *real*
   regression the rewrite introduced.
2. **Codegen placement across the new crate boundary**, the effect ticket 05
   documented under "What the `lex` row does *not* mean". Building both
   binaries with `CARGO_PROFILE_BENCH_CODEGEN_UNITS=1` moved the same code from
   −7.1% to −4.4% on `whole-corpus` with nothing else changed. Splitting a
   crate re-partitions what is left behind as well, so **a few per cent of any
   M3 split's apparent regression is placement, not code**; measure a suspect
   split at `codegen-units=1` before optimising for it.

A third thing that turned out *not* to matter: how the visitor holds the
children of the element it is building. A `Vec<Vec<XmlNode>>` frame stack and a
single `Vec<XmlNode>` swapped in and out by `UsxWriter::element` measured the
same. The committed version is the swap, because it writes each node once.

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

Ticket 05 added one more rule, learned the hard way (see "What the `lex` row
does *not* mean"): **build both binaries first, then alternate them round by
round.** `cargo bench` three times on the old code and three times on the new
code measures the half-hour between the two as much as the change — over one
sitting the same binary drifted 4% — and a single binary's numbers for a
benchmark it does not exercise can still move 8% from codegen-unit placement.
`cargo bench -p usfm_benchmark --no-run` prints the bench executable's path; it
is relocatable (the corpus path is absolute), so copy it somewhere, rebuild,
copy the other, and run them turn about with `--bench <filter>`.
