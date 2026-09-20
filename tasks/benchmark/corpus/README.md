# Benchmark corpus

The fixed USFM the M2 benchmarks (ticket 04) and the fuzz seeds (ticket 06)
run against. Everything here is committed, including the tools that produced
it, so a number in `docs/benchmarks.md` can always be tied to the exact bytes
it was measured on.

```
corpus/
  web/                       86 files, the whole World English Bible
  synthetic/
    attributes-heavy/        3 books, every word carries \w attributes
    alignment-heavy/         Luke, every word inside \zaln-s/\zaln-e
  tools/
    usfx_to_usfm.py          USFX -> USFM 3, one file per book
    synthesize.py            web/ -> synthetic/, seeded and deterministic
```

## Source and licence

The text is the **World English Bible** (WEB), which is in the **public
domain** — no copyright, no attribution requirement, no restriction on
redistribution. It is eBible.org's own edition, taken from the
[`seven1m/open-bibles`](https://github.com/seven1m/open-bibles) collection,
whose README lists `eng-web.usfx.xml` as public domain.

| | |
| --- | --- |
| File | `eng-web.usfx.xml` (6.2 MB) |
| URL | `https://raw.githubusercontent.com/seven1m/open-bibles/f257a3559025c3f873b48a75019f53a9354ed7de/eng-web.usfx.xml` |
| Commit | `f257a3559025c3f873b48a75019f53a9354ed7de` |
| Fetched | 2026-09-19 |

### Why USFX and not the eBible.org USFM zip

The obvious source is `https://ebible.org/Scriptures/eng-web_usfm.zip`, the
USFM eBible.org publishes directly. **This environment's network policy
refuses it**: the proxy answers `403` to `CONNECT` for `ebible.org`, and for
`git.door43.org` as well. `raw.githubusercontent.com` is reachable, so the
corpus is built from the same publisher's USFX rendering of the same text.

USFX is eBible.org's own lossless XML serialisation of its USFM — `<p
sfm="ip">` is `\ip`, `<q level="2">` is `\q2`, `<v id="1"/>` is `\v 1`,
`<f caller="+">` is `\f + ... \f*` — so `tools/usfx_to_usfm.py` is a
mechanical un-tagging, not a translation between formats. The counts below
show the round trip is exact.

## Regenerating

```bash
curl -L -o /tmp/eng-web.usfx.xml \
  https://raw.githubusercontent.com/seven1m/open-bibles/f257a3559025c3f873b48a75019f53a9354ed7de/eng-web.usfx.xml

python3 tools/usfx_to_usfm.py /tmp/eng-web.usfx.xml web
python3 tools/synthesize.py --seed 20260919 web synthetic
```

Both tools are Python 3 standard library only, take no network at run time,
and are deterministic: rerunning either over the same input gives
byte-identical files. `synthesize.py` seeds one `random.Random` per output
file from `seed|class|book` through `Random.seed(..., version=2)`, which is
SHA-512 based; nothing calls `hash()` on a string, whose value is randomised
per process.

Anything `usfx_to_usfm.py` has no rule for raises `Unsupported` rather than
being dropped, so the element list below stays exact if eBible.org revises
the file.

## What the converter does

Block elements, one USFM line each: `<id>`, `<ide>`, `<h>`, `<toc level>`,
`<c id>`, `<cp id>`, `<cl>`, `<b/>`, `<d>`, `<p sfm level>`, `<q level>`.
A `<p>` with no `sfm` is `\p`, a `<q>` with no level is `\q`, and `level="2"`
appends the level (`\q2`, `\ili2`, `\mt2`).

Inline: `<v id>` is `\v N ` (verse text follows on the same line), `<ve/>` is
dropped because USFM leaves verse ends implicit, `<f caller>`/`<x caller>`
become `\f + ...\f*` / `\x + ...\x*` with `<fr> <ft> <fq> <fqa> <fl>` and
`<xo> <xt>` as runs closed implicitly by the next run or by the note's end —
the way eBible.org's own USFM writes them. Note text that is not inside a run
element (how most WEB footnotes are carried) opens the note's default run,
`\ft` or `\xt`. `<wj> <add> <k> <qs> <it> <bk> <vp>` become
`\wj ...\wj*` and so on. `<ref tgt>` is USFX bookkeeping with no USFM marker:
the tag is dropped and its text kept inside the surrounding run.

Whitespace is normalised so each block is one line with single spaces and no
trailing space. Source whitespace is preserved rather than invented: a note
sits directly against the word it annotates (`God\f + ...\f* created`), and a
closing marker never has a space in front of it.

Two deliberate exceptions, both recorded here rather than done silently:

- `<toc level="4">` (Psalms only) is left out. USFM 3 defines `\toc1`–`\toc3`
  and the stylesheet has no `\toc4`; the same text — `Psalm` — is already
  carried by the `<cl>` element three lines later.
- One 2 Esdras footnote has `caller=""` with the `+` sitting in the element's
  leading text. The converter recovers the caller from there.

File names are `NN-BBB.usfm`, `NN` being the USFM book-identifier number
(`01`–`39` Old Testament, `41`–`67` New Testament, `68`–`85` deuterocanon),
as unfoldingWord and Paratext name their files. The two peripheral books the
USFX carries are numbered outside that table so every file has a numeric
prefix: `00-FRT.usfm` and `88-GLO.usfm`. The number is an identifier, not a
sequence index, so a directory listing puts the deuterocanon after Revelation
while the USFX order (and the order of `\id` lines) keeps it between Malachi
and Matthew.

## Verification

### 1. Counts

Every structural count in the converted corpus matches the USFX tag count it
came from, measured both in the USFM and in the USX the CLI produces from it:

| | USFX tags | `web/*.usfm` | USX from `web/` |
| --- | --- | --- | --- |
| verses | `<v>` 37654, `<ve>` 37654 | `\v` 37654 | `<verse sid>` 37654 |
| chapters | `<c>` 1391 | `\c` 1391 | `<chapter sid>` 1391 |
| footnotes | `<f>` 2514 | `\f*` 2514 | `<note style="f">` 2514 |
| cross references | `<x>` 358 | `\x*` 358 | `<note style="x">` 358 |
| words of Jesus | `<wj>` 2268 | `\wj*` 2268 | `<char style="wj">` 2268 |

The other tags line up the same way: `add` 1050, `b` 1021, `d` 139, `k` 91,
`qs` 74, `fl` 47, `fr` 44, `fq` 4, `it` 2, `vp` 2, `bk` 1, `cl` 1, `cp` 1,
`xo` 1, `h` 86, `id` 86, `ide` 66, `toc1`/`toc2`/`toc3` 86 each (the 259th
`<toc>` is the dropped level 4). `\ft` 2551 and `\xt` 358 are higher than
`<ft>` 1333 and `<xt>` 20 because bare note text opens the default run.

### 2. The parser accepts it

Every file was run through `target/release/usfm parse <file> -o out.usx`
(`cargo build -p usfm_cli --release`; the binary was `usfm_parser` until ticket
15 moved it to `apps/usfm_cli`).

`web/` — 86 files, **not one diagnostic** in the whole corpus:

| Code | Severity | Count |
| --- | --- | --- |
| — | — | 0 |

`synthetic/` — 4 files, **no errors**:

| Code | Severity | Count |
| --- | --- | --- |
| `character-style-nested-without-plus` | Info | 23908 |
| `unknown-custom-milestone` | Info | 2 |

The two Info codes are expected and realistic: `\w` inside `\wj` (Luke) and
inside `\qs` (Psalms) is what unfoldingWord's aligned texts write too, and
`\zaln-s`/`\zaln-e` are custom milestones no stylesheet declares.

Both tables are the output of `usfm parse`, which since ticket 19 is the
facade's — the parser's diagnostics *and* `usfm_semantic`'s. Ticket 23 added
four verse- and chapter-order warnings to the second half, and the counts
above are unchanged by them:

| Code | Severity | `web/` | `synthetic/` |
| --- | --- | --- | --- |
| `duplicate-verse-number` | Warning | 0 | 0 |
| `verse-out-of-order` | Warning | 0 | 0 |
| `duplicate-chapter-number` | Warning | 0 | 0 |
| `chapter-out-of-order` | Warning | 0 | 0 |

A published Bible is the case these checks are quiet on, and the WEB is one:
every chapter of all 86 books numbers its verses once, upwards, including the
deuterocanon, whose Greek Esther and Daniel additions are the obvious place
for a chapter to repeat. (Verse ranges do occur — five of them, all in
Sirach, `\v 15-16` through `\v 19-27` — and none overlaps its neighbours.) The corpus therefore still parses with **zero
errors**, which is the property the benches and the fuzz seeds rely on.

### Fixed: a `*` note caller

`web/78-1MA.usfm` line 22 (1 Maccabees 2:18) has `\f * \ft See 1 Maccabees
3:38; ...\f*`. USFM 3 allows any custom caller character, but the parser used
to report `missing-note-caller`, fall back to `+` and leave the `*` in the
note text, because a bare `*` lexes as `Kind::Star`. Ticket 07 taught
`parse_note` to accept that token as the caller, so the note now reads
`<note caller="*" style="f">` and the corpus parses with **zero** error
diagnostics.

### 3. It is not in the formatter's shape, but it is a fixed point

`usfm format --check tasks/benchmark/corpus/web/*.usfm` (ticket 26) reports
**78 of the 86 files**. The corpus is `usfx_to_usfm.py`'s output, not
`usfm_codegen`'s, and the two spell two constructs differently:

| | `usfx_to_usfm.py` (here) | `usfm_codegen` |
| --- | --- | --- |
| a note's content runs | `\f + \ft text\f*` | `\f + \ft text\ft*\f*` |
| `\cp` (once, `85-PS2.usfm`) | on its own line after `\c 151` | `\c 151 \cp 151` |

Both are ticket 25's canonical spellings — the writer emits the one spelling
of each construct that a reparse cannot read two ways — and both parse to the
same tree as what is committed here, which is why the round-trip test
(`crates/usfm_codegen/tests/roundtrip.rs`) passes over all 86 files. So
nothing here is regenerated to match the writer: the converter's output is the
input the benches and fuzz seeds are measured on.

What does hold is the property a formatter is judged by. After one
`usfm format --write` pass over a *copy* of `web/`, `usfm format --check`
exits 0 on all 86 files and prints no diagnostic: one pass reaches the fixed
point.

## File classes

Ticket 04's benches report throughput per class and over the whole corpus.

| Class | Path | Files | Size |
| --- | --- | --- | --- |
| plain | `web/` | 86 | 5354.6 KB |
| attributes-heavy | `synthetic/attributes-heavy/` | 3 | 3940.2 KB |
| alignment-heavy | `synthetic/alignment-heavy/` | 1 | 3792.1 KB |
| note-heavy | `web/71-WIS.usfm` (already in `web/`) | 1 | 78.3 KB |
| | **total, not double-counting note-heavy** | **90** | **13086.8 KB (12.8 MB)** |

Per file:

| File | plain | attributes-heavy | alignment-heavy |
| --- | --- | --- | --- |
| `01-GEN.usfm` | 200.6 KB | 1387.2 KB | — |
| `19-PSA.usfm` | 254.6 KB | 1596.2 KB | — |
| `43-LUK.usfm` | 144.5 KB | 956.8 KB | 3792.1 KB |

attributes-heavy takes three books: the longest Old Testament prose book
(Genesis), the Psalms (poetry: 2508 `\q` plus 2969 `\q2` in 254.6 KB, the
densest paragraph markup in the corpus) and the longest New Testament book
(Luke, which is also the only one of the three with `\wj` and `\x`).

alignment-heavy takes **Luke alone**. Alignment markup expands the source
about twenty-sixfold, so three books came to 15.6 MB — too much repository
weight for one benchmark class when the class measures markup density rather
than book variety. The New Testament is where unfoldingWord's alignment shape
comes from, and Luke at 3.8 MB is already an order of magnitude past anything
else the benches read. `synthesize.py` carries the per-class book list, so a
regeneration reproduces exactly what is committed.

**note-heavy needs no synthetic file.** Wisdom of Solomon is the densest book
in the corpus at **3.42 footnotes per KB** (268 `\f` in 78.3 KB), ahead of
1 Esdras (2.78), 2 Maccabees (2.35) and Sirach (1.20); for comparison the
whole corpus averages 0.47. It also carries 279 `\fqa` and 108 `\add`, so it
exercises note-internal character runs as well as the note path itself.

### What the synthetic files are for

No public project file with word-level attributes or alignment milestones is
available under a licence we can commit — unfoldingWord's en_ult and en_ust
are CC BY-SA 4.0 but live on `git.door43.org`, which this environment cannot
reach. So both classes are generated from the WEB by `tools/synthesize.py`,
in the markup shape unfoldingWord's aligned texts use:

```
\w In|lemma="in" strong="H3448"\w*

\zaln-s |x-strong="G03170" x-lemma="since" x-morph="Gr,V,PAA,,,NMS," x-occurrence="1" x-occurrences="1" x-content="since"\*\w Since|x-occurrence="1" x-occurrences="1"\w*\zaln-e\*
```

Strong's numbers are `H` for the Old Testament and `G` for the New. About one
alignment group in six spans two words (15.4% of the 21173 groups in Luke),
the way a real alignment binds two English words to one original-language
word. The words — and so the byte volume and the token mix the parser sees —
are real; the lemmas, Strong's numbers and morphology are not, and nothing
here should be mistaken for a translation resource.

Only verse text is wrapped: identification lines, headings, chapter lines,
Hebrew titles (`\d`) and the inside of notes are left as they are, as a real
aligned text leaves them. Each paragraph stays on one line, as in `web/`, so
the only difference between the three classes is markup density.
