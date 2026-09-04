#!/usr/bin/env bash
set -euo pipefail

for command in node sed; do
  command -v "$command" >/dev/null 2>&1 || { echo "$command is required" >&2; exit 1; }
done

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

cargo_version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)"
web_version="$(node -p "require('./web/package.json').version")"
macos_version="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' macos/Info.plist 2>/dev/null || sed -n '/CFBundleShortVersionString/{n;s/.*<string>\([^<]*\)<.*/\1/p;}' macos/Info.plist)"

if [[ -z "$cargo_version" || "$cargo_version" != "$web_version" || "$cargo_version" != "$macos_version" ]]; then
  echo "Version mismatch: Cargo=$cargo_version Web=$web_version macOS=$macos_version" >&2
  exit 1
fi

echo "Version consistency passed: $cargo_version"
