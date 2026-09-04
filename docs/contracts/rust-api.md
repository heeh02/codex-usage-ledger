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

Changing the intended public surface requires an ADR. Do not make an internal
module public solely to simplify a test; place unit tests beside the module or
add a narrow public contract when external use is intentional.
