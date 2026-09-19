# 18. `\ts-s |\*`: an empty attribute list on a milestone

Status: ready-for-agent
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
