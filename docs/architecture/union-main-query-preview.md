# Request union through the product query chain

The diagnostic union reader previously bypassed the dashboard's query services.
That proved candidate arithmetic, but did not prove summary, project, conversation
and chart integration. The connection-local main-query preview now exercises the
existing services without promoting a stored policy or rewriting source facts.

## Invocation and compatibility

```sh
codex-usage-ledger preview-union-bundle --db /work/review/ledger.sqlite3 \
  --query '{"period":"custom","startDate":"2026-04-01","endDate":"2026-04-02","timezone":"UTC"}'
```

The command requires an existing current-schema ledger and a fully processed,
resolved policy-v2 union projection. Missing, old, pending or unresolved inputs
fail; they are not silently migrated, refreshed, filtered out or replaced by the
old selector. Normal CLI/dashboard defaults are unchanged. There is no new HTTP
route or wire-schema change: the `bundle` member is the existing dashboard DTO.
The wrapper explicitly identifies `request_union_preview`,
`productionPolicyChanged: false` and `historyComplete: false`.

The main file is opened with SQLite READ_ONLY flags. Query-only mode is briefly
disabled to install connection-local TEMP views, then enabled before the store
is exposed. Main-file write protection remains throughout. TEMP views replace
the three effective event/day/hour read relations, not the persistent database
views. Closing the connection restores ordinary reads automatically.

## Query integration

- Selected request occurrences supply the event relation, including retained-only
  sampling after raw compaction. Daily/hourly relations use the same records.
- Existing account, model, classified-project, conversation membership and exact
  timezone queries remain responsible for product filtering and aggregation.
- The extra retained-request branch in exact-time queries is disabled in this
  lane: those requests are already present, and appending them would count twice.
- API dispatch retains the preview connection. Opening another ordinary reader
  for an endpoint would silently return to the legacy selector.
- A bundle freezes one SQLite snapshot and rechecks projection readiness inside
  it. Later writes that dirty the selection prevent a subsequent fresh read;
  the preview never recalculates or repairs the source data itself.
- Confirmed freshness and selected reconstruction amount use the same selection.
  Raw unknown/quarantined diagnostics still describe their original evidence.

## Acceptance and remaining limits

The synthetic A100+B200 / B200+C300 overlap now yields 600 through the existing
summary, day/hour rollup, exact-time series, account/project/model/thread queries
and actual async dashboard bundle, rather than the legacy 500. The sampling-only
model remains visible after raw compaction. Component conservation and UTC,
Shanghai and New York custom-window checks cover the shared product chain.
File bytes remain unchanged, writes fail, and reopening normally restores the
legacy result. Pending/unresolved selections fail instead of reporting zero.

This is the integration acceptance lane, not completed production promotion.
Historical replay correction and source-occurrence linkage still require review.
The preview neither manufactures keys for legacy records nor fills official
differences. Request/turn evidence endpoints retain their own evidence scope;
union totals do not imply every selected reconstruction has a sampling receipt.
Large-data performance, populated native journeys and installation remain gates.
Official account totals and quota observations are not changed by this adapter.
