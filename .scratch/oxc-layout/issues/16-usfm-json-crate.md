# 16. `usfm_json`: the AST as JSON

Status: resolved
Milestone: M3
Blocked by: 15

The ADR lists `usfm_json` (AST -> JSON) among the outputs; nothing exists yet.
It is the shape the language server and any non-Rust consumer will read, so
it is written before the facade.

- New crate `usfm_json` depending on `usfm_ast` and `serde_json` (`serde` is
  already a workspace dependency). Emit the AST as it is, one object per node
  with `"type"`, `"span": [start, end]`, `"style"` as the marker name (resolve
  through `document.style_sheet()`), children, attributes, and the note /
  chapter / verse fields; no USX-flavoured renaming. Implement as a
  `usfm_ast::visit::Visit` building `serde_json::Value`, or a hand-written
  writer to a `String` if that benches better; either way no `Serialize`
  derives on the AST (keeps `usfm_ast` free of serde).
- `to_json_string(&Document) -> String` and a `--format json` arm in the CLI.
- Tests: snapshot (`insta`) of two small documents, and a test that every
  `Block`/`Inline`/`Note` variant appears in the output of a document that
  uses them all (build it from `usfm_ast::test_fixtures` if that has one, else
  from a short USFM string).
- Bench group `parse_json` in `tasks/benchmark`, recorded in
  `docs/benchmarks.md` as a new row, not compared to anything.

Done when the gate is green and the CLI emits JSON.

## Answer

Landed via PR #19 (2026-09-19). `usfm_json` (depends on `usfm_ast`,
`usfm_style`, `serde_json`; the AST stays serde-free): `to_json_value`,
`to_json_string`, `to_json_string_pretty`, `TYPES`. One object per node with
`type`, `span`, `style` (marker name via the document's stylesheet),
`children`, `attributes` (ordered, duplicates kept, default attribute named
`""`), and the node's own fields under their AST names; absent optionals are
omitted, USFM numbers stay strings, keys are sorted (`serde_json`'s `Map`).
A direct recursion rather than `Visit`: nothing is carried between nodes.
Tests: two insta snapshots, a coverage test in both directions against
`TYPES`, well-formedness, and 268 notes in `71-WIS.usfm`. CLI `--format
json`. `parse_json` whole-corpus 7.90 MiB/s (attributes-heavy 4.28: a
`BTreeMap` per node and one object per attribute pair; a streaming writer
would be the fix if it ever matters), recorded in `docs/benchmarks.md`.
