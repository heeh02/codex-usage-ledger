#!/usr/bin/env bash
set -euo pipefail

for command in cargo cmp mktemp npx; do
  command -v "$command" >/dev/null 2>&1 || { echo "$command is required" >&2; exit 1; }
done

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
temporary_directory="$(mktemp -d)"
trap 'rm -rf -- "$temporary_directory"' EXIT INT TERM

cargo run --quiet --manifest-path "$repo_root/Cargo.toml" --example export_api_schema \
  > "$temporary_directory/dashboard-bundle.schema.json"
cmp "$temporary_directory/dashboard-bundle.schema.json" \
  "$repo_root/web/src/api/dashboard-bundle.schema.json"

(
  cd "$repo_root/web"
  npx json2ts \
    --input src/api/dashboard-bundle.schema.json \
    --output "$temporary_directory/wire.generated.ts" \
    --no-additionalProperties
)
cmp "$temporary_directory/wire.generated.ts" "$repo_root/web/src/api/wire.generated.ts"

echo "Rust schema and generated TypeScript API contract are current."

cargo run --quiet --manifest-path "$repo_root/Cargo.toml" --example export_request_evidence_schema \
  > "$temporary_directory/request-evidence.schema.json"
cmp "$temporary_directory/request-evidence.schema.json" "$repo_root/web/src/api/request-evidence.schema.json"
(
  cd "$repo_root/web"
  npx json2ts --input src/api/request-evidence.schema.json \
    --output "$temporary_directory/request-evidence.generated.ts" --no-additionalProperties
)
cmp "$temporary_directory/request-evidence.generated.ts" "$repo_root/web/src/api/request-evidence.generated.ts"
echo "Retained-request API contract is current."
