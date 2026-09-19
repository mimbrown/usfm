#!/usr/bin/env bash
# Setup script for a Claude Code cloud environment. Idempotent.
# Needs network access to github.com, crates.io, static.crates.io and
# static.rust-lang.org.
set -euo pipefail
cd "$(dirname "$0")/.."

if ! command -v rustup >/dev/null; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
  . "$HOME/.cargo/env"
fi
rustup show active-toolchain            # installs the pinned toolchain
rustup toolchain install nightly --profile minimal --component miri,rust-src
command -v cargo-fuzz >/dev/null || cargo install cargo-fuzz --locked
command -v cargo-insta >/dev/null || cargo install cargo-insta --locked

git submodule update --init tcdocs
cargo fetch
