# 09. sillsdev/machine.py USFM fixtures (`\fe`, `\rq`, broken projects)

Status: ready-for-agent
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
