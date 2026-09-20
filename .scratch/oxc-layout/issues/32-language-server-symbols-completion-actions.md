# 32. Document symbols, completion and code actions

Status: ready-for-agent
Milestone: M6
Blocked by: 31

- `textDocument/documentSymbol`: chapters as symbols with their verses as
  children, from `usfm_semantic::ReferenceIndex` (spans -> ranges); sidebars
  and periphs as symbols too. This is the outline view in VS Code.
- `textDocument/completion` after a `\`: marker names from the document's
  stylesheet, filtered by the parent the cursor is in when the stylesheet's
  `OccursUnder` lists one (the semantic scope rules from ticket 20, exposed
  by `usfm_semantic` as a small `placement::parent_at(document, offset)` or
  reimplemented locally, say which), with the `Name` as detail and a snippet
  that adds the closing marker for a character style (`\nd $1\nd*`).
- `textDocument/codeAction` for the parser repairs that have an obvious
  edit: `unknown-marker` (delete the marker), `character-style-not-closed`
  (insert the closer), `missing-note-caller` (insert `+ `),
  `attribute-value-not-quoted` (quote it), `empty-milestone-attribute-list`
  (delete the `|`). Each action's edit is applied in a test and the
  re-parse must no longer report that code.
- The stdio integration test gains one request of each kind.

Done when the gate is green.
