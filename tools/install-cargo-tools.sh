#!/usr/bin/env bash
# dx and sqlx-cli are released in lockstep with dioxus and sqlx, so Cargo.lock pins them.
set -euo pipefail
cd "$(dirname "$0")/.."

locked() {
  local id
  id="$(cargo pkgid --locked --package "$1")"
  echo "${id##*[#@]}"
}

args=(--locked)
if [ $# -gt 0 ]; then
  args+=(--root "$1")
fi

cargo binstall "${args[@]}" --disable-telemetry --no-confirm "dioxus-cli@$(locked dioxus)"
# sqlx-cli has no prebuilt binary, and its default features need OpenSSL's
# headers on Linux.
cargo install "${args[@]}" --features postgres,rustls --no-default-features "sqlx-cli@$(locked sqlx)"
