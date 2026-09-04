#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if [[ "${GITHUB_REF_TYPE:-}" != "tag" ]]; then
  echo "Release tag gate skipped outside a tag run."
  exit 0
fi

version="$(awk '
  /^\[package\]$/ { in_package = 1; next }
  /^\[/ { in_package = 0 }
  in_package && /^version = / {
    gsub(/^version = \"|\"$/, ""); print; exit
  }
' Cargo.toml)"
expected_tag="v${version}"
actual_tag="${GITHUB_REF_NAME:-}"

if [[ -z "$version" || "$actual_tag" != "$expected_tag" ]]; then
  echo "Release tag must equal Cargo package version: expected $expected_tag, found ${actual_tag:-<empty>}" >&2
  exit 1
fi

if [[ "$(git cat-file -t "refs/tags/$actual_tag" 2>/dev/null || true)" != "tag" ]]; then
  echo "Release tag $actual_tag must be an annotated tag." >&2
  exit 1
fi

tag_commit="$(git rev-list -n 1 "refs/tags/$actual_tag")"
if [[ -n "${GITHUB_SHA:-}" && "$tag_commit" != "$GITHUB_SHA" ]]; then
  echo "Release tag resolves to $tag_commit, not workflow commit $GITHUB_SHA." >&2
  exit 1
fi

echo "Release tag gate passed: $actual_tag -> $tag_commit"
