# 47. Real-world USX, and USX 2 (no `sid`/`eid`)

Status: resolved
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

## Answer

Landed 2026-09-27 (PR below).

- **Vendored** `tasks/conformance/fixtures/machine-py/usx/`: `WEB-DBL/{1JN,2JN,3JN}.usx`
  and `Tes/{MAT,MRK}.usx`, byte-identical at `e2af2c8` (the existing
  fixtures' commit) and at `615cd59`, so they sit beside the existing
  fixtures under the same `LICENSE`; README and `NOTICE.md` updated. The
  directory's `.SFM` walkers do not pick them up.
- **The WEB books** read with no diagnostic at all, and round-trip USX ->
  USFM -> USX: the parse of the written USFM equals the read tree, and its
  USX matches the file under the harness comparison with usfm-grammar's
  whitespace collapse (DBL indents inside paragraphs). 2 and 3 John are
  snapshotted whole; 1 John is held to no diagnostics and the round trip (a
  whole snapshot would be 227 KB).
- **Tes MAT**, pinned whole: the verse outside a paragraph is an implicit
  `\p` with `content-outside-paragraph`; the repeated and out-of-order
  verses are read as written (`usfm_semantic`'s to report); USX 2's
  `<figure file size ref>` is `\fig` with `src`/`size`/`ref`. The ticket was
  wrong about `restore`: `\restore` is in Paratext's sheet, so it reads with
  nothing to say. Tes MRK reports nothing.
- **USX 2:** a file with no `eid` gets its ends where the parser would put
  them, silently. **Mirrored, not shared**: the parser places each end while
  building the container, from state only a parse has (its open-marker
  stack, the paragraph's text type, the `\esbe` line pushed before it is
  read); an output crate may not depend on the parser; and the shared
  alternative, a pass over the finished tree, is the path arithmetic plan D4
  took out of the parser. The stripped-`eid` test keeps the two in step: all
  271 references of both roots, stripped, read to the unstripped tree, with
  one known entry, `usfmjsTests/invalid` (a `fail` case the harness never
  compares), whose reference ends `\v 11` before a verse both the reader and
  the parser drop as malformed — the reference's placement differs, not the
  reader's. A hand-written USX 2 file (heading, verse in a character style,
  sidebar, table cells) also equals `usfm::parse` of the same USFM.
- **Mixed files** (some `eid`s, not all, as Tes MAT): a file with any `eid`
  closes its own verses, so each end it lacks is built the same way and
  reported with a new Warning, **`usx-verse-end-missing`**. The reader's
  diagnostics are now sorted by position, as the parser's are.
- Harness: `--usx-read` drops the built ends before writing when the
  reference has no `eid` (only `advanced/complex`), as the harness parses
  such a case.
