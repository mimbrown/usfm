# Aligned corpus: unfoldingWord's Acts, from usfm-js

Real word-aligned USFM (ticket 10): the only whole book of it reachable from
the cloud environment, where `git.door43.org` — home of unfoldingWord's
`en_ult` and `en_ust` — is refused by the network policy. It replaced the
synthetic `alignment-heavy` Luke as the benchmark's aligned class; see
`../README.md`, "File classes".

| | |
| --- | --- |
| Source | https://github.com/unfoldingWord/usfm-js, `__tests__/resources/` |
| Commit | `0ecae6f169f912e1c30da6f519a7724d31dcd841` (`master`, 2026-05-23) |
| Fetched | 2026-09-26 |
| Licence | the scripture text: **CC BY-SA 4.0**, unfoldingWord; `LICENSE` here is the licence's legal code. usfm-js's own code is ISC (its `package.json`) and none of it is here |

## The files

| File here | Upstream file | Bytes upstream | Bytes here | What it is |
| --- | --- | ---: | ---: | --- |
| `45-ACT.ult.usfm` | `large.usfm` | 3 883 044 | 3 921 324 | Acts in the **unfoldingWord Literal Text** (ULT), English, every word inside `\zaln-s`/`\zaln-e` alignment milestones that point into the Greek below: 19 140 alignment groups, 25 284 `\w` words, 28 chapters, 1 006 verses. `\id ACT EN_ULT en_English_ltr Sat Jun 23 2018 10:10:55 GMT-0400 (EDT) tc` — a translationCore export from 2018 |
| `45-ACT.ugnt.usfm` | `45-ACT.ugnt.oldformat.usfm` | 1 613 122 | 1 613 434 | Acts in the **unfoldingWord Greek New Testament** (UGNT), the original-language text the ULT is aligned to: 18 474 `\w` words with `lemma`, `strong`, `x-morph` and often `x-tw`, 156 `\k-s`/`\k-e` key-term milestones, 163 `\p`, 4 footnotes. `\id ACT unfoldingWord® Greek New Testament` |

The file names follow `../web/`'s `NN-BBB` book numbering, with the text
named after the dot. Neither header carries a copyright or attribution line of
its own (no `\rem`, and `\ide` is `UTF-8` in both); the `\id` lines above are
the whole of what the files say about themselves.

`SHA-256` of the upstream files, for anyone regenerating:

```
3b1d5d5dad4a41ce29a0131c2533f18aa38074aa4b760cb1c64f1a237c91a70d  large.usfm
e3ec9682e158738f97aff29fd25c9c4e1c20d27feb4956154740c8e8a4fc2de9  45-ACT.ugnt.oldformat.usfm
```

### Not taken

- `45-ACT.ugnt.usfm`, which ticket 10 named, is a **130-byte placeholder** at
  this commit ("Placeholder ... waiting for actual content. Previous content
  was not actually in the new format which caused testing trouble."). The
  `oldformat` file is the real one.
- Of the 32 upstream files with `\zaln-s`, 23 are already in tcdocs as
  `usfmjsTests` (under their upstream names, some with edits), and of the
  other nine only `large.usfm` is a whole book of unfoldingWord text. Seven
  are excerpts of 13 KB or less (`psa_140_8.*`,
  `usfm-body-testF-paragraph-*`, `misc/RU-EST-UTF8_BOM.usfm`,
  `filter/mat-4-6.aligned.raw.usfm`), and the ninth,
  `phm.hi.alignment.oldformat.usfm` (Philemon, 75 KB), is a whole book but
  the Hindi IRV, which is not unfoldingWord's text, so the answer on ticket
  10 does not cover it.

## One change: the milestones are closed

Both upstream files are in what usfm-js calls the **old format**: a
milestone's attribute list runs to the end of its line and is never closed.

```
\zaln-s |x-strong="G35880" x-lemma="ὁ" x-morph="Gr,EA,,,,AMS," x-occurrence="1" x-occurrences="1" x-content="τὸν"
\w The|x-occurrence="1" x-occurrences="1"\w*
\zaln-e\*
```

USFM 3 requires `\zaln-s |...\*`, which is how usfm-js has written them
since; tcdocs judges the old shape `validated=fail`
(`usfmjsTests/acts_1_milestone.oldformat`, `tit1-1_alignment.oldformat`).
Read as it is, `large.usfm` reports 19 140 `unexpected-pipe` Errors and
drops every milestone, keeping its attributes as verse text — a benchmark of
the recovery path, not of alignment. So the files here are the upstream files
passed through `../tools/usfmjs_oldformat.py`, which appends `\*` to every
`\zaln-s |` or `\k-s |` line whose attribute list reaches the line's end
(19 140 in the ULT, 156 in the UGNT) and checks that nothing else changed:
delete every `\*` from both sides and the texts are equal. Everything else —
the `\s5` chunk markers, the paragraph markers at the ends of lines, the line breaks — is as upstream
wrote it. Under CC BY-SA 4.0 this is an adaptation, and this section is the
indication of changes the licence asks for.

```bash
git clone https://github.com/unfoldingWord/usfm-js /tmp/usfm-js
git -C /tmp/usfm-js checkout 0ecae6f169f912e1c30da6f519a7724d31dcd841
python3 ../tools/usfmjs_oldformat.py /tmp/usfm-js/__tests__/resources/large.usfm 45-ACT.ult.usfm
python3 ../tools/usfmjs_oldformat.py /tmp/usfm-js/__tests__/resources/45-ACT.ugnt.oldformat.usfm 45-ACT.ugnt.usfm
```

## Licence

The ULT and the UGNT are published by unfoldingWord under the Creative
Commons Attribution-ShareAlike 4.0 International licence. Attribution: **unfoldingWord®**,
https://www.unfoldingword.org/ult and https://www.unfoldingword.org/ugnt.
`LICENSE` is the licence's legal code,
https://creativecommons.org/licenses/by-sa/4.0/legalcode.txt, copied
verbatim — fetched from Creative Commons' own `cc-legal-tools-data`
repository (commit `3edadd4e29295ef7c74d84e9949af826122066a8`,
`docs/licenses/by-sa/4.0/legalcode.txt`, the file creativecommons.org
serves), because this environment's proxy refuses `creativecommons.org`.
The rest of this repository stays MIT; the licence applies to these two files
and to the fuzz seeds copied from them. See `NOTICE.md` at the repository
root.

## Fuzz seeds and round trip

`tasks/fuzz/seed.sh` copies both files into every fuzz corpus as
`usfm-js__45-ACT.ult.usfm` and `usfm-js__45-ACT.ugnt.usfm`, truncated to
64 KiB like every long seed. `crates/usfm_codegen/tests/roundtrip.rs`'s
`the_benchmark_corpus_round_trips` reads this class beside `web/`.
