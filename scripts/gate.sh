#!/usr/bin/env bash
# The gate every commit passes before it is pushed. CI runs the same steps.
# Grows with the milestones (clippy after ticket 02, Miri after 05).
set -euo pipefail
cd "$(dirname "$0")/.."

export INSTA_UPDATE=no

cargo build --workspace
cargo test --workspace --exclude usfm_tests --no-fail-fast
cargo run --package usfm_tests -- --baseline tests/tcdocs-baseline.txt
