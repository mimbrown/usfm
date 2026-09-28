# 62. Scoped text edits, and inserting next to a node

Status: needs-triage
Milestone: after M7 (publishing foundation)

render's text edits are scoped:
- only inside given styles, or only outside notes;
- only after the first `\c`;
- some insert text next to a verse end or a note, which render does today
  with regexes over its serialised JSON (`render:lib/edits.ts`,
  `importAppContent.ts:364-367`).

`TextReplacement` has no scope. The shape would be a small selector
(style, ancestor, note or not, after a node) and an insert-before/after
operation. Triage when render's port shows which of these it actually
uses.
