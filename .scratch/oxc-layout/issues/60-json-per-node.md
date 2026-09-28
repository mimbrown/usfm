# 60. `usfm_json`: per-node values, and a custom node between them

Status: ready-for-agent
Milestone: after M7 (publishing foundation)

The JSON writer is a pure recursion with no state. Make `block_value` and
`inline_value` (with the sheet they resolve styles against) public, so a
caller can build its own array with its own objects between ours.

Document that a custom object is the caller's own business, as long as it
does not reuse a `type` from `usfm_json::TYPES`. `to_json_value`'s output
must not change.
