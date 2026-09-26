#!/usr/bin/env bash
# The gate every commit passes before it is pushed. CI runs the same steps.
#
# Fuzzing is deliberately not here: `tasks/fuzz` needs nightly and a sanitizer,
# and a run that finds anything takes minutes. Run it on demand instead —
# `cargo +nightly fuzz run --fuzz-dir tasks/fuzz parse_lossy -- -max_total_time=600
# -max_len=65536` — before a release and after a change to the lexer, the
# parser or a writer. The `roundtrip` target (ticket 27) is the one to run
# after a parser change: the round-trip step below is the same property over a
# fixed corpus, and the fuzzer is what finds the inputs it does not hold on.
# See `tasks/fuzz/README.md`.
set -euo pipefail
cd "$(dirname "$0")/.."

export INSTA_UPDATE=no

cargo build --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --exclude usfm_tests --no-fail-fast
cargo run --package usfm_tests -- --baseline tasks/conformance/tcdocs-baseline.txt
# Ticket 27: the round trip as an invariant — parse -> USFM -> parse over every
# conformance case of every root, `pass` and `fail` alike, gated against the
# known-failure list the way the baseline is. About a second on the corpus.
cargo run --package usfm_tests -- --roundtrip tasks/conformance/roundtrip-known.txt
# Every crate the shipped binaries link must carry a licence file, or the
# extension's ThirdPartyNotices.txt (`npm run notices`) would name a licence
# without its text. The npm half needs node_modules, so it is checked when the
# notices are written rather than here.
python3 scripts/third_party_notices.py --check --no-npm
# Ticket 05: Miri over the lexer, the parser's byte handling and
# `string_parser`. Needs nightly + miri; `scripts/session-start.sh` installs it.
scripts/miri.sh
