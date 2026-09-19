# 03. Benchmark corpus

Status: ready-for-agent
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
