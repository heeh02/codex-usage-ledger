# Architecture overview

Codex Usage Ledger is a local-first macOS product with three independently
testable layers and two deliberately separate ledgers.

```text
Codex read-only sources
  app-server account usage ───────────────► official account ledger
  state session index ─┐
  post-sampling rows ──┼─► ingest/replay/reconstruction ─► local attribution ledger
  retained rollouts ───┘                                  │
                                                           ▼
Rust query and reconciliation services ─► loopback JSON/SSE API
                                             │
                                             ▼
                                  React dashboard and explorer
                                             │
                                             ▼
                               native macOS lifecycle/window shell
```

The official ledger answers how much an account used over a backend-covered
period. The local ledger explains retained machine evidence by project, root
session, subagent, model, and time. Their values may differ because their scope
and retention differ. Reconciliation presents that difference; it never hides
it by inventing project allocation.

## Rust core

- Source adapters parse Codex-owned read-only inputs.
- Identity and project resolution attach temporal account and project evidence.
- Replay and reconstruction convert cumulative or duplicated source events into
  immutable usage facts without summing inherited thread history.
- Store repositories persist facts, cursors, revisions, and migrations.
- Query services aggregate one fact domain at a time.
- Presentation DTOs form the versioned HTTP contract.
- Runtime owns polling, cancellation, bounded work, and loopback serving.

## Web dashboard

The browser layer consumes typed DTOs and owns filtering, exploration,
visualization, accessibility, responsiveness, and localization. It does not
reconstruct usage or reinterpret unknown evidence. Demo mode uses only synthetic
fixtures and must exercise the same view models as the live client.

## macOS shell

The native layer owns menu-bar and window presentation, process lifecycle,
runtime path discovery, safe local navigation, and native preferences. It does
not implement accounting. Messages crossing the WKWebView boundary are a typed,
allowlisted contract.

## Change strategy

Large files are split by moving behavior behind these boundaries before logic is
changed. A pure move must keep JSON fixtures, database schema, and accounting
tests byte-for-byte or semantically equivalent. Algorithm changes require a
separate pull request with synthetic RED/GREEN evidence.
