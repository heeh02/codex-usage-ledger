#!/usr/bin/env bash
set -euo pipefail

for command in cargo node npm; do
  command -v "$command" >/dev/null 2>&1 || { echo "$command is required" >&2; exit 1; }
done

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
output_dir="${1:-$repo_root/.ci-artifacts/sbom}"
mkdir -p "$output_dir"

cargo metadata --locked --format-version 1 > "$output_dir/cargo-metadata.json"
node "$repo_root/scripts/cargo-metadata-to-cyclonedx.mjs" \
  "$output_dir/cargo-metadata.json" \
  "$output_dir/rust.cdx.json"
npm --prefix "$repo_root/web" sbom --package-lock-only --sbom-format cyclonedx > "$output_dir/web.cdx.json"
node "$repo_root/scripts/check-dependency-licenses.mjs" \
  "$output_dir/cargo-metadata.json" \
  "$repo_root/web/package-lock.json"
node "$repo_root/scripts/generate-third-party-licenses.mjs" \
  "$output_dir/cargo-metadata.json" \
  "$repo_root/web/package-lock.json" \
  "$output_dir/THIRD_PARTY_LICENSES.txt"

echo "Dependency inventories written to $output_dir"
