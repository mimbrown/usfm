# 03. Benchmark corpus

Status: resolved
Milestone: M2

Pick and commit the corpus under `tasks/benchmark/corpus/`. Proposal: the World
English Bible USFM (public domain, 66+ files, ~5 MB), plus one file heavy in
`\w …|…\w*` attributes and one heavy in notes, since those are the hot paths a
plain text does not exercise. Record source URL and date in a README.

Decided 2026-09-19 (Michael): WEB is acceptable. No real project file for now, so
generate the attribute-heavy and alignment-heavy files synthetically: a small
deterministic generator (seeded, committed, output committed) that wraps WEB words
in `\w …|lemma="…" strong="…"\w*` and `\zaln-s`/`\zaln-e` milestones in the shape
unfoldingWord's aligned texts use. Also look for other public USFM suites worth
adding to `tasks/conformance` or the fuzz seeds (usfm-grammar, usfm-js, Paratext
sample projects, unfoldingWord ULT/UST) and ticket any that are, with licence.

## Answer

Landed via PR #6 (2026-09-19). `tasks/benchmark/corpus/`: the whole WEB as 86
USFM files (5.4 MB), `synthetic/attributes-heavy/` (GEN, PSA, LUK; 3.9 MB) and
`synthetic/alignment-heavy/` (LUK; 3.8 MB), both tools committed, output
byte-identical on regeneration (checked by the orchestrator too). Note-heavy
class is `web/71-WIS.usfm` (3.42 footnotes per KB), no synthetic file needed.

Route change from the ticket text: ebible.org is refused by the cloud
environment's network policy (proxy 403), so the WEB comes from its USFX
rendering in `seven1m/open-bibles` (commit `f257a35`), converted by
`tools/usfx_to_usfm.py`. Same publisher, same public-domain text; the README
records the verification counts (verses, chapters, notes, `\wj` all exact
through USFM and USX).

Corpus diagnostics: one error over 12.8 MB, a `*` note caller in 1 Maccabees
2:18 that the parser rejects on legitimate input; ticket 07. Other public
suites assessed and ticketed: 08 (usfm-grammar `autofix`/`bugfixes`, MIT),
09 (machine.py fixtures, MIT), 10 (usfm-js aligned files, licence call for
Michael, `ready-for-human`).
