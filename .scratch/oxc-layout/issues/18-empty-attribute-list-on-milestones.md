# 18. `\ts-s |\*`: an empty attribute list on a milestone

Status: resolved
Milestone: M3

Found by ticket 08 in usfm-grammar's `autofix/fr-textTranslation-FR_TLX.txt`,
a real unfoldingWord-aligned file: 24 `empty-attribute-list` Errors, all for
`\ts-s |\*` (a translation-section milestone written with a pipe and no
attributes). The convention is common in unfoldingWord output; tcdocs has no
`|\*` at all, so nothing there constrains us.

- Decide the severity with the USFM 3 attribute grammar in hand (the spec
  says the list after `|` is attributes; an empty one is malformed but
  harmless). Proposal: keep `empty-attribute-list` as it is for character
  styles, and for a milestone with `|\*` report it at Warning, or at Info if
  Paratext accepts the file silently (check `docs` for what Paratext's
  checker says, if the local docs copy exists; otherwise say what was
  assumed).
- Test in `recovery.rs` (a variant on `empty_attribute_list`), USX unchanged
  (`<ms style="ts-s"/>`).

Done when the file parses with no Error diagnostics and the gate is green.

## Answer

Landed in `09f2ca4` (PR #22, 2026-09-19). A milestone with `|\*` now reports the new
`empty-milestone-attribute-list` (Warning, nothing dropped, same USX as
without the pipe); `empty-attribute-list` (Error) is the character-style
case only. Severity stays a property of the `Code`, hence a second code
rather than a per-emit severity. Tests: `recovery.rs::
empty_milestone_attribute_list` and a USX byte-equality test.
`fr-textTranslation-FR_TLX.txt` parses under `--strict` (24 warnings, 2
infos). Local USFM docs were absent; Warning was chosen from the spec's
grammar, not Paratext's checker.

Follow-up on ticket 20: an unknown milestone with `|\*` still reports nothing.
