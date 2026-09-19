# 09. sillsdev/machine.py USFM fixtures (`\fe`, `\rq`, broken projects)

Status: resolved
Milestone: M2
Blocked by: 06

Assessed by ticket 03. https://github.com/sillsdev/machine.py is MIT
(© 2022 SIL International). `tests/testutils/data/usfm/` (14 `.SFM`, 76 KB) and
`samples/data/` (700 KB) are hand-written Paratext-shaped projects using `\rq`,
`\fe` endnotes and `\w keyword|a special concept\w*`, plus deliberately broken
projects (`invalid_id/`, `mismatch_id/`). The WEB corpus has no `\fe` or `\rq`.

- Vendor the small `tests/testutils/data/usfm/` set with the MIT text and a
  NOTICE.md entry; skip `samples/data/` unless it adds a marker the small set
  lacks.
- Same role decision per file as ticket 08 (recovery test, snapshot, fuzz seed).
- Also look at the C# `sillsdev/machine` and `sillsdev/silnlp` test data for
  the same kind of fixture; ticket anything worth adding.

Done when the vendored files have roles, tests pass, and the gate is green.

## Answer

Landed via PR #13 (2026-09-19). `tests/fixtures/machine-py/` (MIT, commit
`e2af2c8`): the five `Tes` books, `custom.sty`, and the two `07JDG.SFM`
project-mismatch files (seeds and a README note only: a parser cannot see a
filename mismatch). `41MATTes.SFM` is a whole-file snapshot with its eight
codes asserted, plus variant tests for the shapes no existing test had
(`\w*` with no opener, a word-looking unknown marker, content after `\esbe`);
Leviticus (`\v 55b`), Mark (front matter only) and the empty John file are
snapshots too. `custom.sty` is covered by a test that extends the default
sheet through the existing `StyleSheet` API. 14 fuzz seeds. `samples/data`
adds no marker the corpora lack, so it was skipped.

One parser fix: content after `\esb`/`\esbe` is checked for placement
against the implicit `\p` that holds it, not against the sidebar marker
(bogus `marker-not-listed-here`).

Follow-ups: a duplicated or out-of-order verse number is reported nowhere;
it is a document check for `usfm_semantic` (M4), noted in the spec's open
list. An empty file reports nothing, not `missing-id`; left as is.
