# 47. Real-world USX, and USX 2 (no `sid`/`eid`)

Status: ready-for-agent
Milestone: M7
Blocked by: 45

Every USX in the conformance roots was generated from USFM by one tool. Real
exports differ, and sillsdev/machine.py (MIT, already vendored for USFM under
`tasks/conformance/fixtures/machine-py/`) ships two kinds:

- `samples/data/WEB-DBL/release/USX_1/` — 1JN, 2JN, 3JN of the World English
  Bible as a DBL release writes it (USX 3.0, every word a
  `<char style="w" strong="…">`, pretty-printed inside paragraphs). The text
  is public domain; the files are the repo's, MIT.
- `tests/testutils/data/usx/Tes/release/USX_1/` — MAT and MRK, a USX 2.6
  test book that is malformed on purpose: a `<verse>` outside any `<para>`,
  `sid` on some verses and not others, a repeated and an out-of-order verse,
  a USX 2 `<figure file=… size=… ref=…>`, and an unknown `restore` style.

Vendor both under `tasks/conformance/fixtures/machine-py-usx/` (or beside
the existing machine-py fixtures if the commit is the same; the existing
ones are from `e2af2c8`, the files were checked at `615cd59`), with the
licence and a `NOTICE.md` entry. Snapshot each file's tree and diagnostics
the way `recovery.rs`'s `machine_py_*` tests do; the WEB books must read with
no Error and round-trip USX -> USFM -> USX.

USX 2: a file with no `eid` milestones reads to the verse and chapter ends
the parser would build (spec, "Verse ends"). Decide whether that shares
`OpenVerse`'s placement with the parser or mirrors it, and record why. The
test is M7's: strip every `eid` from every reference of both roots, read, and
compare with the unstripped read by `eq_ignoring_spans`. A file with some
`eid`s and not others (the Tes book) is the malformed case: say what it
reads to and pin it.

Done when the fixtures are vendored with their licence, the snapshots are
committed, and the stripped-`eid` test passes over every reference.
