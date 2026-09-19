# 02. Clippy clean, gated in CI

Status: ready-for-agent
Milestone: M1
Blocked by: 01

118 warnings on 2026-09-12. Start with `cargo clippy --fix`, then by hand. Delete
dead code rather than `#[allow]` it (the commented-out block at the end of
`usx.rs`, unused imports). An `#[allow]` needs a reason in a comment.

Also fix the `unstable_name_collisions` warning in `usfm_ast/src/reference.rs:128`.

Done when `cargo clippy --workspace --all-targets -- -D warnings` passes and is a
CI step.
