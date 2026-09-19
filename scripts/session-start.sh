#!/usr/bin/env bash
# SessionStart hook (.claude/settings.json): project setup, local and cloud.
# Never fails the session; the gate reports anything that is actually missing.
cd "$(dirname "$0")/.." || exit 0
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"

# The conformance suite. Without it the usfm_tests build fails on purpose.
if [ ! -e tcdocs/tests ]; then
  git submodule update --init tcdocs >&2 || echo "session-start: tcdocs submodule not initialised" >&2
fi

# Cloud VMs start cold: fetch crates once so the first gate run is not a download.
if [ "${CLAUDE_CODE_REMOTE:-}" = "true" ]; then
  cargo fetch --quiet >&2 || true
fi
exit 0
