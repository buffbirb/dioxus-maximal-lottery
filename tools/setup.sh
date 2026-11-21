#!/usr/bin/env bash
# Installs the tools the repository pins into one directory (.tools unless given
# another), with their commands in its bin/, and the git hook.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
dir="${1:-$root/.tools}"
mkdir -p "$dir/bin"
dir="$(cd "$dir" && pwd)"
cd "$root"

if [ Cargo.lock -nt "$dir/.crates2.json" ]; then
  tools/install-cargo-tools.sh "$dir"
  touch "$dir/.crates2.json"
fi

for tool in prek supabase; do
  if ! cmp -s "tools/$tool/package-lock.json" "$dir/$tool/package-lock.json" ||
    [ ! -e "$dir/$tool/node_modules/.package-lock.json" ]; then
    mkdir -p "$dir/$tool"
    cp "tools/$tool/package-lock.json" "tools/$tool/package.json" "$dir/$tool/"
    npm ci --ignore-scripts --no-audit --no-fund --prefix "$dir/$tool" --silent
  fi
  ln -fns "../$tool/node_modules/.bin/$tool" "$dir/bin/$tool"
done

# Every worktree shares the hook, which records the path of the prek that
# installed it, so it's reinstalled only when that prek is gone.
hook="$(git rev-parse --git-path hooks/pre-commit)"
if [ ! -x "$(sed -n 's/^PREK="\(.*\)"$/\1/p' "$hook" 2>/dev/null)" ]; then
  "$dir/bin/prek" install --quiet
fi
