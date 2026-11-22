#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

rustup toolchain install
curl -fsSL "https://github.com/cargo-bins/cargo-binstall/releases/latest/download/cargo-binstall-$(uname -m)-unknown-linux-musl.tgz" |
  tar -xz -C "${CARGO_HOME:-$HOME/.cargo}/bin"
tools/setup.sh ~/.tools
