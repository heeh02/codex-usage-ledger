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

## Conversation pagination

`period=custom` accepts `startDate` and `endDate` as ordered YYYY-MM-DD dates
in the selected timezone. The ending day is included, using an exclusive
midnight boundary on the following day. Missing/reversed dates return 400.
Custom windows do not assume a previous-period comparison. Future ending
boundaries keep the result marked partial rather than claiming future zeroes.

Unsupported period, grain, metric or session-sort values return HTTP 400 before
storage access. Invalid IANA timezones, page sizes outside 1–100 and searches
longer than 256 characters are also rejected. Invalid input must not silently
fall back to lifetime or surface as a server error.

The additive `year` period selects January 1 at local midnight through now,
with monthly default grain. Its comparison uses the same calendar date/time
in the preceding year; February 29 maps to February 28 when necessary. This
is distinct from the existing trailing twelve-calendar-month period.

`/v1/explorer` and `/v1/bundle` accept `sessionSearch`, `sessionSort` (tokens,
output, requests, recent), `sessionOffset` and `sessionLimit` (1–100, default 30).
The additive optional `explorer.sessionPage` object reports total matches,
offset, limit, hasMore, search and applied sort. Clients must follow pagination
to enumerate all roots; the old fixed project cap is no longer a completeness
boundary. Search is literal substring search, not a SQL wildcard expression.

Scope filtering and usage sorting happen before the page limit. Page/search
controls do not change summary/chart denominators. Account/model filters only
include roots with matching usage in the selected period. Existing clients can
still read `sessions`; updated clients use the page metadata to load the rest.
No stored event or catalog membership is modified by pagination.

## CSV export format

The dashboard CSV is a source-tagged table, not a join driven by official dates.
Each local bucket is exported even if official daily activity is unavailable.
Official buckets have their own source and grain, with absent token components
left blank. Project/model/session exports contain only their local scope; a
conversation export follows the selected own/descendant timeline.

Stable machine-readable columns include the applied account, project, session,
model, period boundaries and timezone. Cache-write observations and coverage
are separate columns; no coverage yields a blank observation, not zero.
Consumers of the original two-total-column CSV must use `source` and `total`
instead. Official and local rows must never be added to each other.
