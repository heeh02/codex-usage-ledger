# Streaming reconstruction comparison

`audit-reconstruction-file` extends the
[bounded prefix diagnostic](reconstruction-prefix-audit.md) to one complete
source file when it fits explicit byte/Token-record budgets. It shares the same
target containment, canonical-ID checks, reconstruction parser and per-record
comparison logic; it never runs ingestion or changes source bindings.

A [sealed correction draft](reconstruction-correction-draft.md) can now retain
the individual comparison records for review and revalidate their expected old
values against a read-only ledger. This does not apply corrections.

```sh
codex-usage-ledger audit-reconstruction-file --db ./synthetic-ledger.sqlite3 \
  --codex-home ./synthetic-codex --thread synthetic-root \
  --max-bytes 1073741824 --max-token-rows 1000000
```

The dedicated source-audit opener permits the unchanged evidence tables in
schemas 35–38, with SQLite read-only/query-only access. It will not create,
migrate, optimize, refresh projections or discover authentication. Other CLI,
HTTP and storage readers retain their current-schema requirement. This explicit
allowlist must be reviewed rather than automatically expanded by a future
schema bump. Missing/incompatible tables fail; there is no fallback importer.

## Work bounds and evidence scope

- At most 2 GiB and 10 million Token records per invocation; default limits are
  1 GiB and 1 million. File reads advance in chunks of at most 4 MiB. Partial JSON
  state is bounded by the shared 64 MiB line limit. Output aggregates categories
  rather than retaining every record/prompt in memory.
- One read transaction fixes ledger facts and attribution for the scan. Source
  files are not frozen. Before/after identity, size and modification metadata
  detect ordinary source changes; a tail reset fails instead of restarting.
- The diagnostic has no durable resume cursor. It streams once per invocation;
  stopping it does not mark any audit work complete. This does not change the
  separate collectors' durable incremental behavior.
- Every old fact found at a processed Token-record position is counted once.
  The complete source-scoped stored count is compared with visited positions.
  EOF alone cannot conceal an old fact at a missing/shifted position.
- `allStoredPositionsSeen` requires EOF, canonical metadata, no malformed
  records, no observed metadata change and no unvisited stored positions. It is
  **not** Token equality, source identity, replay freedom or history completeness.

`processedRecordsDigest` hashes framed complete lines (offset, length, raw line),
not the entire raw file. It excludes unprocessed/trailing partial data. Do not
present it as the original file's historical SHA-256 or an identity-rebind proof.
Concurrent in-place rewrites can exceed the protection of metadata checks.

## Comparison semantics

Output scope is `streamed_source_comparison_not_a_migration_receipt`, version 1.
Categories are `unchanged`, `changed_candidate`, `suppressed_candidate`,
`new_candidate` and `not_emitted`. Each has record counts and nullable stored/
proposed usage. An absent side stays null, including an entire empty scan.
All additive components use checked arithmetic; cache fields stay within input,
reasoning within output. Counter prefixes are separate bookkeeping.

`usageChangedPairs` counts only pairs with both versions present. It must not be
interpreted as the count of all corrections: suppressed/new observations are
separate categories. `suppressedByRule` explains the current parser's rule for
each stored-but-unemitted row: foreign history, child prefix, unchanged counter,
initial/reset counter without valid last usage, awaiting canonical metadata, or
an explicitly unresolved unknown/zero reason. These are parser explanations,
not server-certified billing decisions. Their counts/components must sum to the
suppressed category. Missing legacy source keys remain separately counted.

The command always reports `migrationReady=false` and `historyComplete=false`.
Stored/proposed category amounts must not be added together or automatically
subtracted from production totals: source-overlap selection may cover the same
record elsewhere. Neither full-file agreement nor a parser suppression authorizes
historical deletion, inferred account assignment or identity rebinding.

Tests cover a multi-chunk file beyond the prefix cap, late new records, visited
versus orphaned positions, inherited-history suppression, nullable category
amounts, row/byte limits, partial JSON, old-schema refusal/acceptance without
migration and source/index/ledger preservation. Real-source receipts stay outside
the public repository. Historical repair still requires those receipts, replay
validation, union reconciliation and code-owner review.
