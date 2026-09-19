#!/usr/bin/env bash
# SessionStart hook (.claude/settings.json): project setup, local and cloud.
# Never fails the session; the gate reports anything that is actually missing.
cd "$(dirname "$0")/.." || exit 0
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"

# The conformance suite. Without it the usfm_tests build fails on purpose.
if [ ! -e tcdocs/tests ]; then
  git submodule update --init tcdocs >&2 || echo "session-start: tcdocs submodule not initialised" >&2
fi

# `scripts/gate.sh` ends in `scripts/miri.sh`, which needs a nightly toolchain
# with miri and rust-src. Everything else stays on the pinned 1.98.0 from
# rust-toolchain.toml. `rustup toolchain install` is idempotent, so this is a
# no-op once nightly is there; the `component list` check keeps it from hitting
# the network on every session.
if command -v rustup >/dev/null 2>&1; then
  installed=$(rustup component list --toolchain nightly --installed 2>/dev/null || true)
  if ! { echo "$installed" | grep -q '^miri' && echo "$installed" | grep -q '^rust-src'; }; then
    rustup toolchain install nightly --profile minimal --component miri,rust-src >&2 ||
      echo "session-start: nightly miri not installed; scripts/miri.sh will fail" >&2
  fi
fi

# Cloud VMs start cold: fetch crates once so the first gate run is not a download.
if [ "${CLAUDE_CODE_REMOTE:-}" = "true" ]; then
  cargo fetch --quiet >&2 || true
fi
exit 0
