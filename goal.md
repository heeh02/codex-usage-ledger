# Open-Source Collaboration Goal

Status: **ACTIVE**
Started: 2026-09-04
Previous completed product goal: [`docs/archive/goals/2026-09-04-product-goal.md`](docs/archive/goals/2026-09-04-product-goal.md)

Safety hold resolution: the public target was recreated as a fresh **Private**
repository from the sanitized source tree. Its first CI run passed the complete
reachable-history privacy gate. The pre-governance remote remains a separate,
read-only private archive and is not reachable from the public target. Public
visibility still waits for the tagged release gate and artifact verification.

## Outcome

Make Codex Usage Ledger safe and practical for multiple human contributors and
Codex agents to develop in parallel without weakening accounting correctness,
privacy, database compatibility, or the installed macOS application.

## Non-negotiable product invariants

- Official account totals and local project attribution remain separate ledgers.
- Never sum Codex cumulative per-thread counters.
- Token arithmetic remains mutually exclusive: uncached input + cache read +
  cache write + output = total; reasoning is an output subset.
- Unknown, pending, quarantined, or unrecoverable usage never becomes fabricated
  zero or confirmed usage.
- The app never writes Codex credentials or refreshes OAuth state.
- The dashboard remains loopback-only and the macOS shell remains a lifecycle
  and presentation adapter.
- Existing schema 22 databases must migrate to schema 24 without changing
  historical totals; schema 24 audits persisted facts and rejects new
  non-conserving confirmed rows, including durable rollups.

## Workstreams

### Phase A — Public repository safety and governance

- [x] Add root and module-scoped `AGENTS.md` files.
- [x] Add contribution, conduct, governance, ownership, issue, and PR policies.
- [x] Add editor, line-ending, toolchain, changelog, and generated-file rules.
- [x] Classify current contracts, architecture, release evidence, and history.
- [x] Replace real project/account/path evidence in current public documentation
      with synthetic examples and add an automated privacy gate.
- [x] Recreate the public target without legacy refs or releases and prove the
      full-history privacy gate on the resulting one-root history.
- [x] Protect `main` with required pull requests and strict CI checks.

### Phase B — Behavior-neutral Rust boundaries

- [x] Split `api.rs` into routes, DTOs, query services, reconciliation, explorer,
      quota, and presentation modules without changing JSON contracts.
- [x] Split `store.rs` into migrations and repository modules without changing
      migration order or transaction semantics.
- [x] Move dashboard SQL behind intent-specific store read models and enforce
      the API/storage boundary in CI.
- [x] Reduce the public Rust API to intentional contracts.
- [x] Move large inline integration tests out of production modules where this
      reduces merge conflicts without losing private-unit coverage.

### Phase C — Web and macOS boundaries

- [x] Split the Web app by Overview, Projects, Sessions, Accounts, and Quality.
- [x] Split the global CSS into tokens, base, layout, shared components, and
      feature styles while preserving responsive screenshots.
- [x] Replace inline bilingual strings with keyed locale catalogs and enforce
      locale-key parity.
- [x] Add a typed bridge boundary for WKWebView messages.
- [x] Split the macOS service controller by lifecycle, runtime paths, and process
      diagnostics; add testable pure state transitions.

### Phase D — Contracts, tests, and release gates

- [x] Make typed Rust response DTOs the API source of truth and generate or
      mechanically verify the TypeScript contract.
- [x] Add contract fixtures, Web unit tests, responsive browser tests, Swift
      tests, documentation links, privacy scanning, and version consistency.
- [x] Add license/SBOM checks and pin third-party GitHub Actions.
- [x] Create a reproducible tagged release path with checksums and signing
      boundaries documented separately from notarization evidence.
- [x] Build tagged Linux x86-64 and Windows x86-64 CLI/local-service packages
      alongside the Apple Silicon macOS GUI artifact.

### Phase E — Acceptance and delivery

- [x] All 93 Rust library tests, 3 binary tests, and the API-schema integration
      test pass with the accounting invariants intact.
- [x] Web typecheck/build and new unit/E2E gates pass in both locales.
- [x] macOS app builds, deep-signature verification passes, and the installed app
      opens the existing ledger in daemon/live mode.
- [x] A clean checkout can follow `CONTRIBUTING.md` without maintainer-only paths.
- [x] Independent review reports P0=0 and P1=0 for repository contribution,
      module boundaries, privacy, and release safety.
- [x] Install the verified schema 24 app and reopen the existing ledger in live
      mode without changing Token totals.
- [ ] Publish and verify the tagged macOS/Linux/Windows release, restore Public
      visibility, and remove only rebuildable caches and temporary copies.

## Change control

- Deliver the goal as small, reviewable commits; do not combine governance,
  schema behavior, UI redesign, and cleanup in one patch.
- First split files with behavior unchanged. Algorithm changes require a later,
  explicit issue and invariant tests.
- Keep `main` usable after every phase.
- Record evidence and remaining external limitations in `docs/releases/`.
