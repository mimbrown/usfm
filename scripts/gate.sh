#!/usr/bin/env bash
# The gate every commit passes before it is pushed. CI runs the same steps.
# Grows with the milestones (fuzzing after 06).
set -euo pipefail
cd "$(dirname "$0")/.."

export INSTA_UPDATE=no

cargo build --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --exclude usfm_tests --no-fail-fast
cargo run --package usfm_tests -- --baseline tests/tcdocs-baseline.txt
# Ticket 05: Miri over the lexer, the parser's byte handling and
# `string_parser`. Needs nightly + miri; `scripts/session-start.sh` installs it.
scripts/miri.sh
