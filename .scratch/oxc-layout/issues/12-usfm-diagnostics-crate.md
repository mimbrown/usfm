# 12. `usfm_diagnostics`: `Diagnostic`, `Code`, `Severity`, `ParseResult` policy

Status: ready-for-agent
Milestone: M3
Blocked by: 11

`usfm_parser/src/diagnostics.rs` (580 lines) moves to its own crate so the
semantic pass (M4), the outputs and the CLI can report and render diagnostics
without depending on the parser.

- New crate `usfm_diagnostics` depending on `usfm_span` and `usfm_ast`
  (`ParseResult` holds a `Document`). `usfm_parser` re-exports
  `Code`, `Diagnostic`, `ParseResult`, `Severity` from it as today, so
  `usfm_parser::diagnostics::*` paths in tests keep working.
- The recovery table (each `Code`'s doc comment) moves with the enum; the rule
  "every `Code` has a test in `usfm_parser/tests/recovery.rs`" is unchanged.
  Add a unit test in the new crate that every `Code` variant has a stable
  kebab-case name (`Code::name()` or `Display`) and that names are unique;
  the CLI and the language server key on those names.
- Rendering: one `Diagnostic::render(&self, source: &str, path: &str) ->
  String` giving the `file:line:col: severity[code]: message` line the CLI
  prints today (`main.rs` `report_diagnostics`), using `usfm_span::LineIndex`.
  `main.rs` calls it. Also a `serde`-free JSON rendering
  (`Diagnostic::to_json_line`) for the M3 CLI's `--diagnostics json`; a
  hand-written writer is fine, no new dependency.
- `ParseResult::strict()` and any other policy helpers stay on `ParseResult`.

Done when the gate is green and nothing in `usfm_parser` refers to a
diagnostic type except through the re-export.
