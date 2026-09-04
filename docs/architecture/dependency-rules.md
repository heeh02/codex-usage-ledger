# Dependency rules

Dependencies flow inward from adapters and presentation toward stable domain
contracts. Reverse edges require an ADR.

```text
Codex sources ─► adapters ─► domain facts ─► repositories
                                      │            │
                                      └────► query services ─► API DTOs ─► Web

macOS shell ─► process/HTTP/WKWebView contracts; never ─► accounting internals
```

## Allowed boundaries

- Parsers may depend on domain types, never on API presentation types.
- Replay and reconstruction may depend on parsed source facts and explicit
  cursor state, never on React filters or display rounding.
- Repositories expose intent-specific operations; callers do not embed SQL.
- Dashboard reads use the typed read models in `src/store/dashboard_repository.rs`;
  `src/api` cannot depend on `rusqlite` or request a raw database connection.
- Query services may combine repository results only according to the accounting
  contract. They do not mutate source state.
- API routes validate input and map errors; aggregation belongs in services.
- Web feature modules depend on the generated/verified API contract and shared UI
  primitives, not on one another's internal state.
- The macOS shell starts and stops the core and hosts the dashboard. It cannot
  read Codex databases directly.

## Module ownership

| Area | Owns | Must not own |
|---|---|---|
| `src/ingest`, `sampling`, `reconstruction`, `replay` | source normalization and replay safety | product copy or HTTP layout |
| `src/store` | migrations and persistence | account/project inference policy |
| `src/api` | request validation and stable DTOs | source scanning or SQL |
| `web/src/api` | transport and contract types | accounting arithmetic |
| `web/src/features` | page-specific view state | database or auth access |
| `macos/Sources` | native lifecycle and bridge | token aggregation |

## Conflict-minimizing changes

Keep a pull request inside one ownership row when possible. Contract changes may
touch producer, fixture, and consumer together, but must not also include a
visual redesign or schema cleanup. Mechanical moves precede behavior changes so
reviewers can distinguish relocation from semantics.
