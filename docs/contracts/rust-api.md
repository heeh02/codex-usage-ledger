# Rust API boundary

The crate is an application core, not a general-purpose Codex SDK. Its intended
public surface is deliberately small:

- `api`: loopback router/state, query input, response DTOs, and generated wire contract;
- root accounting types: `TokenUsage`, `UsageEvent`, attribution and quality enums;
- root ledger access: `LedgerStore`, aggregate filters/dimensions, collector
  status, and the typed `LedgerTableCounts` diagnostic result.

Source parsers, identity resolution, replay guards, reconstruction, runtime
orchestration, quota normalization, and repository implementation modules are
private. The hidden `cli_support` re-exports only what the package's separate
binary crate needs and is not a stable third-party interface.

`LedgerStore` exposes typed operations rather than its SQLite connection. CLI
diagnostics use `ledger_table_counts`; schema details remain crate-private.

The package service also calls the bounded maintenance operation
`LedgerStore::backfill_quota_window_index_chunk(limit)`. It returns completion of
the captured pre-upgrade snapshot target, not source-history completeness. New
snapshots project within their append transaction. Quota-window repository and
normalization types remain private; see
[ADR 0007](../adr/0007-quota-window-index.md) for the migration/review boundary.

`backfill_quota_history_chunk` advances versioned boundary projection;
`quota_history_page` reads stable seek pages for one account or `all` without
changing accounting facts. Cursor/page types are available to the package binary
through `cli_support`. See [ADR 0008](../adr/0008-versioned-quota-history.md);
this is not a guarantee of complete real-world collection or verified grants.

Changing the intended public surface requires an ADR. Do not make an internal
module public solely to simplify a test; place unit tests beside the module or
add a narrow public contract when external use is intentional.
