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

Period descriptors in one bundle share a server-internal reference instant,
including rolling-window start/end computation. Clients cannot supply this
clock through query parameters. Bundle parts also read one consistent database
snapshot; concurrent WAL commits become visible on a later response. Separate
requests/pages are not pinned to this snapshot. See
[exact-window indexing and clock](../architecture/exact-window-query-clock.md).

`GET /healthz` returns `service`, `status`, and integer `processId`. Native shells
must verify the responding process is their current live child before loading
the dashboard; service name alone does not distinguish instances on a reused
port. This is an accidental-instance check, not an authentication token. See
[the native process-binding decision](../adr/0005-native-health-process-binding.md).

Summary `matchRate`, `cacheRate`, and `averagePerDay` are required nullable
numbers. Empty denominators or absent confirmed samples return null, not a
fabricated rate or average. `metrics.localAttributedTotal` is unknown/null
without confirmed events; recorded zero events retain a numeric zero sample.
Additive usage/count structures remain available for conservation, not as proof
of observed zero activity. Deploy this contract with its matching dashboard;
see [empty evidence semantics](../adr/0002-empty-evidence-metrics.md).

Collection phase `degraded` indicates that at least one local sampling, quota
or reconstruction step failed and will retry. Existing ledger facts remain
available; missing updates are not zero usage. `message` contains stable source
codes, not raw source errors. The UI supplies localized failure copy. This
closed-enum addition requires paired backend/Web deployment as recorded in
[the collector lifecycle decision](../adr/0001-collector-degraded-state.md).

`timeseries.dailyPoints` retains daily local aggregates even when the main
plot uses week/month grain. Missing days are absent, not implicitly zero.

Session detail accepts `nodeOffset`, `nodeLimit` (1–1000, default 200), and
`nodeSearch` over safe display labels/IDs. Its optional `nodePage` metadata
describes matching nodes; own/tree totals and event counts always cover the
full selected scope, independent of the visible page. Pagination replaces the
old non-enumerable 800-node truncation.

The additive `timeseries.modelSeries` and `timeseries.accountSeries` collections
contain local source aggregates on the same requested window/grain as the
project series. They are not derived from official totals or proportional
allocation. Each dimension conserves the corresponding local time series.

`period=custom` accepts `startDate` and `endDate` as ordered YYYY-MM-DD dates
in the selected timezone. The ending day is included, using an exclusive
midnight boundary on the following day. Missing/reversed dates return 400.
If a clock transition skips midnight, the boundary is the first existing
instant of that civil date. A completely skipped boundary date returns 400;
it must never resolve to the current time or silently yield a different range.
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

For `period=rolling7`, conversation sorting and displayed own/tree totals use
the exact half-open timestamp range, not all events from the boundary dates.
The same per-thread/time projection feeds detail nodes and own/descendant
timelines, independent of list/node pagination. This corrects query results
without changing DTOs or persisted accounting/source selection. Non-Shanghai
rolling timelines use exact timestamp evidence in the requested timezone;
stored Shanghai-hour labels must not be mixed with timezone-local boundaries.

For supported non-Shanghai `timezone` values, calendar-window local totals and
curves likewise use exact timestamp bounds and timezone-local date/hour labels.
Local previous-period values and comparison curves follow their descriptor's
bounds rather than rounding them to Shanghai storage dates. This changes
affected query values, not DTO fields, stored facts or official provider dates.

If local historical hourly aggregates survive but per-request timestamps needed
to split those hours do not, a non-hour-aligned timezone request returns HTTP
422 with an insufficient-time-precision error. Clients must retain the last
accepted scope/response and show the error, not replace it with a zero or a
smaller partial curve. Choose an hour-aligned timezone or a range with retained
timestamp evidence. Ordinary schema validation errors remain HTTP 400.

Error JSON adds a stable `code` alongside the existing diagnostic `error`:
`insufficient_time_precision` for HTTP 422, `invalid_query` for invalid query or
account-count HTTP 400, and `request_failed` for other failures. Older clients
can still read `error`. New clients must handle unknown/missing codes generically
and must not infer a precision failure from arbitrary error prose or HTTP status
alone. The Web client accepts the precision code only with 422, renders localized
copy, and does not expose raw response bodies as UI text.

The dashboard retains a typed failure rather than a translated string so language
switches update an already visible error. Accepted data and filters stay together
on failure. With no accepted data, a precision error offers a user-triggered today
range with account/project/model/session scope preserved; it does not silently
reset scope or invent an empty successful response. Existing snapshots remain
visible with an accessible error notice.

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

## Retained request evidence

Request and turn responses expose `backfillComplete`: completion of the bounded
upgrade raw-evidence target, not complete lifetime history. Clients may show a
pending notice only when false. `historyComplete` remains independently false.

The response type is defined in Rust wire DTOs and generated independently as
`request-evidence.schema.json` and `request-evidence.generated.ts`. The API
contract gate checks these artifacts alongside the dashboard bundle contract.

`GET /v1/request-evidence` accepts required `threadId`, RFC3339 `start`
and `end` (half-open), optional `limit` (1–500, default 100), and paired
`afterTime`/`afterId` from the preceding response's `next` object.
Optional `account` and `model` filter before pagination; omitted or `all`
means unfiltered. Account membership uses current retained assignments, not
the observed-account value returned on each row. Selections are trimmed;
empty selections are unfiltered consistently in both execution and metadata.
Unknown parameters are rejected.
`selectionAttribution=current_ledger`, `selectedAccount` and `selectedModel`
make this distinction explicit; nullable selection fields mean unfiltered.

Responses identify `scope=thread_own_retained_observations`,
`attribution=ingest_observed`, and `historyComplete=false`. Rows contain
event ID, effective time, nullable turn/model/observed account/project, both
attribution confidences, quality and standard token dimensions. `next=null`
ends the retained page sequence, not evidence of complete lifetime collection.
Invalid ranges, limits or cursors return 400. The existing loopback boundary
applies. These observations must not be added to effective aggregate totals.

## Retained turn evidence

Rust wire DTOs generate `turn-evidence.schema.json` and
`turn-evidence.generated.ts`; the shared contract gate validates both.

`GET /v1/turn-evidence` uses required threadId/start/end, optional account/model,
limit (1–500, default 100) and offset (default 0). It returns typed turn rows
grouped before paging, with nextOffset and historyComplete=false. Stable groupId
distinguishes explicit turns from separate requests lacking turn IDs. Each row
has firstAt/lastAt, requestCount, confirmedRequestCount and confirmedUsage;
confirmedUsage is null when no confirmed requests exist. It is never a complete
turn/lifetime claim or a second amount to add to account totals.

## Scoped JSON export details

JSON exports use `format: codex-usage-ledger.scoped-usage`, `version: 1`,
`generatedAt` and `rows`. Row field names and scope match the CSV contract.
Unavailable CSV fields become JSON null, not numeric zero. Session own/tree
selection applies equally to both formats; a mismatched selected-session
response yields no rows rather than exporting another conversation.

This replaces the previous raw bundle dump. Consumers must not expect embedded
catalogs, account registries, diagnostic payloads or unrelated conversation
titles. Exports still contain the explicitly selected scope identifiers and
are not anonymous sharing artifacts. Official and local rows remain independent.
