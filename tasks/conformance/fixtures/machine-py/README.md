# machine.py fixtures

The USFM test data from
[sillsdev/machine.py](https://github.com/sillsdev/machine.py), commit
`e2af2c868043c2b3594789d1110b05eda96de30e`, fetched 2026-09-19. MIT, © 2022
SIL International; `LICENSE` here is that repository's licence file, copied
verbatim. Scripture excerpts inside the inputs belong to their respective
publishers. See `NOTICE.md` at the repository root.

Only `tests/testutils/data/usfm/` is vendored for the parser, and only the
files we use; `usx/` below holds the USX (ticket 47). Upstream's
`samples/data/` Paratext projects (`WEB-PT`, `VBL-PT`, `PEV-PT`) are left
there: between them they use 28 markers, every
one of which already appears in `Tes/41MATTes.SFM` or in the tcdocs and
usfm-grammar corpora. These are hand-written Paratext-shaped projects, not
conformance cases: they carry no expected USX and no `<validated>` verdict, so
they are not a harness root. The expectation for them is the parser's own tree
and diagnostics, recorded as snapshots in `crates/usfm_parser/tests/recovery.rs`.

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

## `usx/`

USX for the reader (`usfm_usx::read_usx`), from the same commit
(`e2af2c868043c2b3594789d1110b05eda96de30e`, fetched 2026-09-27; the five
files are byte for byte the same at `615cd599ab08633505b794f05244f966921e18e7`,
where ticket 47 found them). Copied byte for byte, byte-order marks included:
upstream's blob hashes are the ones `git hash-object` gives here. Like the
USFM books, they carry no expected output: the expectation is the reader's own
tree and diagnostics, recorded as the `machine_py_*` snapshots in
`crates/usfm_usx/tests/reader.rs`; `tasks/conformance/tests/usx_reader.rs`
round-trips the WEB books.

| File | Upstream path | Role |
| --- | --- | --- |
| `WEB-DBL/1JN.usx`, `2JN.usx`, `3JN.usx` | `samples/data/WEB-DBL/release/USX_1/` | The World English Bible's 1–3 John as a Digital Bible Library release writes them: USX 3.0 with a byte-order mark, every word a `<char style="w" strong="…">`, a line break and an indent after each `<para>` and each paragraph's first `<verse/>`, `vid` on a paragraph that continues a verse, every verse and chapter closed with an `eid`. The bundle's `metadata.xml` states the text is PUBLIC DOMAIN (rights holder eBible.org); the files are upstream's, MIT. They read with nothing to report and round-trip USX -> USFM -> USX. |
| `Tes/MAT.usx`, `Tes/MRK.usx` | `tests/testutils/data/usx/Tes/release/USX_1/` | A USX 2.6 test book, malformed on purpose: a `<verse>` outside any `<para>`, a `sid` and an `eid` on `2:1` alone, a repeated `\v 6` and a `\v 5` after it, USX 2's `<figure file=… size=… ref=…>`, and a `<para style="restore">`. `MRK` stops after its introduction, as `42MRKTes.SFM` does. |

The rest of the DBL bundle (`metadata.xml`, `license.xml`, `styles.xml`,
`eng_en.ldml`, `versification.vrs`) is left upstream: the reader takes USX and,
optionally, a stylesheet, and none of the books uses a style the default sheet
lacks. The `.usx` files are not fuzz seeds yet; that is ticket 49's, with the
reader's fuzz target.

## Fuzz seeds

Every `.SFM` here is a fuzz seed; `tasks/fuzz/seed.sh` copies them into both
corpora as `machine-py__<project>__<book>.usfm`.
