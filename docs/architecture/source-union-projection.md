# Incremental local measurement union candidate

Status: implemented staging projection; **not the production accounting policy**.
It is the durable replacement-path implementation for the
[day-max counterexample](source-overlap-audit.md), using the existing
[local-record union planner](source-union-shadow.md). No HTTP/dashboard consumer
has been switched. Shared local-record identity is not server-request identity.

## Storage and compatibility

Schema 38 adds only `measurement_union_*` derived tables, indexes and triggers.
It does not rewrite source observations, assignments, existing rollups or the
active source selector. The migration records the indexed maximum event ID from
each evidence side; it does not scan/import all historical records. Genuine
schema-37 upgrade and fresh-database paths are tested. Read-only open still
requires the current schema. Real migration receipts and code-owner review are
required before installation; synthetic migration tests do not satisfy that gate.

Schema 39 adds `policy_version=2` to candidate diagnostic counts for the
planner's version-3 coverage reconciliation. It resets only the two historical
seek checkpoints to new indexed high-water IDs. Existing candidate/source rows
remain in place, but readiness becomes false while any retained input awaits
bounded reconsideration. No old migration is edited and no source amount is
rewritten. A genuine schema-38 upgrade test starts with an old coverage-only
conflict, verifies pending state, resumes processing, preserves all source facts,
and confirms no writes on the next completed tick. Policy version alone does
not prove every group has been reprocessed: readers must also require readiness.

`measurement_union_backfill` persists a seek position and fixed initial high-water
ID for each side. `measurement_union_dirty` is a unique queue of affected groups.
`measurement_union_groups` records member count and an explicit unresolved reason.
`measurement_union_selected` contains a single canonical observation per resolved
group, with typed time/account/project/model/thread and all Token components.
`measurement_union_counts` maintains diagnostic counts transactionally, avoiding
a growing full-table recount on every batch/status read.

There are three group namespaces: a shared record key, an unkeyed sampling ID,
or an unkeyed reconstruction ID. They cannot collide. Unkeyed observations remain
unresolved; equal Token values or nearby timestamps alone never establish a key.
Missing thread IDs are also unresolved instead of aborting the entire scan or
being invented as an ordinary thread. Existing shadow CLI reports may now expose
the additive `missing_thread` reason; empty thread strings represent missing IDs.

## Incremental transaction

1. Seek at most the requested historical scan budget across the two sides.
   Enqueue the current groups and persist the last scanned IDs.
2. Read a bounded page of dirty groups. For each group, fetch all existing
   counterparts using the shared-key index before applying any dimension filter.
3. Resolve with the existing planner. Replace only that group's candidate row(s),
   preserving all seven Token fields. Conflicts withdraw the previous selection
   and retain their reason, never a fabricated zero.
4. Remove processed queue entries and commit together with the cursor and counts.

Fact/assignment/key insert, update and delete triggers enqueue the affected
groups within the original write transaction. Key changes enqueue old/new keyed
groups and the unkeyed singleton. Thus late records on either side of the initial
cursor, late counterparts, changed assignments and key removals are reconsidered.
Idempotent queue insertion uses explicit `ON CONFLICT DO NOTHING`, not a trigger
`OR IGNORE` policy that an outer upsert can override.

Scan/group budgets are each 1–1,000; the counterpart-observation budget is
1–10,000 **across the whole batch**, not per group. A group is never truncated.
If earlier groups consume the budget, they commit and the next intact group waits
for a fresh budget. An oversized first group fails and rolls back the entire
batch. Stale provenance links can still require additional index work; these
limits are not a fixed millisecond latency guarantee.

Restart resumes the saved cursor/queue. A complete unchanged projection performs
no row writes and no historical scan. Counts may describe a stale staged result
while the queue is nonempty: `projectionReady=false` must gate any future reader.
`projectionReady=true` means the supplied retained evidence has been processed,
**not** that it is all resolved or that source history is complete. Inspect
`unresolvedGroups` separately. No totals are published by the status command.

## Explicit diagnostic operation

```sh
# Read-only status; will not create or migrate a database.
codex-usage-ledger union-projection --db ./synthetic-ledger.sqlite3

# Explicit derived writes, only on an existing current-schema diagnostic ledger.
codex-usage-ledger union-projection --db ./synthetic-ledger.sqlite3 --advance --batches 10
```

Each CLI batch allows 200 scanned records, 200 groups and 10,000 counterpart
observations. `scannedRecords`/`recomputedGroups` describe the final executed
batch; the other counts describe its resulting snapshot. The command stops early
when ready. It does not collect Codex files, compact history, enable a background
job, migrate an older database, replace the installed app or change active policy.
The emitted scope is `staged_local_measurements_not_inference_usage` with
`productionPolicyChanged=false` and `historyComplete=false`.

## Acceptance and outstanding gates

Synthetic tests cover source-overlap deduplication with sampling-only models,
all-component conservation in every dimension, late cross-midnight/month pairing,
account conflicts, missing assignments/keys/thread IDs, recorded zero, oversized
groups, rollback, bounded schema-37 backfill, late inserts outside the cursor,
reopen/no-op behavior, derived count conservation and unchanged source facts.
CLI tests enforce explicit writes and read-only/missing/older-schema behavior.

Before promotion, validate copied/replayed identities and legacy missing keys;
compare candidate scope totals and every component with retained evidence;
establish coverage and supported query semantics for unresolved groups; preserve
timezone/exact-boundary behavior and retained-detail parity; record controlled
real-shadow migration and two-account receipts. Then switch summary, series,
rankings, conversation and quota readers together to one reviewed selection
policy. Until then, the existing day-max defect remains a release blocker.

The [candidate query reader](source-union-query.md) now supplies same-snapshot
summary/time/dimension parity, guarded by completed staging and zero unresolved
groups. Schema 40 adds its all-account time-range index. This read path still
does not switch any production consumer or establish legacy identity eligibility.
