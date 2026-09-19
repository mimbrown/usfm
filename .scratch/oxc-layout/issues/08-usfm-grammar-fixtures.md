# 08. usfm-grammar's `autofix` and `bugfixes` fixtures as conformance cases and fuzz seeds

Status: ready-for-agent
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
