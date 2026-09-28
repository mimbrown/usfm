# 60. `usfm_json`: per-node values, and a custom node between them

Status: resolved (2026-09-28)
Milestone: after M7 (publishing foundation)

The JSON writer is a pure recursion with no state. Make `block_value` and
`inline_value` (with the sheet they resolve styles against) public, so a
caller can build its own array with its own objects between ours.

Document that a custom object is the caller's own business, as long as it
does not reuse a `type` from `usfm_json::TYPES`. `to_json_value`'s output
must not change.

## Answer

`usfm_json::JsonWriter` is public: `new(sheet)`, and `document`,
`block`, `inline`, `blocks`, `inlines` and `marker`. The type doc shows a
caller's own object between our values. A test in `tests/json.rs` checks
that each block's value, written on its own, is the document's child.
