# 10. Real word-aligned USFM from usfm-js (licence call)

Status: ready-for-human
Milestone: M2

Assessed by ticket 03. https://github.com/unfoldingWord/usfm-js
`__tests__/resources/` holds 126 `.usfm` (14 MB), 32 with `\zaln-s`; tcdocs
already vendors 77 of them as `usfmjsTests`. Not in tcdocs: `large.usfm`
(3.8 MB), `45-ACT.ugnt.usfm` and `45-ACT.ugnt.oldformat.usfm` (1.6 MB): the only
real, non-synthetic word-aligned USFM reachable from the cloud environment
(git.door43.org, where unfoldingWord's `en_ult`/`en_ust` live, is blocked by the
network policy; there is no GitHub mirror).

The package declares ISC in `package.json` and has no root LICENSE file; the
content is unfoldingWord UGNT/ULT scripture, CC BY-SA 4.0 upstream. Whether
that can be committed here, and under which notice, is Michael's call.

Question for Michael: may `45-ACT.ugnt.usfm` (and/or an `en_ult` book supplied
out of band) be committed under `tasks/benchmark/corpus/aligned/` with a
CC BY-SA 4.0 notice, replacing the synthetic `alignment-heavy` class? If yes,
this becomes a ticket to vendor it and rerun the benchmarks; if no, the
synthetic class stays and this ticket is `wontfix`.
