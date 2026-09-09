# Changelog

All notable user-visible changes will be documented here. The project follows
semantic versioning once a public contract is declared stable.

## Unreleased

## 0.2.0

- Account-first navigation, M Token units, project/chat/subagent and model drilldown.
- Bilingual responsive trends, full rankings and retained quota interval charts.
- Request-level source union with incremental processing and explicit history gaps.
- Exact-match migration tooling, recoverable installation and non-destructive startup.
- Snapshot rollup reuse and queued-query cache reuse for faster dashboards.
- macOS native application and Linux/Windows CLI plus local Web dashboard packages.

### Limitations

- Missing historical evidence is not recovered or allocated to projects.
- Official account totals remain separate from local attribution.
- macOS is ad-hoc signed, not notarized. Linux/Windows use the browser dashboard,
  not a native GUI; package availability depends on successful release CI.

## 0.1.0

### Added

- Open-source contribution, governance, ownership, and agent working agreements.
- Normative architecture, dependency, privacy, and documentation classifications.
- Automated public-privacy, documentation-link, version, and generated-file gates.
- A README screenshot generated exclusively from synthetic demo data.
- Deterministic third-party license receipts embedded with Rust and Web SBOMs
  in every platform package.

### Fixed

- Schema 24 now audits persisted Token invariants, safely normalizes legacy
  reconstruction cache-write coverage metadata without changing Token totals,
  and guards confirmed durable rollups against non-conserving writes.
- Old confirmed events take the same invariant checks before direct compaction,
  with cursor advancement remaining atomic on rejection.
- Release builds run native Linux/Windows tests and binary smoke checks; only a
  source-free final publisher job receives repository write permission.

## 0.1.0 — 2026-09-04

- Initial replay-safe local ledger, official account reconciliation, project and
  Session/Subagent explorer, quota views, bilingual responsive dashboard, and
  standalone Apple Silicon macOS application.
