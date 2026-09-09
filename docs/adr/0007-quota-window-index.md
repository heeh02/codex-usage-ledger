# ADR 0007 — Durable normalized quota-window index

Status: implemented in source; code-owner review and real-shadow acceptance pending before release.  
Date: 2026-09-07  
Parent: [approved account-first goal](0006-account-first-usage-experience.md).

## Context

The quota preview repeatedly decoded the latest normalized snapshot JSON. That
path cannot support efficient full-history browsing. Snapshots are already
durable: normalization needs an index, not another source collector or a new
account identity system. The preview remains bounded until its full-history
pagination contract is implemented; index completion does not complete ADR 0006.

## Decision

Schema 36 adds `quota_window_observations` and `quota_window_index_state` only.
Window observations reference immutable snapshot IDs and retain the supplied
account scope, auth epoch, observed timestamp, normalized stream key, pool
metadata, window role/ordinal, used percentage, duration and reported reset.
Stream-key construction is shared by direct and indexed readers. No Token
amounts, request ownership, quota grants or reset causes are inferred here.

Snapshot append and window projection commit in one transaction. Identical
re-appends are idempotent. An existing projection must match every projected
column; disagreement aborts instead of silently overwriting history. Invalid
percentage values cannot become valid quota values; unsigned duration metadata
is retained as decimal text without narrowing it into SQLite signed integers.

Migration captures the pre-upgrade snapshot rowid high-water mark. Historical
backfill commits a bounded batch of projections and its cursor atomically.
Live appends index themselves and do not advance that historical cursor, so a
new observation cannot skip an older unfinished batch. Late source timestamps
remain ordered by observed time in the read index while still being captured by
the append path. Once complete, a backfill tick performs no writes.

The rowid cursor is an upgrade-work cursor on this application's append-only
snapshot table, not an inference ID. Deleting/replacing snapshots or rewriting
their rowids during backfill is unsupported and requires a separately reviewed
repair receipt. This change does not authorize that operation.

Daemon and dashboard-service writers advance up to 200 historical snapshots on
startup and periodic maintenance ticks. A batch error rolls back and leaves the
direct snapshot reader available. When all pre-upgrade rows have been indexed,
the preview reads the indexed windows instead of decoding snapshot JSON. Reads
reuse an existing bundle transaction, or open their own consistent read view;
they never start a nested transaction inside a frozen bundle snapshot.

## Compatibility and boundaries

- Original snapshots, Token ledgers, rollups and account ownership are unchanged.
- No HTTP/TypeScript DTO change is introduced by the storage index itself.
- `LedgerStore::backfill_quota_window_index_chunk` is a bounded maintenance
  operation for the package's separate service binary, not a new source SDK.
  Quota normalization and repository types stay private.
- Existing schema-35 readers must not open a schema-36 database. Do not downgrade
  its version field. Installation still requires the goal's migration/release gates.
- The current 20-interval/latest-1,000-snapshot preview limit still applies to the
  reader, not the index: all retained snapshots can be projected incrementally.
- Full-history keyset paging, durable interval segmentation/repair, index-health
  presentation, and complete grant/cycle acceptance remain unfinished.

## Rejected alternatives

- Reparse all historical JSON on each page load: unbounded repeated work.
- Advance history cursor on a live append: can skip unprocessed older snapshots.
- Mark complete after a partial batch or overwrite conflicting rows: hides gaps
  or corrupts provenance.
- Sum window observations into Token usage: percentages and Token facts have
  different meanings; this index never writes Token totals.

## Verification required

Synthetic schema-35 upgrade preserves original snapshot digests; more than 1,000
snapshots survive bounded processing and disk reopen; live/late appends remain
idempotent without skipping older rows; forced append/backfill failures roll
back rows and cursor; projection conflicts remain errors; direct and indexed
preview results match under a shared query clock and bundle read transaction.
Real-shadow migration receipts and native/installed acceptance remain separate.
