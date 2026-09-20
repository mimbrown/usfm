# 32. Document symbols, completion and code actions

Status: resolved
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

## Answer

Landed in `e304772` (PR #40, 2026-09-20). Three more capabilities, each a module of
pure functions with unit tests and the wiring in `main.rs`:

- `documentSymbolProvider` (`symbols.rs`): one walk, not `ReferenceIndex`
  — the index reads chapters from the top-level blocks only, and a
  `\periph` division runs to the next `\periph` or `\id`, so a front-matter
  book lost its chapters; the walk also sees sidebars and periphs, which
  the index does not model. Book `MODULE` > chapter `NAMESPACE` > verse
  `KEY`, sidebar/periph `OBJECT`; `selection_range` the marker, `range` the
  content up to the next start or the container's end; nested by
  containment, overlaps become siblings.
- `completionProvider` with `\` as trigger (`completion.rs`): markers from
  `document.style_sheet()` with `\Name` as detail and `\Description` as
  documentation; `KEYWORD` for paragraph markers, `SNIPPET` closing
  character, note and milestone markers (`nd $1\nd*$0`); the edit range
  starts after the `\`, so a bare `\` and a `\n` prefix both complete.
  The filter is ticket 20's rule, now exposed as
  `usfm_semantic::placement::check(sheet, rule, parent) -> Placement`
  (`Listed`/`NotListed`/`NotAllowed`; the crate's own `placement_verdict`
  is two lines over it, so the check and the filter are one
  implementation); only the Error half filters, so `\fq` is not offered
  outside a note while advisory placements stay.
- `codeActionProvider`, kind `quickfix` (`actions.rs`): all five codes the
  ticket names, matched by code and range against the request's
  diagnostics and computed from the span and the source: delete an unknown
  marker (and the spaces after it), close an unclosed character style at
  the end of its last child (the diagnostic's span is the opener; the
  node's span runs to wherever it closed), insert ` +` for a missing note
  caller, quote an unquoted attribute value (skipped if it holds a `"`),
  delete an empty milestone attribute list's `|`. Each action's test
  applies the edit, re-parses, and asserts the code is gone and no code
  gained.

`tests/lsp.rs` gains one request of each kind over the wire. 55 unit tests
in the binary, 1 in `usfm_semantic::placement`. Gate exit 0.
`vscode/README.md` has the manual steps for the outline, completion and
quick fixes. No `completionItem/resolve` or `codeAction/resolve`.
