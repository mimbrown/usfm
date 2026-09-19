# 02. Clippy clean, gated in CI

Status: resolved
Milestone: M1
Blocked by: 01

118 warnings on 2026-09-12. Start with `cargo clippy --fix`, then by hand. Delete
dead code rather than `#[allow]` it (the commented-out block at the end of
`usx.rs`, unused imports). An `#[allow]` needs a reason in a comment.

Also fix the `unstable_name_collisions` warning in `usfm_ast/src/reference.rs:128`.

Done when `cargo clippy --workspace --all-targets -- -D warnings` passes and is a
CI step.

## Answer

Landed in `6c68d6e` (PR #5, 2026-09-19). `cargo clippy --workspace --all-targets
-- -D warnings` is clean and runs in `scripts/gate.sh` between the build and the
tests. Dead lexer primitives and the commented-out `Serialize` impl in `usx.rs`
were deleted; `string_parser::NoMatch` replaces `Result<_, ()>`;
`Caller::from_str` is `From<&str>`; one `#[allow(clippy::too_many_arguments)]`
on `ParseAndTransform::new` with its reason (the M3 clap rewrite replaces it).

CI ran a newer stable (1.98.0) than the cloud image (1.94.1) and reported two
lints the local gate did not, so `rust-toolchain.toml` now pins `1.98.0`; bump
it deliberately.

Follow-ups, not done here:
- The repository is not rustfmt-clean (58 hunks in 15 files). `cargo fmt --all
  -- --check` is not gated. Worth one formatting-only PR before M3 moves files.
