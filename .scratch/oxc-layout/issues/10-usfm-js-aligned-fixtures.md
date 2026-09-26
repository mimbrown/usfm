# 10. Real word-aligned USFM from usfm-js (licence call)

Status: resolved
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

## Comments

2026-09-26, Michael (via the orchestrator): **yes, vendor them.** Commit the
usfm-js aligned files under `tasks/benchmark/corpus/aligned/` with the
CC BY-SA 4.0 notice the ticket describes (the scripture is unfoldingWord's
UGNT, CC BY-SA 4.0 upstream; usfm-js's own code is ISC), replacing the
synthetic `alignment-heavy` class as the benchmark's aligned input. The
synthetic generator and its other classes stay. Rerun the benchmarks so
`docs/benchmarks.md` has the new class's numbers, and add the files as fuzz
seeds and to the round-trip test's inputs like the other vendored corpora.

## Answer

2026-09-26. `tasks/benchmark/corpus/aligned/` (CC BY-SA 4.0: `LICENSE` is the
legal code, `README.md` the provenance, plus a `NOTICE.md` entry) holds
usfm-js at `0ecae6f169f912e1c30da6f519a7724d31dcd841`:
`large.usfm` as `45-ACT.ult.usfm` (the ULT, aligned) and
`45-ACT.ugnt.oldformat.usfm` as `45-ACT.ugnt.usfm` (the UGNT). The
`45-ACT.ugnt.usfm` this ticket named is a 130-byte placeholder upstream now,
and the only other aligned whole book not in tcdocs (`phm.hi.alignment`) is
the Hindi IRV, not unfoldingWord's, so neither was taken.

Both upstream files are usfm-js's old format (milestones never closed with
`\*`), which tcdocs judges `fail` and the parser reports as 19 140 + 156
`unexpected-pipe` Errors. Committed is upstream with every milestone closed,
by `tasks/benchmark/corpus/tools/usfmjs_oldformat.py`, which checks it
changed nothing else. The closed files show no parser bug; the recovery of the
unclosed shape is ticket 43 (`needs-triage`).

The benchmark class is `aligned`, replacing the synthetic `alignment-heavy`
(no longer committed; `synthesize.py --class alignment-heavy` regenerates it).
`docs/benchmarks.md`, "usfm-js aligned corpus (ticket 10)": `parse` reads the
real text at 90.2 MiB/s against the synthetic Luke's 88.4 in the same
sitting. Both books round-trip and are fuzz seeds (`usfm-js__*`).
