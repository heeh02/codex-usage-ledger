# HTTP API contract

The loopback dashboard's `/v1/bundle` response has one source of truth:
`src/api/wire.rs`. Its Rust DTOs describe required fields, required nullable
fields, optional compatibility fields, nested objects, and closed enums.

The contract pipeline is:

```text
Rust wire DTOs
  └─ schemars ─► web/src/api/dashboard-bundle.schema.json
                   └─ json-schema-to-typescript ─► web/src/api/wire.generated.ts
                                                        └─ web/src/api/types.ts
```

Do not edit either generated file by hand. Run:

```bash
cargo run --quiet --example export_api_schema \
  > web/src/api/dashboard-bundle.schema.json
cd web
npx json2ts --input src/api/dashboard-bundle.schema.json \
  --output src/api/wire.generated.ts --no-additionalProperties
```

`scripts/check-api-contract.sh` regenerates both files in a temporary directory
and requires byte-for-byte equality. Rust integration tests also require the
checked-in schema to equal the DTO-generated schema, and the synthetic backend
bundle test deserializes the actual JSON into `DashboardBundle` before checking
accounting conservation.

Request filters and imperative refresh methods remain hand-written client types
because they are inputs rather than bundle response data. A new response field
must be added to the Rust DTO first; an incompatible removal or enum change
requires an ADR and release boundary.
