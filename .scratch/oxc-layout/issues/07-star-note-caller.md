# 07. A `*` note caller is lexed as a closing star

Status: resolved
Milestone: M2
Blocked by: 03

Found by ticket 03 on legitimate input: `tasks/benchmark/corpus/web/78-1MA.usfm`
line 22 (1 Maccabees 2:18) has `\f * \ft See 1 Maccabees 3:38 …\f*`. USFM 3
allows any custom caller character; the parser reports `missing-note-caller`,
assumes `+`, and leaves the `*` in the note text (`<note caller="+">* <char
style="ft">…`).

Cause: `usfm_parser/src/lexer/mod.rs` (around line 209) lexes a bare `*` as
`Kind::Star`, and `parse_note` takes the caller through `Cursor::eat_word`,
which accepts only `Kind::Word`. `+`, `-`, `a`, `?` and `†` all work; only `*`
fails. A `*` that does not follow a marker name cannot be a closing marker.

- Accept `Kind::Star` as a caller in `parse_note` (or lex `*` after `\f `/`\x `
  as a word); do not weaken closing-marker detection elsewhere.
- Test in `usfm_parser/tests/recovery.rs` (a passing case, plus the existing
  `missing-note-caller` case still failing for a genuinely missing caller).
- Rerun the corpus check in `tasks/benchmark/corpus/README.md` and update its
  diagnostic table: the corpus should then parse with zero errors.

Done when the corpus reports zero error diagnostics and the gate is green.

## Answer

Landed in `5a6e429` (PR #11, 2026-09-19). `ParserImpl::eat_note_caller` accepts a
`Kind::Star` as the caller (and a word glued to it, so `*abc` reads like
`+abc`); the lexer is unchanged. Tests: `recovery.rs::star_note_caller`,
`star_note_caller_with_trailing_word`, `usx_text.rs::star_note_caller_is_written`;
`missing_note_caller` still fails for a genuinely missing caller. The corpus
now parses with zero diagnostics over `web/` (README updated).
