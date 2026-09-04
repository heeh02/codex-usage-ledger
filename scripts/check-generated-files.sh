#!/usr/bin/env bash
set -euo pipefail

command -v git >/dev/null 2>&1 || { echo "git is required" >&2; exit 1; }

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if command -v rg >/dev/null 2>&1; then
  tracked="$(git ls-files | rg '^(target/|dist/|web/dist/|web/node_modules/|macos/Assets/AppIcon\.iconset/)' || true)"
else
  tracked="$(git ls-files | grep -E '^(target/|dist/|web/dist/|web/node_modules/|macos/Assets/AppIcon\.iconset/)' || true)"
fi
if [[ -n "$tracked" ]]; then
  echo "Generated build output is tracked:" >&2
  echo "$tracked" >&2
  exit 1
fi

echo "Generated-file policy passed."
