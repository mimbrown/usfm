# 05. Miri on the unsafe code

Status: resolved
Milestone: M2
Blocked by: 01

41 `unsafe` blocks in `lexer/source.rs`, 3 in `lexer/mod.rs`, 1 in `cursor.rs`,
plus `usfm_ast/src/string_parser.rs`. Run `cargo +nightly miri test` over the
lexer, whitespace and recovery suites (trim any test too slow for Miri). Every
block gets a `// SAFETY:` comment or is replaced with safe code; after 04, replace
any block whose removal costs under 2%.

Done when Miri is clean and is a CI job.

## Answer

Landed in `84ee98e` (PR #8, 2026-09-19). Miri was clean from the first run: none of the
27 unsafe uses (21 in `lexer/source.rs`, not 41; 3 in `lexer/mod.rs`, 2 in
`string_parser.rs`, 1 in `cursor.rs`) hid a bug. 26 of them are now safe code:
`Source` holds `&str` + byte offset instead of three raw pointers, at no
measurable cost on `lex` (153.7 vs 152.4–153.5 MiB/s interleaved). The one
left is `ParserImpl::src` (`cursor.rs`), where the safe slice costs 2.3–3.5%
on `parse/whole-corpus`, over the 2% line; it keeps a `debug_assert` of the
in-bounds and char-boundary invariant that every test and Miri run checks.

`scripts/miri.sh` (about 3 min 50 s) is the last gate step: `usfm_ast` lib,
parser lib minus the regex-bound `text_replacements` tests, and the
`whitespace`, `attributes`, `usx_text`, `verse_ends`, `spans` suites plus 11 of
76 `recovery` cases. CI installs nightly Miri in its own step;
`scripts/session-start.sh` installs it on a fresh VM. `docs/benchmarks.md` has
an "After ticket 05" section with the numbers and a warning: whole-binary
`lex` numbers move ±5–8% with codegen-unit placement, so M3 must compare
interleaved builds (or use `codegen-units = 1`).

Follow-ups, not done here:
- `parse_usx`, `parse_html` and `reference_index` tables in `docs/benchmarks.md`
  were not rerun after the lexer change; rerun all five at the M2 boundary.
