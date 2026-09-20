# 31. Formatting and hover in the language server

Status: resolved
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

## Answer

Landed via PR #39 (2026-09-20). The server now advertises
`documentFormattingProvider` and `hoverProvider` beside full sync and
UTF-16 positions.

- `textDocument/formatting` (`format.rs`): `usfm_codegen::to_usfm_string`
  of the stored text's parse, with the same trailing newline as `usfm
  format`, as one `TextEdit` over the whole document; `[]` when the text
  is already the writer's; `null` plus a `window/showMessage` warning when
  the parse has an Error-severity diagnostic (the rule of `usfm format
  --write` without `--force`; `null` rather than `[]` because `[]` means
  "already formatted"). `rangeFormatting` and a minimal diff are not done.
- `textDocument/hover` (`locate.rs`, `hover.rs`): one `NodeRef::descendants`
  walk finds the innermost node whose span holds the offset and records the
  `\id`/`\c`/`\v` passed on the way (`Location`, which ticket 32 reuses).
  A styled node answers with the stylesheet's `Name`, `Description` and
  `OccursUnder` for its marker, over the node's whole span; a verse or
  chapter, or text inside one, answers with the reference (`**GEN 1:1**`);
  `\id` with the book name; a table cell spells its marker from
  `header`/`alignment`/`column`. `ReferenceIndex` was not used: the walk
  already has the answer.
- `convert::offset` is the UTF-16 inverse of `position` (columns past the
  line end clamp to the line, a position inside a surrogate pair rounds
  back to the character), and `whole_document` the edit's range.

Surprise: `StyleRule` had no `name`/`description` — the `.sty` parser
dropped `\Name` and `\Description`. `usfm_style` now keeps both as
`Option<String>` (empty is `None`, a repeated `\Marker` amends), `build.rs`
writes them into the default sheet, and the three hand-built rules pass
`None`. Tests: 2 in `usfm_style`, 4 `convert`, 4 `locate`, 8 `hover`, 4
`format`, and `tests/lsp.rs` gains a formatting request (the edit equals
the writer's output over the whole document; `null` and the warning on an
Error) and a hover request (`\p`'s name; the reference on `\v`). Gate
exit 0. `vscode/README.md` has the manual steps (hover, Format Document,
`editor.formatOnSave`, the refusal).
