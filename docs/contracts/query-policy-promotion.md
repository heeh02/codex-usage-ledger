# Request union query policy promotion

Schema 41 adds `usage_query_policy`. Existing and new databases default to
`max_thread_day_v1`; schema upgrade alone never promotes historical evidence.
Source event tables and existing migration definitions are unchanged.

Schema 43 adds an explicit `--allow-incomplete-history` promotion option. The
default remains strict. With this option, a fully materialized ledger may expose
only confirmed selected records while retaining unresolved groups outside totals.
Pending projection work still blocks reads. The choice persists across restart;
data quality reports a global gap count with unknown Token quantity, and local
pages display an incomplete-history note in both languages. Official account
totals are unchanged. Quota interval review uses the requested account/time scope,
so permitting available history does not falsely certify a gap-containing cycle.
This option does not reconstruct, delete, estimate or relabel missing records.

After source review and incremental projection preparation, an operator may run
`codex-usage-ledger promote-union --db /path/to/reviewed-ledger.sqlite3`.
The command requires an existing current-schema database. Pending or incomplete
materialization rejects activation; strict mode also rejects unresolved groups.
It stores `request_union_v2` and the
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
