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

Seven groups, each reporting throughput over the bytes of USFM it was given:

| Group | One iteration does |
| --- | --- |
| `lex` | `Lexer::new(&text, …)` driven to exhaustion, every token black-boxed |
| `parse` | `Parser::new(&text).parse(&sheet)` — the parser alone |
| `parse_semantic` | `usfm::parse(&text)` — the parser, then `usfm_semantic::analyze` |
| `parse_usx` | `parse`, then `usx::to_usx_string(&document)` |
| `parse_html` | `parse`, then `to_html_string(&document, document.style_sheet())` |
| `parse_json` | `parse`, then `usfm_json::to_json_string(&document)` |
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
of all seven groups is about **6 minutes**.

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
| `crates/usfm_parser/src/lexer/source.rs` | 21 (15 blocks, 6 `unsafe fn`) | 0 |
| `crates/usfm_parser/src/lexer/mod.rs` | 3 | 0 |
| `crates/usfm_ast/src/string_parser.rs` | 2 | 0 |
| `crates/usfm_parser/src/cursor.rs` | 1 | **1** |
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

## After ticket 14

`usfm_html` (2026-09-19): `serialize_html.rs`, `serialize.rs` and `context.rs`
moved out of `usfm_parser` into their own crate, and the writer gained the
escaping it never had — `&`, `<`, `>` in text, `"` as well in an attribute
value, and U+FFFD for the characters HTML cannot carry, through the same
byte-table scan ticket 06 gave the USX writer. Only `parse_html` can move, so
only `parse_html` was rerun.

Same VM, toolchain and profile as the baseline. Two binaries built first — one
from `d918bd4` in a worktree, one from this tree — then run turn about, three
rounds each, `--bench parse_html`, as "Reading a regression" says. MiB/s,
criterion's point estimate per round; median of three. The last column is
against the "M2 close" medians, which is the spec's exit test.

| Id | base R1 | base R2 | base R3 | **base** | new R1 | new R2 | new R3 | **new** | Δ base | Δ M2 close |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `parse_html/plain` | 54.51 | 54.95 | 54.11 | **54.51** | 54.94 | 53.97 | 53.32 | **53.97** | −1.0% | +0.9% |
| `parse_html/attributes-heavy` | 25.01 | 25.35 | 25.06 | **25.06** | 27.16 | 26.61 | 26.24 | **26.61** | +6.2% | +4.4% |
| `parse_html/alignment-heavy` | 49.75 | 50.39 | 48.99 | **49.75** | 49.26 | 47.58 | 49.24 | **49.24** | −1.0% | +2.4% |
| `parse_html/note-heavy` | 37.72 | 37.50 | 36.78 | **37.50** | 37.65 | 37.41 | 37.66 | **37.65** | +0.4% | +1.7% |
| **`parse_html/whole-corpus`** | 40.10 | 39.92 | 38.95 | **39.92** | 39.67 | 38.84 | 38.26 | **38.84** | **−2.7%** | **−1.4%** |

Every class is inside the 3% the spec asks for, on both comparisons.

### What the escaping cost, and what paid for it

The first version — escaping bolted on with `write!` where the old code had
`write!` — measured −2.6% on `whole-corpus` and **−4.9% on
`alignment-heavy`**, which is over the line. Two changes brought it back, in
this order:

1. **`write_str` instead of `write!` for everything that is not a number.** An
   alignment-heavy document is mostly milestones with several attributes each,
   and every ` data-x="y"` went through `format_args!`. Writing the literal,
   the name and the value as three `write_str` calls skips the formatting
   machinery entirely; the same change went into the `<span class="…">` of
   `Char`, `Para` and `Milestone`. `alignment-heavy` −4.9% → −3.5%,
   `whole-corpus` −2.6% → −1.4%.
2. **`position` instead of an indexed loop in the scan.** `while let
   Some(offset) = bytes[index..].iter().position(|&b| STOP[b as usize])` is the
   same scan with the bounds check hoisted out. Measured on its own,
   interleaved, three rounds each at `codegen-units=1`: `plain` +0.8%,
   `alignment-heavy` +1.5%, `whole-corpus` +1.1%, `attributes-heavy` −1.0%,
   `note-heavy` ±0. The USX writer's `write_escaped` still uses the indexed
   form and would take the same win.

**How much of the rest is placement.** Both binaries rebuilt with
`CARGO_PROFILE_BENCH_CODEGEN_UNITS=1` and run interleaved (this was measured on
version 1 above, before the `position` change): `plain` −2.4%,
`attributes-heavy` +0.9%, `alignment-heavy` −2.5%, `note-heavy` −1.6%,
`whole-corpus` −3.1%, against −3.0 / +3.0 / −3.5 / −1.3 / −1.4 at the default
16 codegen units in the same sitting. So the per-class numbers move by 1–3%
between the two builds with no code change at all — the effect ticket 05 and
ticket 13 both ran into. The cost of the escaping itself, read off the
`codegen-units=1` run, is about 2–3% on the text-heavy classes: one table scan
over every byte of text the writer emits, which is what buys HTML that a
browser reads back as what the AST held.

## After ticket 15

`usfm_pipeline` and `apps/usfm_cli` (2026-09-19): the binary, the text
replacements, the sectioning, the diglot weaving and the SILE output left
`usfm_parser`; no library code path the benches touch changed, so nothing
should move. One `whole-corpus` run against the recorded medians put
`parse_usx` 6% low (18.01 vs the 19.24 of "After ticket 13"), so the three
groups were rerun interleaved against a `5d376e8` worktree build, both at
`CARGO_PROFILE_BENCH_CODEGEN_UNITS=1`, three rounds each — **`parse` 47.50 →
47.10 MiB/s (−0.8%), `parse_usx` 18.04 → 18.04 (−0.0%), `parse_html` 40.03 →
40.14 (+0.3%)**, all inside 3%. The VM is simply a few percent slower today
than when tickets 13 and 14 were measured: at `5d376e8` itself `parse_usx`
measures 18.04 here, not 19.24, which is why the file says to interleave
rather than to compare absolutes.

## `parse_json` (ticket 16)

The new group, 2026-09-19, on the machine and toolchain of the baseline above.
It is a **new row, compared to nothing**: `usfm_json` did not exist before, so
there is no earlier number and no 3% question. Three consecutive runs,
`cargo bench -p usfm_benchmark --bench corpus -- parse_json`, nothing else
running; MiB/s, as everywhere in this file.

| Class | Run 1 | Run 2 | Run 3 | Median | Spread |
| --- | ---: | ---: | ---: | ---: | ---: |
| plain | 17.64 | 17.11 | 17.82 | **17.64** | 4.0% |
| attributes-heavy | 4.28 | 4.26 | 4.28 | **4.28** | 0.4% |
| alignment-heavy | 8.35 | 8.25 | 8.23 | **8.25** | 1.4% |
| note-heavy | 11.23 | 11.15 | 11.24 | **11.23** | 0.8% |
| **whole-corpus** | 7.90 | 7.86 | 8.00 | **7.90** | 1.7% |

Whole-corpus throughput is **7.90 MiB/s**: the 12.78 MiB corpus parses and is
written as JSON in about 1.62 s. `parse/whole-corpus` was rerun in the same
sitting for the subtraction and came out at 47.9 MiB/s (0.27 s), which leaves
the writing itself at about **9.5 MiB/s**. The same subtraction on ticket 15's
same-day numbers puts USX at about 29 MiB/s and HTML at about 270, so JSON is
by some way the most expensive output this repo writes.

**Why it is the slowest output, and why `attributes-heavy` is under 5 MiB/s.**
The unit of JSON output is an object per node, and a `serde_json::Map` is a
`BTreeMap<String, Value>`: every field costs a `String` key and a tree
insertion, and every attribute pair is a second object of its own with two
keys and two owned values. USX, by contrast, adds an `OwnedAttribute` to an
element that already exists — no container per pair — and HTML writes bytes
straight into one `String`. `attributes-heavy` is the class where that
multiplies worst: nearly every word is a `\w …|lemma="…" strong="…"\w*`, so
per source byte it has the most nodes *and* the most attribute pairs, and it
lands at **4.28 MiB/s**. Ticket 16 asked for this to be noted rather than
optimised away; nothing in the writer is gratuitous (numbers go through
`Value::from`, not `format!`; the only formatted strings are a chapter or
verse number, once per marker), and the remaining cost is the shape of the
output, not the walk. A caller that needs this class faster wants a streaming
writer to a `String` rather than a `Value` tree — which `to_json_value`, the
API the language server asked for, rules out for now.

## M3 close: the split, measured against `bfaa57f`

The M3 exit criterion ("benchmarks within 3% of M2") for ticket 17, which moved
every crate under `crates/`, the runner under `tasks/conformance` and put the
`usfm` facade between the applications and the libraries.

Method, as "Reading a regression" below prescribes, plus one thing it asks for
by name: **both binaries were built with `CARGO_PROFILE_BENCH_CODEGEN_UNITS=1`**,
because a split that moves every function into a new crate is exactly the change
the "What the `lex` row does *not* mean" section warns would otherwise read as a
regression. The baseline is a `git worktree` of `bfaa57f` — the commit the "M2
close" table above was taken on, and the one that predates the split — built in
place with `cargo bench -p usfm_benchmark --no-run`. The two bench executables
were then run turn about, three rounds each, on the whole-corpus id of the five
original groups (`parse_json` did not exist at `bfaa57f` and is not comparable).
Cells are criterion's point estimate in MiB/s; Δ is new median over base median,
so **positive is faster**.

| Id | `bfaa57f` R1 | R2 | R3 | median | ticket 17 R1 | R2 | R3 | median | Δ |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `lex/whole-corpus` | 142.2 | 142.4 | 141.0 | **142.2** | 146.3 | 146.3 | 145.6 | **146.3** | **+2.8%** |
| `parse/whole-corpus` | 49.6 | 49.5 | 49.0 | **49.5** | 48.3 | 49.9 | 48.9 | **48.9** | **−1.3%** |
| `parse_usx/whole-corpus` | 18.8 | 18.9 | 18.7 | **18.8** | 18.7 | 18.7 | 19.0 | **18.7** | **−0.4%** |
| `parse_html/whole-corpus` | 38.0 | 38.6 | 39.4 | **38.6** | 39.6 | 40.4 | 40.2 | **40.2** | **+4.3%** |
| `reference_index/whole-corpus` | 254.4 | 258.3 | 269.4 | **258.3** | 254.5 | 261.7 | 272.9 | **261.7** | **+1.3%** |

**Verdict: the criterion is met.** Nothing is more than 3% slower; the two
groups outside ±3% are both faster, and `reference_index`, which is judged only
past ~15%, moved 1.3%.

Two things worth reading off this table:

- **`parse_html` is 4.3% faster than `bfaa57f` even though `bfaa57f` escaped
  nothing.** Ticket 14 measured the escaping it added at 2–3% on text-heavy
  classes, and that cost is real — it is in the per-byte scan, and the "After
  ticket 14" table above still records it. It does not show here because that
  measurement was taken with the default 16 codegen units, where this crate's
  own placement noise is ±5–8%; with codegen units pinned to 1 both binaries
  are laid out deterministically and the escaping is inside the noise of the
  layout change that came with the split. Do not read this row as "the
  escaping is free" — read it as "the split did not cost anything that the
  escaping had not already been charged for."
- **The `lex` and `parse_html` gains are not an optimisation.** Nothing in the
  lexer or the HTML writer changed in ticket 17; the crates they live in did.
  Pinning codegen units to 1 is itself worth a few percent on both binaries,
  and what this table shows is that the change is symmetric — which is the
  whole point of building both sides the same way.

Spread — (max − min) / median over the three rounds — is 0.5–1.0% for `lex`,
1.1–1.6% for `parse_usx`, 1.2–3.3% for `parse`, 2.0–3.6% for `parse_html` and
5.8–7.0% for `reference_index`: the same ordering the M2 table shows, which is
why the 3% threshold applies to the first four and `reference_index` is judged
only past ~15%. Raw criterion output is not committed; rerun with the recipe
above.

## `parse_semantic` (ticket 19)

The new group, 2026-09-19, on the machine and toolchain of the baseline above.
One iteration is `usfm::parse(&text)`: the parse of the `parse` group, plus
`usfm_semantic::analyze` over the tree it built and the merge and sort of the
two diagnostic lists. Like `parse_json` it is a **new row compared to
nothing** — the semantic pass did not exist before — but unlike `parse_json`
it has a natural neighbour, so `parse` was rerun in the same sitting, turn
about with it, from one bench binary built once
(`cargo bench -p usfm_benchmark --no-run`, then
`corpus-… --bench parse_semantic` and `corpus-… --bench '^parse/'`, three
rounds each). MiB/s, as everywhere in this file.

| Class | Run 1 | Run 2 | Run 3 | Median | Spread | `parse` median, same sitting |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| plain | 65.55 | 66.11 | 65.19 | **65.55** | 1.4% | 65.47 |
| attributes-heavy | 31.24 | 32.28 | 32.61 | **32.28** | 4.3% | 32.51 |
| alignment-heavy | 59.74 | 61.78 | 61.77 | **61.77** | 3.3% | 62.18 |
| note-heavy | 47.19 | 47.13 | 47.10 | **47.13** | 0.2% | 47.49 |
| **whole-corpus** | 48.49 | 47.46 | 47.34 | **47.46** | 2.4% | 49.01 |

**What the pass costs: about 3% of a parse, and nothing that shows above the
noise on three of the five classes.** Whole-corpus is the one number worth
reading: 47.46 against 49.01 MiB/s is 0.269 s against 0.261 s for the 12.78 MiB
corpus, so the semantic walk plus the merge and the sort cost about **8 ms over
12.78 MiB**, or a walk running at roughly 1 500 MiB/s. That is what a second
full traversal of a built tree costs when the only check on it so far reads one
block kind: the walk itself dominates, not `unlisted-book-code`. `plain` shows
the two groups inside 0.2% of each other and `attributes-heavy` shows
`parse_semantic` *faster* than `parse`, which is this VM's noise floor (spread
within one group is 1.4–4.3%) rather than a finding.

The number to watch as ticket 20 and ticket 21 move the placement, attribute
and table checks across is the gap between the two medians, not the absolute:
the traversal is already paid for, so a check added to the existing walk should
cost far less than the first one did. A future run that finds the gap widening
past ~10% on whole-corpus means a check is doing real work per node and wants
its own line in this file.

**After ticket 20** (placement and the attribute checks moved across), same
sitting, three rounds turn about: whole-corpus **45.58** MiB/s against `parse`
at **52.05** — the gap is 12.4%, past the ~10% this section said to watch for,
and the cause is the attribute checks rather than the placement stack. Removing
only `check_attributes` from the walk puts it back at 50.0 MiB/s, and the split
by class says the same: `plain`, which has almost no attribute lists, is 65.2
against 66.1 (1.4%), while `attributes-heavy` is 30.5 against 36.1 (15.4%).
Most of that is not the checking but the *reading*: taking the per-pair loop
out and leaving only "is the list empty, has it an unnamed value" recovers
barely 1 MiB/s, so what it costs is a second pass over attribute lists that the
parser used to check while they were still in cache. `parse` itself gained the
same work back (52.05 against 49.01 in the run above, on a VM that also drifted
up). This is the price of the split, not a check doing something silly per
node, and it is paid only by documents that are mostly attributes.

**After ticket 21** (the structure, verse-placement, table-column and
`empty-word` checks moved across — nine more codes), same method, three rounds
turn about on whole-corpus: `parse_semantic` **45.89** MiB/s (45.49, 46.56,
45.89) against `parse` at **51.51** (51.51, 51.09, 51.58). The gap is **10.9%**,
against 12.4% after ticket 20 — that is, the nine checks added here cost
nothing that shows above this VM's noise, which is what a check reading fields
of nodes the walk already visits should cost. One further round with the classes
split says where the remaining gap is, and it is where ticket 20 left it:
`plain` 65.08 against 66.28 (1.8%), `note-heavy` 45.67 against 49.15 (7.1%),
`alignment-heavy` 57.08 against 66.65 (14.4%) and `attributes-heavy` 30.13
against 36.70 (17.9%). The two heavy classes are the two full of attribute
lists — an alignment file is `\zaln-s |x-strong="…"` most of the way down — so
the cost is still the second pass over those lists, not the checks moved here.

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
