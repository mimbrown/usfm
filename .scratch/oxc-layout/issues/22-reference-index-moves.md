# 22. `ReferenceIndex` moves to `usfm_semantic`

Status: resolved
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

## Answer

Landed via PR #26 (2026-09-19). `reference.rs` moved by `git mv` into
`usfm_semantic` (`ReferenceIndex::new(&Document)`, `ChapterRef`, `VerseRef`;
lifted to the facade root); `usfm_ast` keeps only `NodePath`, now in
`cursor.rs` beside what uses it, and `Document::reference_index()` is gone.
The moved tests kept the hand-built fixture through a new off-by-default
`testing` feature on `usfm_ast`. Document order was already what ticket 23
needs and is now pinned: a duplicated verse or chapter number appears twice
in order, `verse(c, v)`/`chapter(n)` return the first, and a verse before
the first `\c` is kept with `chapter() == None`.
