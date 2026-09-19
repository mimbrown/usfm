# machine.py fixtures

The USFM test data from
[sillsdev/machine.py](https://github.com/sillsdev/machine.py), commit
`e2af2c868043c2b3594789d1110b05eda96de30e`, fetched 2026-09-19. MIT, © 2022
SIL International; `LICENSE` here is that repository's licence file, copied
verbatim. Scripture excerpts inside the inputs belong to their respective
publishers. See `NOTICE.md` at the repository root.

Only `tests/testutils/data/usfm/` is vendored, and only the files we use.
Upstream's `samples/data/` Paratext projects (`WEB-PT`, `VBL-PT`, `PEV-PT`) and
its `WEB-DBL` bundle are left there: between them they use 28 markers, every
one of which already appears in `Tes/41MATTes.SFM` or in the tcdocs and
usfm-grammar corpora. These are hand-written Paratext-shaped projects, not
conformance cases: they carry no expected USX and no `<validated>` verdict, so
they are not a harness root. The expectation for them is the parser's own tree
and diagnostics, recorded as snapshots in `usfm_parser/tests/recovery.rs`.

## `Tes/`

A one-project corpus of five books plus the project's own stylesheet.

| File | Role |
| --- | --- |
| `41MATTes.SFM` | The one book that exercises anything: `\fe`, `\rq`, `\fm`, `\pn` with a nested `\+pro`, `\fig`, an `\esb` sidebar, two tables, `\ts-s`/`\ts-e`, `\va`/`\vp`, `//`, verse segments, and four deliberate mistakes. Snapshotted whole as `machine_py_41mat`, with a `check_variant` test for each shape no other test covered. |
| `03LEVTes.SFM` | `\v 55b`, a verse segment continuing `\v 55` in the same paragraph, and `\id Leviticus` — a book name where the code belongs. Snapshot `machine_py_03lev`. |
| `42MRKTes.SFM` | A scripture book that stops after its introduction: no `\c`, no `\v`, and nothing reported. Snapshot `machine_py_42mrk`. |
| `44JHNTes.SFM` | Zero bytes, which a Paratext project uses for a book nobody has started. Snapshot `machine_py_44jhn_empty`: it parses to no blocks, reports nothing, and must not panic. |
| `131CHTes.SFM` | Plain verses and one `\v 3-7` range, both covered by the tcdocs snapshots. Fuzz seed only. |
| `custom.sty` | The project stylesheet, defining `\test` as a character style. `machine_py_custom_stylesheet` builds the default sheet plus this one and checks that `\test` resolves instead of being an unknown marker. |

## `invalid_id/`, `mismatch_id/`

One book each, both named `07JDG.SFM` and both disagreeing with their own
`\id`: `invalid_id` says `\id JGS - Test` (well formed, but not a book USFM
lists, so `unlisted-book-code`) and `mismatch_id` says `\id JUD - Test` (Jude,
a perfectly good code — for a different book). Upstream uses them to test a
Paratext project loader, which knows the filename and can compare. A parser is
handed one file's text and cannot see the disagreement at all, so neither has
a test here; both are fuzz seeds.

## Fuzz seeds

Every `.SFM` here is a fuzz seed; `tasks/fuzz/seed.sh` copies them into both
corpora as `machine-py__<project>__<book>.usfm`.
