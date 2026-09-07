# Retained hash provenance and safe backfill

The raw event hash identifies the serialized ingestion record, not necessarily
its current database projection. Existing project-reprojection SQL can change
project fields without changing that ingestion hash. Previously request backfill
decoded the projected raw row and called the ordinary ingest retention helper,
which recomputed a hash. The resulting retained hash could differ from the raw
identity even with identical token/time/model fields and cause guarded compaction
to fail. The synthetic regression captures this sequence before the fix.

New backfill inserts missing retained evidence directly from the raw row,
including its persisted hash. It fills missing origin/assignment rows but never
overwrites existing retained observations or reviewed assignments merely because
another companion row is absent. Per-batch transaction/cursor semantics remain
unchanged. No historical hash update or database migration runs automatically.
The normal compaction mismatch predicate remains in force.

## Read-only diagnostic

```sh
codex-usage-ledger audit-retained-hashes --db ./synthetic.sqlite3 \
  --after-rowid 0 --limit 1000
```

The existing-schema read-only opener and one read snapshot per page are used.
`limit` is 1–1000; the cursor is a nonnegative raw rowid. Follow `nextAfterRowid`
until null. Pages must be traversed on an idle consistent ledger copy; they are
not pinned to one cross-process revision. Missing pairs are outside the audit,
not counted as aligned. The command does not import sources, read credentials,
migrate or refresh derived selectors.

Version 1 reports paired row counts and hash mismatches. Only mismatching pairs
are reserialized with the current Rust serializer; the two match counters say
whether that result equals the stored raw or retained hash. No row IDs, account
keys, model names, paths or token values are printed (only the numeric cursor).
`repairAuthorized=false` always. A current-serialization match is consistent
with a prior metadata rehash but does not independently prove its historical
cause, source identity, token correctness or permission to rewrite old hashes.

Synthetic tests cover paging, unchanged rows, rehashed projections, read-only
behavior, invalid bounds, preserved reviewed assignments, and successful guarded
compaction following a new backfill. Previously generated mismatches remain for
a receipt-controlled repair. Do not disable compaction guards or bulk-copy hashes
to make them disappear.
