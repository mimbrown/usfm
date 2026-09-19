# 08. usfm-grammar's `autofix` and `bugfixes` fixtures as conformance cases and fuzz seeds

Status: resolved
Milestone: M2
Blocked by: 06

Assessed by ticket 03. https://github.com/Bridgeconn/usfm-grammar is MIT
(© 2021 Bridge Connectivity Solutions). Its `tests/` mirrors ten of tcdocs'
twelve categories, which we already run; two are new to us:

- `tests/autofix/` (12 files, 200 KB): deliberately malformed inputs (`\id`
  with no or a wrong book code, no space before a chapter or verse number, a
  space inside a chapter number, `\b`/`\s3`/`\s5` with no `\p`, an empty
  marker, a stray `\` in text). These are recovery shapes; each wants a `Code`
  and a `recovery.rs` test.
- `tests/bugfixes/` (16 case directories, 348 KB): `attrib-for-tl`,
  `custom-attrib-hyphens`, `empty-table-cell`, `list_milestones`, `liv1`,
  `marker-ex`, `multiple-sr`, `nestedchar-footnote`, `new-marker-ipc`/`-ta`/`-wl`,
  `new-vid-milestone`, `q4`, `rem_with_char`, `tcc`, `thc`.

- Vendor only those two directories (not the whole 30 MB `tests/`) under
  `tasks/conformance/` once M3 creates it, or under `tests/fixtures/usfm-grammar/`
  until then, with the MIT text and a NOTICE.md entry (scripture excerpts
  belong to their publishers, same wording as for tcdocs).
- Decide per file: a `recovery.rs` test (with its `Code`), a snapshot, or a
  fuzz seed only. usfm-grammar's expected outputs are its own JSON, not USX, so
  they are not oracle files; our expectation is the parser's diagnostics.
- Add all of them to the fuzz corpus from ticket 06.

Done when every vendored file has a decided role, the new recovery tests pass,
and the gate is green.

## Comments

2026-09-19, orchestrator: `tests/bugfixes/` is in tcdocs' own layout
(`metadata.xml` with `<validated>`, `origin.usfm`, `origin.xml`), so the 13
cases that have an `origin.xml` can run through the existing `usfm_tests`
harness as a second root rather than as hand-written tests; the 3 without one
(`multiple-sr`, `nestedchar-footnote`, `rem_with_char`) and the 12 `autofix`
inputs are recovery/snapshot material. Vendor under `tests/fixtures/` for now
(M3 ticket 17 moves `tests/` to `tasks/conformance`). Upstream commit
`4ee1b91c9b725f7f5be3654805ab6447b111da58`, MIT, © 2021 Bridge Connectivity
Solutions.

## Answer

Landed via PR #12 (2026-09-19). `tests/fixtures/usfm-grammar/` (MIT, commit
`4ee1b91c`) holds `bugfixes/` (16 cases, run by the `usfm_tests` harness as a
second root, category `usfm-grammar/bugfixes`, 16 / 0) and `autofix/` (13
inputs, covered by `recovery.rs` tests: 8 already-diagnosed shapes get a
variant test each, 4 are valid USFM and get a snapshot, none was silently
accepted). All 29 inputs are fuzz seeds.

Parser fixes on the way: `tl`/`wl` default `lang`, `vid` default `ref`;
`usfm-extra.sty` (appended to Paratext's unchanged `usfm.sty` by `build.rs`)
adds `\ipc`, `\wl`, `\ta` and `\xta` under `\ex`, all from usx.rnc; `\tcc3`
writes `style="tcc3" align="center"`; and `BookCode::Other([u8; 3])` keeps a
well-formed but unlisted code (`\id TST`) as USX allows, reported as
`unlisted-book-code` (Warning), while `unknown-book-code` (Error) is now only
for a code that matches nothing. Harness: a `pass` case with no `origin.xml`
must parse with no error diagnostic; for this root ASCII whitespace is
collapsed on both sides and a truncated `<usx version>` is restored. Five
patches under `tests/tcdocs-patches/usfm-grammar/bugfixes/`, each with its
rationale (two reference files are not valid USX; three place a verse end
differently from tcdocs, whose placement we follow).

Follow-ups: ticket 18 (`\ts-s |\*`, an empty attribute list on a milestone,
is an unfoldingWord convention we call an Error).
