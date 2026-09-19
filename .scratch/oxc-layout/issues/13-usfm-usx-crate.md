# 13. `usfm_usx`: USX output as its own crate, no shared `Context`

Status: resolved
Milestone: M3
Blocked by: 12

`usfm_parser/src/usx.rs` (470 lines) and `xml_document.rs` (209 lines, the
`XmlNode` tree and writer, plus the reader `tests/src/lib.rs` uses to compare
against reference USX) move to `usfm_usx`. The ADR says "no output crate
depends on another output crate" and the spec's open question is whether
`Context` (note numbering, counters) survives as HTML-only state.

- New crate `usfm_usx` depending on `usfm_ast`, `usfm_style`, `xml-rs`. Public
  API: `to_usx_node(&Document) -> XmlNode`, `to_usx_string(&Document) ->
  String`, `XmlNode` and its reader/writer. `usfm_parser::usx` and
  `usfm_parser::xml_document` become re-exports for one milestone, then go.
- `Context` decision: USX needs the book code, the current chapter and verse
  (for `vid`/`sid`/`eid`) and the `include_vid` flag; it does not need note
  numbering. Give `usfm_usx` its own private state struct built from
  `document.style_sheet()`, and remove its dependency on
  `usfm_parser::context::Context`. Record the decision in the spec's open
  list ("M3: Context survives as HTML-only state: yes/no").
- Implement the walk as `usfm_ast::visit::Visit` (or `fold::Fold`) rather
  than the `SerializeUsx` trait with a `Context` parameter, per the spec. If
  a node needs its parent (verse ends inside notes, table `vid`), the visitor
  keeps a small stack; do not add a parent pointer to the AST.
- `usfm_parser/tests/usx_text.rs` and the tcdocs harness are the tests; they
  must pass unchanged. tcdocs stays 215 / 0 / 44.
- Bench: `parse_usx` medians within 3% of `docs/benchmarks.md` "After ticket
  05" numbers, measured interleaved as that file says; record the run.

Done when the gate is green, tcdocs is unchanged, and the bench is within 3%.

## Answer

Landed via PR #16 (2026-09-19). `usfm_usx` (depends on `usfm_ast`,
`usfm_style`, `xml-rs`; never on the parser) owns `usx.rs` and
`xml_document.rs`. The walk is a `usfm_ast::visit::Visit` over private state
(book code, chapter, open verse, `include_vid`, the document's stylesheet); no
parent stack was needed. API: `to_usx_node`, `to_usx_string`,
`to_usx_node_with_options(UsxOptions { include_vid })`, `XmlNode` and the
reader the harness uses. `usfm_parser::usx` / `::xml_document` re-export it
until ticket 17; the harness now calls `usfm_usx` directly (two lines).

`parse_usx` interleaved against `fe63ec3`, median of three: whole-corpus
19.27 → 19.24 MiB/s (−0.1%), plain −2.7%, attributes-heavy −2.6%,
alignment-heavy +0.5%, note-heavy −0.4%; all within 3%. Two earlier visitor
versions were 5–7% slower (an attribute-name double allocation, and codegen
placement across the new crate boundary), recorded in `docs/benchmarks.md`
"After ticket 13". Spec's `Context` question: USX needs none of it; HTML is
ticket 14's call.
