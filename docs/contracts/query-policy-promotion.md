# Request union query policy promotion

Schema 41 adds `usage_query_policy`. Existing and new databases default to
`max_thread_day_v1`; schema upgrade alone never promotes historical evidence.
Source event tables and existing migration definitions are unchanged.

After source review and incremental projection preparation, an operator may run
`codex-usage-ledger promote-union --db /path/to/reviewed-ledger.sqlite3`.
The command requires an existing current-schema database. Pending, unresolved or
incomplete projections reject activation. It stores `request_union_v2` and the
first activation timestamp atomically with connection-local query routing.
It does not repair, delete, reattribute or import events. Readiness is not a
substitute for reviewing source correctness.

Restart collector/dashboard processes after promotion. Normal writable store
startup restores the selected query views even when new ingestion is pending;
queries then wait for readiness instead of reverting to daily maxima. Existing
connections are not hot-switched. Collector and dashboard maintenance advance
bounded union batches for active union connections only. Cursor progress remains
in the existing projection tables, so restart does not reset historical work.

No HTTP response shape changes. The existing usage-policy field reports
`request_union_v2` on union connections. Read-only diagnostic constructors keep
their explicitly selected diagnostic behavior. Promotion does not change official
account totals, authenticate accounts, redeem resets or publish an application.
