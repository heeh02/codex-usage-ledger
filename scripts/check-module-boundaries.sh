#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if command -v rg >/dev/null 2>&1; then
  violations="$(rg -n '\.connection\s*\(|rusqlite' src/api.rs src/api --glob '*.rs' || true)"
else
  violations="$(grep -nE '\.connection[[:space:]]*\(|rusqlite' src/api.rs || true)"
  nested="$(grep -RInE '\.connection[[:space:]]*\(|rusqlite' src/api --include='*.rs' || true)"
  if [[ -n "$nested" ]]; then
    violations="${violations}${violations:+$'\n'}${nested}"
  fi
fi

if [[ -n "$violations" ]]; then
  echo "API modules bypass the intent-specific storage boundary:" >&2
  echo "$violations" >&2
  exit 1
fi

echo "Module-boundary policy passed."
