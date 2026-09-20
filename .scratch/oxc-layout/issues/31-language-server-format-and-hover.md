# 31. Formatting and hover in the language server

Status: ready-for-agent
Milestone: M6
Blocked by: 30

- `textDocument/formatting`: `usfm_codegen::to_usfm_string` of the parse; one
  `TextEdit` replacing the whole document (a diff-based minimal edit set is a
  follow-up if the client flickers). Refuse with a `window/showMessage` when
  the parse has an Error-severity diagnostic (the same rule as
  `usfm format --write`), since a repaired tree is not the author's text.
  `formatOnSave` then works in VS Code.
- `textDocument/hover` on a marker: the stylesheet's `Name` and `Description`
  for the marker under the cursor (`StyleRule` has them), plus its
  `OccursUnder` list in one line; on a `\v`/`\c`, the reference (`GEN 1:1`)
  from the tree. Locate the node by span: walk with `Cursor` or the visitor
  to the innermost node whose span contains the offset.
- Tests: unit tests for the hover text of `\p`, `\w` and `\v`; the stdio
  integration test from ticket 30 gains a `formatting` request and a `hover`
  request.

Done when the gate is green and both work in VS Code (manual check
described).
