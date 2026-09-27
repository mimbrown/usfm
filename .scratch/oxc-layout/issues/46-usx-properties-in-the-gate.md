# 46. The two USX properties in the gate

Status: resolved
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

## Answer

Landed 2026-09-27 (PR below). `tasks/conformance/src/usx_properties.rs`
defines both properties once — `compared_cases` (an input, a reference, and
`run()` is `Passed`), `check_read`, `check_roundtrip` — comparing through the
harness's own patching, `include_vid` rule, `normalize_for_comparison` and
`compare_xml`. The runner gained `--usx-read`, `--usx-roundtrip` and their
`--write-…-known` forms; the known-list code is shared by all three lists
(`KnownList`, `gate_known`), with unchanged semantics. `--show` prints the
reader's diagnostics and whether each property holds. Both steps are in
`scripts/gate.sh` after `--roundtrip`, **228 / 228 each, both known lists
empty**; nothing failed, so no crate outside the harness changed (a
deliberately broken codegen step made 179 cases fail, so the step bites).
`usx_reader.rs` lost the write-back test, which `--usx-read` now owns.

Not covered, and said so in CLAUDE.md: the 43 references the harness never
compares (`fail` cases whose parse reports an Error). Ungated, `--usx-read`
fails 32 of them and `--usx-roundtrip` 33, on what the tree does not model —
Paratext's `status="invalid"`, `<unmatched>`, a `sid` with no book, `vid` on
`\zaln-s` paragraphs. One is worth a look if anyone wants it:
`paratextTests/MissingColumnInTable` fails only the round trip, with an extra
verse end inside a `<cell>`.
