# Usage Visualization and Product Experience Goal

Status: **ACTIVE**  
Started: 2026-09-05  
Baseline: `93c6aa5dc63290a8c2e94579f81bc51cecd84e25`  
Previous goal: [completed open-source governance](docs/archive/goals/2026-09-04-governance-goal.md).

## Outcome

Make usage understandable through a continuous user workflow:

**Choose accounts and dates → inspect a correctly dated trend → select a peak →
find the contributing conversation → inspect turns, models and subagents.**

Preserve Rust, React and the native shell. Deliver in reviewable batches with
synthetic regression evidence and a verified installed application. The approved
audit is translated into the work items below; public records contain no private
project names, account identifiers, source files or usage snapshots.

## Invariants

- Official account totals and local activity have explicit, independent scopes.
- Summary, chart, ranking and composition use one applied scope and revision.
- Do not sum cumulative counters, inherited history or alternative source rows.
- Preserve input/cache-read/cache-write/output conservation; reasoning is within output.
- Unknown fields and uncovered periods are not confirmed zero.
- A positive local/official difference does not identify an unobserved account.
- Preserve account, model and dates during navigation unless explicitly changed.
- Historical source changes require shadow validation and a migration receipt.
- Keep source credentials inaccessible, sources read-only, and the API loopback-only.
- Preserve the live ledger and active dependency/build caches.

## Phase 1 — Correct query and chart facts

- [ ] A02/A03/A16: real calendar coordinates and aligned comparison grids;
      distinguish absent buckets, zero and unsupported granularity.
- [ ] A04: coverage includes effective historical evidence, is scoped, and never
      equates first/last observations with uninterrupted collection.
- [x] A05: account/model/date filters survive project and conversation navigation.
- [x] A11: all effective historical models are selectable.
- [ ] A12: validate request ranges; support natural year and custom dates with
      honest available precision and explicit errors.
- [x] A13: export local-only and official-only dates with scope, missing states
      and all token dimensions.
- [x] A19: end quota samples at the cycle boundary; avoid claiming all-account
      activity belongs to each pool; distinguish temporal span from coverage.
- [ ] Record versioned contract changes and synthetic regression evidence.

## Phase 2 — Overview and complete conversations

- [ ] A01/A17: trend-first overview, compact metrics, diagnostics in a dedicated page.
- [ ] A08/A10: searchable paginated conversations, sorted after period filtering;
      distinguish matches, directory membership and historical counts.
- [ ] Project/standalone pages show only the selected scope and conversations.
- [ ] Preserve visible rows and focus during background updates.

## Phase 3 — Conversation and subagent analysis

- [ ] A06: selecting a child changes title, usage, composition and chart together.
- [ ] A07: readable axes, exact values and time selection replace decorative bars.
- [ ] A09: complete large-tree browsing/search without silent truncation.
- [ ] Peak-to-conversation drill-down and explicit own/descendant scope.
- [ ] Conserved model/account composition for mixed-model and mixed-account chats.

## Phase 4 — Accounts, models and calendar exploration

- [ ] Dedicated account and model comparisons with common timelines and coverage.
- [ ] Day/week/month/year, previous periods, chosen year, custom ranges and heatmap.
- [ ] Account aliases, observed identity boundaries and unassigned records.
- [ ] Separate quota-cycle history, planned resets and observed resets.
- [ ] Replace unproven residual lower-bound wording with diagnostic differences.

## Phase 5 — Durable detailed evidence and incremental operation

- [ ] Persist compact turn/request identity and usage without prompt bodies.
- [ ] Retain durable turn aggregates before raw compaction; label old detail limits.
- [ ] Validate source overlap using identities and coverage, not merely larger totals.
- [ ] Shadow accounting decisions; no unexplained historical-total replacement.
- [ ] Incremental dirty-key projections and page-specific query/update work.
- [ ] Measure retention size and load behavior before setting long-term budgets.

## Phase 6 — Product acceptance

- [ ] A14/A15: readable container-responsive charts and conversation amounts at
      560/700/900/1280/1440px and 80–160% zoom.
- [ ] Unified typography, light/dark appearance, keyboard access and bilingual copy.
- [ ] A18: distinguish local refresh, official sync and background progress;
      retain last trusted data on slow/failed/out-of-order requests.
- [ ] Scoped CSV/JSON and anonymous sharing preview, no private-data exports by default.
- [ ] Test complete user tasks, including deep/large/historical trees and account changes.
- [ ] Run Rust/Web/contracts/governance gates and verify the installed macOS bundle.
- [ ] Report Windows/Linux checks separately from macOS native evidence.
- [ ] Final code-owner review of accounting, schema and release changes.

## Current checkpoint

Execution on `feat/usage-visualization`. Historical-start/model-catalog repair,
account/model navigation continuity and source-tagged CSV export are implemented.
Scoped interval completeness still remains under A04. Calendar chart coordinates,
gap breaks, date-aligned comparison and source grain labels are implemented;
full scoped coverage/comparison validation remains open. Quota sample boundaries
are fixed. Conversation pagination/search is implemented with server-side scoped
ordering, tested against 513 roots; UI controls and a browser regression are
added. Next: rerun that browser regression when localhost binding is permitted,
then validate child navigation and implement the trend-first layout. Child clicks
now request that node's own detail and preserve a return trail; a narrow-screen
parent button is included. Synthetic child fixtures now conserve descendant
totals; the in-app browser verified child title/usage/curve switching. Full
automated navigation and return verification remains open. Current screenshots still
show excessive page chrome and cramped auxiliary titles; overall GUI acceptance
is not complete.
No live data migration, installed-app replacement or release has occurred.

## Evidence log

- Goal creation: approved product audit converted into this durable execution checklist.
- Repository baseline clean; earlier governance goal archived intact.
- Batch 1: reproduced two failing historical-coverage fixtures and a missing
  reconstructed-model assertion, then fixed them. Rust 95 library + 3 binary +
  1 schema tests pass; Clippy and repository policy checks pass.
- Batch 2: browser regression first failed because the account selector
  disappeared on project navigation. After repair, account/model selection
  survives project and conversation navigation. All 6 browser checks pass.
- CSV source/precision/scope cases plus existing Web tests: 13 unit tests pass;
  typecheck and production build pass. New CSV semantics documented in the
  HTTP/export contract. No official/local totals are added by the exporter.
- Batch 3: timestamp-positioned chart series, sparse-date breaks, calendar
  comparison alignment, separate source grains and container-sized readable
  axes. Composition uses stacked buckets; project comparisons share one axis.
  Web 18 unit and 7 browser tests pass, including narrow-window keyboard values.
  Demo account scope now matches production project/model scope behavior.
- Batch 4: a reset-edge fixture first returned 360 instead of 120; exact partial
  hour handling now excludes pre-observation and post-reset requests. Account
  activity is not labeled pool usage; unsupported coverage/correlation returns
  null. Rust 96 library + 3 binary + 1 schema tests and Clippy pass.
- Additional unknown-only chart bucket regression: Web 19 unit tests pass;
  build and 7 browser tests pass. A synthetic-data browser visual inspection
  verified readable plot axes, local source legend and persistent values. Its
  temporary browser and dev server were closed. No installed app was replaced.
- Batch 5: additive sessionPage API and sessionSearch/sessionSort/offset/limit
  query fields remove the 30/500-root enumeration caps. Search/pagination does
  not change summary totals. Rust 97 library + 3 binary + 1 schema tests and
  Clippy pass; Web 19 unit tests, typecheck and build pass. New browser test is
  unverified: the current sandbox rejects binding the test server to localhost
  (EPERM). This is an environment restriction, not a passed GUI acceptance.
- Batch 6: child navigation is wired to the backend session query rather than
  merely filtering the tree. A parent/child/leaf/sibling fixture verifies child
  own=120, subtree=240, timeline=240 and exclusion of parent/sibling. Targeted
  Rust test and schema consistency pass; Web 19 unit tests and build pass.
  Browser acceptance and mock descendant support remain open.
- Batch 7: conversation trajectories now use the shared dated, keyboard-readable
  chart instead of 45px bars. Synthetic child fixtures and a unit conservation
  check pass. Web 20 unit tests and build pass. Network permission restored
  localhost serving, but test Chrome exits with SIGABRT before assertions; the
  automated browser suite remains unverified. In-app browser inspection
  confirmed child context and exposed/fixed a synthetic tree subtotal mismatch.
  Temporary preview and test service were closed; installed app unchanged.
- Batch 8: overview moves the main trend ahead of composition and diagnostic
  panels. Secondary composition and reconciliation are explicitly expandable.
  This is an initial layout improvement, not completed visual acceptance.
- Batch 9: natural calendar-year period added end-to-end to the API schema,
  selector and display labels. Targeted January boundary and leap-year
  comparison tests pass; Web typecheck/build pass. Custom dates and full
  annual interaction acceptance are still open.
- Batch 10: query validation rejects unsupported periods/grains/metrics/sorts,
  invalid timezone and oversized pagination/search parameters before storage.
  Targeted asynchronous regression verifies HTTP 400 and that storage is never
  called for invalid inputs; Clippy passes. Custom-range implementation remains.
- Batch 11: narrow layouts retain conversation usage, agent own usage and export
  by wrapping/reflowing. Added a 560px interaction regression; browser execution
  remains pending the test-browser launch restriction. Build validation is local.
- Batch 12: dedicated All conversations navigation is available in the sidebar
  and compact navigation picker, with the shared search/order/pagination controls.
  Account/model/time scope is preserved. Web build passes; interaction and
  complete workflow acceptance remain open.
- Batch 13: residual output is versioned as an unexplained difference, with
  isConservativeFloor=false. UI no longer displays inferred missing-account
  project allocation/composition; bilingual explanatory text rejects that
  interpretation. This changes diagnostic semantics, not stored usage facts.
- Batch 14: backend custom date windows include the final calendar day and
  reject missing/reversed dates. Cross-month boundary regression passes.
  Date-picker wiring and end-to-end custom-scope conservation remain open.
- Batch 15: bilingual custom-date picker wired through client query parameters;
  date filters survive navigation and reset conversation pagination. Demo scope
  filtering honors the requested dates. Web 20 unit tests and build pass;
  full custom-range browser and accounting acceptance remain required.
