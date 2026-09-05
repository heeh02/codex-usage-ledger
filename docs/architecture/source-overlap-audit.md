# Read-only source-overlap audit

`audit-overlap` is a bounded diagnostic command, not an importer or a replacement
accounting algorithm. It opens the specified existing ledger read-only, checks
for the supported schema, and refuses older/newer schemas without migrating.
It never calls startup preparation, projection refresh or source discovery.
SQLite WAL coordination may require normal access to its sidecar files; do not
use `immutable=1` to bypass a live database's consistency rules.

```sh
codex-usage-ledger audit-overlap --db ./synthetic-ledger.sqlite3 \
  --thread synthetic-thread --start 2026-01-01T00:00:00Z \
  --end 2026-02-01T00:00:00Z --limit 100
```

The window is half-open UTC. Optional `--account` and `--model` select exact
current-ledger account and model identifiers; omit them for all records in the
thread. This is own-thread retained evidence, not child-inclusive/project
totals. Page sizes are 1–500. Continue using both `--after-time` and `--after-id`
from `next.effectiveAt`/`next.eventId`. Equal timestamps do not discard rows.

Each page is read within one SQLite snapshot. Pages do not share a frozen
revision: restart the traversal after evidence/assignment changes. Group counts
and token amounts cover only that page. `confirmedUsage` is null for an unknown
observation or a category with no confirmed observations, not a fabricated zero.
Raw token dimensions retain input inclusive of cache, cache-read/write counts
and explicit write-coverage weight; reasoning remains inside output. A missing
write-coverage weight does not establish a measured zero cache-write amount.

Categories distinguish unlinked requests, unavailable targets, unverifiable
timestamps, different evidence, shared candidates and consistent candidates.
Sharing is checked against all links, including candidates outside the current
page. Consistency checks time, thread/model and all token components; it is not
proof of request equality. Neither candidate totals nor a guessed corrected
union are added to the report. `requestEqualityProven` and `historyComplete`
remain false. Existing source selection, retained facts and token totals are
not modified; legacy evidence without links stays unclassified as unlinked.

Tests exercise mixed states across equal-time pages, account/model scope,
confirmed-usage conservation, invalid bounds, no writes, missing-file refusal,
unsupported-schema refusal and the actual CLI output/paired-cursor validation.
The unsupported-schema test is a version guard test, not a historical migration
acceptance receipt. Real-ledger retrospective repair, global overlap coverage,
alternative source selection and GUI diagnostics remain separate goal work.
