# 16. `usfm_json`: the AST as JSON

Status: ready-for-agent
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
