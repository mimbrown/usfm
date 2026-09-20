# 29. A block-level milestone with a space before `|` is not read as a milestone

Status: ready-for-agent
Milestone: M5

Found by ticket 25. `\zaln-s |x-strong="H1"\*` on its own line, with no
paragraph open, parses as a dropped unknown marker plus an implicit `\p`
holding the text `|x-strong="H1"`. Inside a paragraph the same text is a
milestone. Real aligned files write the space (unfoldingWord's do), so this
is a live mis-parse; it is also why `usfm_codegen` writes `|` flush against
the marker.

Cause: `parse_block_start` and `parse_milestone_node` call `eat(Kind::Pipe)`
with no `eat_whitespace()` first, unlike the inline path in `parse_marker`.

- Eat whitespace before the pipe on the block path, matching the inline path.
- Tests in `recovery.rs` (a block milestone with and without the space give
  the same tree) and `spans.rs` if the milestone span changes.
- Then `usfm_codegen` may write `\qt-s |who="…"\*` with the space, as USFM 3
  spells it; update its writer and docs, and the round trip must still hold.

Done when the gate is green.
