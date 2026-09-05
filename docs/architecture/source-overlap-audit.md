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

Audit format version 2 adds per-row `sourceModel` and `comparison`: candidate ID,
timestamp, thread/model, raw token dimensions, validity, global link count and
field-level mismatch labels. A dangling link retains its candidate ID but has
null counterpart usage. Positive `candidateMinusSourceNanoseconds` means the
candidate is later; null means an exact nanosecond delta is unavailable.
Unknown source usage is not compared numerically against a candidate's usage.
Shared links keep their shared-candidate status even when other differences
also exist. Candidate amounts are diagnostic references and are never included
in group totals. Consumers must check `auditVersion`; no HTTP schema or persisted
data migration is introduced by this CLI diagnostic format change.

Version 3 adds `dayPolicyContexts` and per-row `policyDay` /
`retainedSideSelectedByDayPolicy`. The contexts recompute the current
`max_thread_day_v1` decision from stored day rollups without updating its cached
projection. They cover the complete storage day (Shanghai day keys), thread,
all accounts and all models, not the narrower audit window/filter. These totals
must not be displayed as the user's selected-period totals or added to page
groups. A false row flag means the retained side is not selected, not proof
that the request is absent from reconstruction; unknown quality has no flag.

## Confirmed policy counterexample, not repaired historical data

A synthetic fixture defines independent requests A=100, B=200, C=300. Retained
evidence has A/B and reconstruction has B/C. The actual current max policy
selects reconstruction=500 rather than the known fixture truth=600, and a model
used only by A displays zero rather than 100. The v3 audit shows A's 100 retained
tokens excluded by a decision comparing 300 with 500 across the whole day, even
when the audit selects only A's model and one-second window. This is a known
accounting defect, not validation of max as an accurate algorithm.

Changing max to simple addition would instead produce 800 in the same fixture.
Replacement therefore needs request-level overlap and source-coverage evidence,
cross-dimension shadow comparisons and a historical migration receipt. A missing
candidate link alone does not prove that two observations are independent.
The counterexample regression intentionally captures the old policy's loss;
it must be replaced by a truth-conservation regression when selection changes.

Tests exercise mixed states across equal-time pages, account/model scope,
confirmed-usage conservation, invalid bounds, no writes, missing-file refusal,
unsupported-schema refusal and the actual CLI output/paired-cursor validation.
The unsupported-schema test is a version guard test, not a historical migration
acceptance receipt. Real-ledger retrospective repair, global overlap coverage,
alternative source selection and GUI diagnostics remain separate goal work.
