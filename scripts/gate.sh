#!/usr/bin/env bash
# The gate every commit passes before it is pushed. CI runs the same steps.
#
# Fuzzing is deliberately not here: `tasks/fuzz` needs nightly and a sanitizer,
# and a run that finds anything takes minutes. Run it on demand instead —
# `cargo +nightly fuzz run --fuzz-dir tasks/fuzz parse_lossy -- -max_total_time=600
# -max_len=65536` — before a release and after a change to the lexer, the
# parser or the USX writer. See `tasks/fuzz/README.md`.
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
