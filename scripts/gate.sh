#!/usr/bin/env bash
# The gate every commit passes before it is pushed. CI runs the same steps.
# Grows with the milestones (clippy after ticket 02, Miri after 05).
set -euo pipefail
cd "$(dirname "$0")/.."

export INSTA_UPDATE=no

# usfm_language_server does not compile yet; ticket 01 removes this exclude.
EXCLUDE=()
if grep -q '"usfm_language_server"' Cargo.toml; then
  EXCLUDE=(--exclude usfm_language_server)
fi

cargo build --workspace "${EXCLUDE[@]}"
cargo test --workspace "${EXCLUDE[@]}" --exclude usfm_tests --no-fail-fast
cargo run --package usfm_tests -- --baseline tests/tcdocs-baseline.txt
