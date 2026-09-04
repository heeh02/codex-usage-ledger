# Rust Core Instructions

The Rust core owns evidence ingestion, accounting, persistence, reconciliation,
and the loopback API. It does not own product copy or macOS presentation.

## Boundaries

- Domain types and arithmetic must not depend on Axum, rusqlite, filesystem paths,
  Codex process discovery, or localized labels.
- Storage code persists typed domain facts and must not construct HTTP JSON.
- Ingest adapters may parse Codex-owned files read-only; keep source-specific
  formats outside accounting policy.
- HTTP routes validate transport and delegate to query services. Do not add SQL
  or accounting rules to handlers.
- Keep transactions explicit. Cursor advancement and the facts it covers must
  commit atomically.

## Database changes

- Published migrations are append-only. Never edit an old migration to make a
  new database pass.
- Test both a new in-memory database and upgrade from the previous schema.
- Never delete or relabel confirmed history without an explicit migration receipt.

## Tests

- Parser fixes require synthetic fixtures for the exact replay/truncation shape.
- Accounting changes require conservation tests across summary, time series,
  project, model, account, and session scopes.
- Prefer private unit tests beside implementation; move broad workflows and
  migration matrices to integration tests to reduce production-file conflicts.

## Code Review Rules

- Reject raw `serde_json::Value` at stable service boundaries when a typed DTO is practical.
- Reject new public exports that exist only to make one test or adapter convenient.
- Reject filesystem or network access from pure accounting modules.
