#!/usr/bin/env bash
# Paste this file's contents into the cloud environment's "Setup script" field.
# It provisions the VM only: it runs once as root, before Claude Code starts,
# must finish in about five minutes and must exit zero or the session will not
# start, and its result is cached. Project setup is scripts/session-start.sh.
#
# The image ships rustc and cargo. rustup is wanted so rust-toolchain.toml is
# honoured and so nightly (Miri, cargo-fuzz) can be added when a ticket needs it.
if ! command -v rustup >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --profile minimal --default-toolchain stable -c clippy -c rustfmt \
    || echo "cloud-setup: rustup not installed; using the image's cargo" >&2
fi
exit 0
