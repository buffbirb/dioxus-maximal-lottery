#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

rustup toolchain install
# The Node.js feature installs only nvm, so the version comes from .node-version.
# nvm's script trips over nounset.
set +u
# shellcheck source=/dev/null
. "$NVM_DIR/nvm.sh"
nvm install "$(cat .node-version)"
nvm alias default "$(cat .node-version)"
set -u
curl -fsSL "https://github.com/cargo-bins/cargo-binstall/releases/latest/download/cargo-binstall-$(uname -m)-unknown-linux-musl.tgz" |
  tar -xz -C "${CARGO_HOME:-$HOME/.cargo}/bin"
tools/setup.sh ~/.tools
