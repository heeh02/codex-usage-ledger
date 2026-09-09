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

cargo run --quiet --manifest-path "$repo_root/Cargo.toml" --example export_request_evidence_schema -- --turns \
  > "$temporary_directory/turn-evidence.schema.json"
cmp "$temporary_directory/turn-evidence.schema.json" "$repo_root/web/src/api/turn-evidence.schema.json"
(
  cd "$repo_root/web"
  npx json2ts --input src/api/turn-evidence.schema.json \
    --output "$temporary_directory/turn-evidence.generated.ts" --no-additionalProperties
)
cmp "$temporary_directory/turn-evidence.generated.ts" "$repo_root/web/src/api/turn-evidence.generated.ts"
echo "Retained-turn API contract is current."

cargo run --quiet --manifest-path "$repo_root/Cargo.toml" --example export_request_evidence_schema -- --quota-history \
  > "$temporary_directory/quota-history.schema.json"
cmp "$temporary_directory/quota-history.schema.json" "$repo_root/web/src/api/quota-history.schema.json"
(
  cd "$repo_root/web"
  npx json2ts --input src/api/quota-history.schema.json \
    --output "$temporary_directory/quota-history.generated.ts" --no-additionalProperties
)
cmp "$temporary_directory/quota-history.generated.ts" "$repo_root/web/src/api/quota-history.generated.ts"
echo "Quota-history API contract is current."

cargo run --quiet --manifest-path "$repo_root/Cargo.toml" --example export_request_evidence_schema -- --quota-interval-usage \
  > "$temporary_directory/quota-interval-usage.schema.json"
cmp "$temporary_directory/quota-interval-usage.schema.json" "$repo_root/web/src/api/quota-interval-usage.schema.json"
(
  cd "$repo_root/web"
  npx json2ts --input src/api/quota-interval-usage.schema.json \
    --output "$temporary_directory/quota-interval-usage.generated.ts" --no-additionalProperties
)
cmp "$temporary_directory/quota-interval-usage.generated.ts" "$repo_root/web/src/api/quota-interval-usage.generated.ts"
echo "Quota-interval usage API contract is current."
