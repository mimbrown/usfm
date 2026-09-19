# 22. `ReferenceIndex` moves to `usfm_semantic`

Status: ready-for-agent
Milestone: M4
Blocked by: 19

`usfm_ast::reference` (`ReferenceIndex`, `VerseRef`, `document.reference_index()`)
is a derived index, not a node: the ADR puts it in `usfm_semantic`.

- `git mv crates/usfm_ast/src/reference.rs crates/usfm_semantic/src/reference.rs`;
  `ReferenceIndex::new(&Document)` replaces `Document::reference_index()`,
  which is removed (the AST freeze is about node shape, not this helper; say
  so in CLAUDE.md's Traversal API paragraph). Callers: `tasks/benchmark`'s
  `reference_index` group, tests, the CLI if any.
- `ReferenceIndex` also becomes what ticket 23's verse checks walk, so give
  it what they need: chapters in document order with their verses in document
  order and each verse's span.

Done when the gate is green and `usfm_ast` has no `reference` module.
