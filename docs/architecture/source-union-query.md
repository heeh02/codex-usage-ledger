# Candidate union query contract

Status: candidate reader, not an active dashboard-policy switch. It reads the
[durable union projection](source-union-projection.md), never native logs,
original evidence tables or the old day-max aggregates.

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

One read transaction first checks projection readiness, policy version and
unresolved groups, then generates the summary, calendar buckets and complete
account/project/model/thread distributions. Pending indexes return `pending`;
any unresolved group returns `unresolved`, even when a chosen dimension could
hide its conflicting counterpart. Both states withhold amounts. This global
guard is intentionally conservative and is not a per-scope completeness model.

Resolved, empty scopes return `no_records`, with nullable usage rather than
measured zero. A selected zero-amount observation returns `available`, counted
once. `historyComplete=false` and `productionPolicyChanged=false` always remain
explicit: resolved retained identities are not proof of complete inference.

Canonical sampling timestamps already selected by the union govern [start,end).
Timezone conversion precedes calendar bucketing; weeks start Monday, months and
years use their calendar boundaries. Repeated local hours retain UTC offsets.
All seven raw fields and event counts conserve independently in every dimension.
No official total, residual allocation or extra reasoning sum enters this query.

Schema 40 adds a time-range index on the selected projection for all-account
queries. Reading seeks only the selected interval and streams matching rows;
it never rebuilds projections. A 10,000-bucket-per-dimension cap bounds result
memory and fails explicitly instead of truncating. Query cost still grows with
the selected interval; this is not a constant-time or benchmarked UI guarantee.

This reader establishes query parity before promotion. Summary, chart, ranking,
chat and quota production consumers still require a reviewed common-policy
switch, real-account comparison and controlled migration receipts.
