# Candidate union query contract

Status: version-2 candidate reader, not an active dashboard-policy switch. It
reads the [durable union projection](source-union-projection.md) or resolves a
bounded scope from retained database evidence. It never reads native log files,
imports data or falls back to the old day-max aggregates.

`read-union-projection` requires an existing current-schema ledger, exact start
and exclusive end timestamps, a timezone and hour/day/week/month/year grain.
Optional account/project/model/thread identifiers are parameterized filters;
omission means all values, including unassigned. Literal `unknown` is not NULL.

```sh
codex-usage-ledger read-union-projection --db ./synthetic-ledger.sqlite3 \
  --start 2026-01-01T00:00:00Z --end 2026-02-01T00:00:00Z \
  --timezone Asia/Shanghai --grain week --account synthetic-account
```

Project identifiers are the staged assignment facts; NULL stays a separate
unassigned bucket. This diagnostic does not reclassify projects from the current
native catalog or allocate account-level differences to a folder.

One read transaction checks policy version and relevant group state, then
generates the summary, calendar buckets and account/project/model/thread
distributions. Readiness includes both matching raw evidence and matching cached
selections: moved/deleted evidence cannot leave a stale selected row visible.
Unrelated pending or unresolved groups do not block this scope.

`resolution=materialized_scope` streams a ready scoped projection. If that scope
is still dirty, `resolution=scoped_read` resolves up to 10,000 database records,
including counterparts outside the selected account/model/time filters. Group
validation precedes final filtering, so filters cannot hide relevant conflicts.
Larger unmaterialized scopes return `pending` with `resolution=scope_limit`, not
a truncated total; unknown policy versions also remain pending. Relevant
conflicts return `unresolved` and withhold amounts. Neither read mode writes or
rebuilds projection state.

Version 2 adds `resolution`, `scopeProjection` and `unconfirmedObservations`, and
uses scope `confirmed_local_measurements_not_inference_usage`. `projection`
retains global cache statistics; `scopeProjection` describes scoped cache state.
Their pending counts are not the availability of a successfully resolved read.

Unkeyed, nonconfirmed sampling observations are reported separately rather than
being counted as measurements or blocking confirmed usage elsewhere. Keyed
nonconfirmed counterparts still participate in identity/conflict validation.
The observation count is not a count of missing independent requests or extra
Token usage: another confirmed source may already cover an observation. Quality,
amounts and source keys remain unchanged.

Resolved, empty scopes return `no_records`, or `unconfirmed_only` when only
nonconfirmed observations are present, with nullable usage rather than measured
zero. A selected confirmed zero-amount observation returns `available`, counted
once. `historyComplete=false` and `productionPolicyChanged=false` always remain
explicit: resolved retained identities are not proof of complete inference.

Canonical sampling timestamps already selected by the union govern [start,end).
Timezone conversion precedes calendar bucketing; weeks start Monday, months and
years use their calendar boundaries. Repeated local hours retain UTC offsets.
All seven raw fields and event counts conserve independently in every dimension.
No official total, residual allocation or extra reasoning sum enters this query.

Schema 40 adds a time-range index on the selected projection for all-account
queries. Ready scopes stream matching rows; readiness also inspects relevant
database evidence and group indexes. A 10,000-bucket-per-dimension cap bounds result
memory and fails explicitly instead of truncating. Query cost still grows with
the selected interval; this is not a constant-time or benchmarked UI guarantee.

This reader establishes query parity before promotion. Its DTO changed from
version 1; no HTTP dashboard DTO or persisted ledger schema changes in this step.
Summary, chart, ranking,
chat and quota production consumers still require a reviewed common-policy
switch, real-account comparison and controlled migration receipts.

## Scoped HTTP access

`GET /v1/source-union` exposes this same version-2 candidate contract over the
existing loopback service. Required query parameters are `start`, `end`, `timezone`
and `grain`; optional `account`, `project`, `model` and `thread` are literal stored
identifiers. This is not `UsageQuery`: natural-period shortcuts and UI catalog
project reclassification are not accepted here. Unknown query fields, malformed
timestamps, invalid grains and invalid timezone/order fail rather than guessing.

The handler uses a read-only store connection and the reader's single snapshot,
with no global bundle precondition, source collection, account refresh or fallback
to day-max data. An unrelated unfinished group does not suppress an available
scope; conflicts in the requested scope still withhold `data`. Empty scopes remain
`no_records`, not measured zero. Existing identifier, counterpart, memory and
bucket limits remain active.

The main bundle/HTTP DTO is unchanged. This endpoint is the data path for future
section-wise UI integration, not a completed frontend switch. A preview server may
serve this endpoint while the globally gated `/v1/bundle` remains unavailable.
Strict CLI bundle previews retain their eager global readiness check.

## Scope-view presentation extension

The HTTP response adds nullable `display`, generated through the existing server
Token presenter. Available results include normalized `usage` and `byTime`,
`byModel`, `byAccount`, `byProject`, `byThread` rows (`id`, `events`, `usage`).
Unavailable results have `display=null`; the raw version-2 evidence DTO remains.
The frontend checks echoed scope, numeric validity, unique groups and component/
record conservation before rendering. No client-side source arbitration or
official-total composition is introduced.

`GET /v1/source-catalog` returns version-1 metadata only: project IDs/names, at
most 500 recent root conversation IDs/safe labels/project IDs, and previously
recorded verified/official account IDs. It does not refresh native indexes, read
credentials or imply coverage from catalog presence. Root labels reuse the
existing sensitive/long-title policy; filesystem roots and prompt bodies are not
returned as separate fields. The selector explicitly states its bounded root list.

The catalog accepts optional `search` (maximum 256 characters), echoed after
trimming. Literal title/ID matching happens across the root catalog before the
500-row limit; `%` and `_` are not wildcards. Existing callers without search keep
their previous behavior. The UI validates the echoed search, preserves last-good
lists on failure, and separates catalog requests from usage requests. A new global
root search clears draft project/thread selection without relabeling an already
displayed usage result. Search enables lookup beyond the initial limit, but is not
full paginated browsing or semantic search of all subagent names.

Global `SnapshotUnavailable` now returns HTTP 503 with code
`snapshot_unavailable` instead of generic HTTP 500. On initial-load failure only,
the existing application offers an independent scope form rather than a dead end.
Other failures retain their error path; a last-good complete bundle remains intact.
The scope form uses actual submitted dates (device timezone, exclusive end),
literal project/account and optional root-thread filters. Thread mode is explicitly
own-only by default, with explicit descendant selection as described below.
New results and their applied captions update atomically; failed/aborted
queries cannot relabel older data. No unavailable amount is rendered as zero.

This supplementary view uses existing model/account/component tables, a ranked
date table and the shared [scoped trend chart](scoped-trend-chart.md). It is not
yet unified sidebar navigation, descendant browsing, full root search/pagination
or natural-period shortcuts.
The source endpoints and special availability status are additive contracts; the
main bundle schema and persisted ledger schema do not change.

## Descendant scope

`includeDescendants=true` (CLI `--include-descendants`, requiring `--thread`)
expands the selected thread through retained catalog parent links in the same
read snapshot. Omission is own-only for compatibility. Distinct node identities
are selected, not cumulative parent/tree amounts. The node set includes the
requested thread even if its catalog row is missing, but does not invent missing
parent-child edges. `historyComplete` remains false.

The same recursive predicate is used for raw readiness seeds, stale selected
rows and cached reads. Direct resolution closes source-key counterparts before
final account/project/model/tree filtering, so a peer outside the selected tree
cannot hide a conflict. Cyclic catalog roots and trees exceeding 10,000 identities
fail without a truncated total. Existing source-record and bucket limits remain.

The supplementary scope view offers an explicit descendant checkbox; its result
caption uses the applied query, and response validation rejects own/tree mismatch.
A descendant scope with unresolved records withholds its amount even if the
parent's own scope is available. This adds scope selection, not full child-name
search/navigation or recovery of deleted hierarchy metadata.
