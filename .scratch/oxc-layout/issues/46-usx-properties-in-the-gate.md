# 46. The two USX properties in the gate

Status: ready-for-agent
Milestone: M7
Blocked by: 45

M7's first two exit criteria, as `usfm_tests` steps beside `--baseline` and
`--roundtrip`, with the same known-list semantics (an unlisted failure is a
regression, a listed case that passes is a stale entry, both fail, and every
entry names the bug):

- `--usx-read <known>`: for every case the harness compares today, read
  `origin.xml`, write it back with `usfm_usx`, compare with the harness's own
  comparison (patches applied, whitespace rules as they are).
- `--usx-roundtrip <known>`: read, `usfm_codegen::to_usfm_string`, parse,
  write USX, compare the same way. The property is on the USX side, so the
  canonicalisation `usfm_codegen` does is invisible to it.

Both steps go in `scripts/gate.sh` and `.github/workflows/ci.yml` after
`--roundtrip`, each with an empty known file under `tasks/conformance/`.
`--show <name>` prints the read tree's diagnostics too. Every failure is
fixed in the crate that is wrong before the list is emptied; a reference
quirk is a patch in `tcdocs-patches/` under its README's rules, never a
reader special case.

Done when both steps are in the gate with empty known lists and CLAUDE.md's
conformance paragraph describes them.
