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

## Approved account-first extension — 2026-09-07

User approved [ADR 0006](docs/adr/0006-account-first-usage-experience.md)
for active execution. This extends, rather than completes or resets, the work below.

- [ ] E01: Token-only M formatting across cards, tables, charts and native views;
      preserve exact raw values, unknown/zero distinction and non-token units.
      Browser cards, charts, sidebar, quota and request/turn detail now use M;
      exact integers remain available in detail titles/exports. Native rebuilt
      application acceptance remains pending.
- [ ] E02: bottom-left all/historical account selector; viewed scope is independent
      of observed login, with stale observations visible and no auth mutations.
      Browser navigation/selector implemented; native and real-account acceptance
      remain pending. Menu displays observed login independently and official
      observation timestamps without promising historical accounts are live.
- [ ] E03: shared overview/project/chat model breakdown, bucket detail and
      complete rankings; preserve own/tree and all applied filters.
      Shared paginated model components now replace the top-seven model view in
      overview/project pages. Project/account rankings expose all returned rows,
      maintaining full-scope shares and global rank numbers. Real-data/native
      acceptance and the accounting policy gate remain open.
- [ ] E04: unified calendar trends and peak-to-chat navigation, responsive bilingual
      presentation, working scroll/zoom/keyboard and retained last-good data.
- [ ] E05: historical account/pool/window quota cycles, same-deadline reset evidence,
      uncertain boundaries and no duplicated cross-window token attribution.
      Bounded observation-interval preview now retains older segments and same-
      deadline decreases. Schema 36 indexes all retained snapshots in resumable
      batches and indexes new appends atomically. Schema 37 now supports versioned
      interval boundaries and stable full-history backend/CLI pages. HTTP/UI
      history browsing is now wired through a generated read-only API contract.
      On-demand interval Token/model/project detail is wired with same-snapshot
      conservation and a temporary overlap rejection guard. Source-union repair,
      grant/cause verification and real-account acceptance remain unfinished.
- [ ] E06: reconcile source overlap and coverage with reviewed migration receipts;
      native acceptance must not be confused with source accuracy or release proof.

Execution: E01 foundation, E02 navigation, E03/E04 shared exploration, E05 cycle
history. Existing accounting P0 work remains a release gate throughout; no visual
change grants authority to overwrite historical facts or declare totals complete.

## Accounting invariants

Current critical accounting finding: `max_thread_day_v1` loses independent
requests when the two sources cover different portions of a day. The verified
A/B versus B/C fixture yields 500 instead of 600 and suppresses a 100-token
model entirely. **This defect is not yet fixed in the main aggregate policy.**
The next policy work must partition request/coverage evidence, shadow all
dimensions, then record a validated migration. Neither max nor an unqualified
sum is an acceptable proof of complete usage. See the
[counterexample and read-only diagnostics](docs/architecture/source-overlap-audit.md).

Priority after interval-detail integration: finish that source-union work rather
than treating temporary consumer guards as the accounting fix.

Parser promotion must also re-qualify persisted reconstruction checkpoints, not
just historical event rows. A checkpoint created by older replay rules may carry
an already-wrong live/prefix state; running a newer binary from that offset alone
does not prove the remaining stream is interpreted under the new policy.
The [resumable policy-upgrade path](docs/architecture/reconstruction-policy-upgrade.md)
now requalifies a valid legacy checkpoint in a separate cursor lane before its
next ingestion work, without rewriting historical quantities. Source-continuity,
historical projection review and real-data/native acceptance remain required
before any installed-app/main-selector promotion.

Schema 38 now stages the request-level union durably, with bounded resumable
backfill and change-triggered group recomputation; see the
[incremental candidate contract](docs/architecture/source-union-projection.md).
It passes the 600/100 counterexample without changing the main selector.
Next: establish legacy identity/coverage eligibility and scope-query semantics,
record a controlled real-shadow comparison, then promote all consumers together.
Staging readiness is not history completeness or release acceptance.

Schema 39 now requeues the candidate for coverage-policy revision 2: six
consumed amounts remain strict, while unequal cache-write observation weights
become unknown without discarding matching amounts. This applies only after
shared record identity and dimensions resolve. A bounded preview/sampling
comparison found a real compatible cohort with metadata-only differences but
missing shared identities; these remain ineligible for automatic union. See the
[comparison contract](docs/architecture/correction-preview.md).

Schema 40 now adds the candidate time-range index, and a read-only
[candidate query](docs/architecture/source-union-query.md) produces all five
dimensions and calendar grains in one snapshot. Pending/unresolved projection
states withhold candidate amounts; it remains a diagnostic reader rather than
the application's active accounting source. Legacy identity eligibility and
controlled real-shadow promotion are still prerequisites.

Full-source inspection now changes the next action: legacy cohorts may have no
shared/receipt keys at all, and parser corrections are not uniform across files.
One private root sample agrees at every old position while another has retained
rows suppressed specifically by the foreign-history guard. Validate ancestral
boundaries and produce per-record correction/identity eligibility before union
promotion; do not globally discount reconstruction or subtract a sample delta.
The [streamed comparison contract](docs/architecture/reconstruction-file-audit.md)
keeps EOF/position coverage distinct from inference and migration proof.

Boundary audit found and fixed another concrete overcount mechanism: random
UUIDv4 prefixes were accepted as UUIDv7 task clocks, ending foreign-history
protection early. The previous private suppression estimate is superseded by a
fresh full-source comparison under strict UUID validation. Ordered declared-
parent comparisons also distinguish raw equality from zero-cache-write field
expansion; a differing trailing rollback snapshot is not silently dropped.
See [identity and prefix evidence](docs/architecture/inherited-prefix-audit.md).

Per-record [sealed correction drafts](docs/architecture/reconstruction-correction-draft.md)
now preserve those proposed changes and verify expected old ledger values in a
single read snapshot. A private full-source draft passed both stream-integrity
and old-fact revalidation. Next build a reversible reviewed correction projection
from that evidence, then verify source-union scope/dimension parity. Draft seals
are not approval, source revalidation, or permission to delete historical facts.

An [isolated correction preview](docs/architecture/correction-preview.md) now
materializes both alternatives from a complete, old-fact-revalidated draft.
It supports exact filters and timezone-aware day/week/month/year distributions
without reopening source logs. A private preview conserves every dimension and
component, and an unaffected later calendar month stays identical across sides.
Next compare this candidate with the other evidence source and resolve identity
eligibility before promoting a unified selection policy; the live max policy is
still not repaired by the existence of a preview artifact.

Source-continuity finding (batch 128): physical device/inode strings differed
in a private existing-file cohort because the device component changed while
inode values matched. The old automatic replacement path could delete derived
history and restart backfill. It is now guarded: identity changes preserve
facts/cursors and require verification. Next work must establish reviewed safe
continuity/rebinding; matching inode alone is not sufficient. See the
[prefix audit and identity-review contract](docs/architecture/reconstruction-prefix-audit.md).

- Official account totals and local activity have explicit, independent scopes.
- Retained sampling amounts are derived from associated rollout usage in the
  current importer; agreement between these representations is not independent
  numeric calibration. See [sampling provenance](docs/architecture/sampling-value-provenance.md).
  Shared numeric normalization and canonical/replay boundary decisions are
  implemented for new sampling and reconstruction reads; shared strict parsing
  and broken-counter continuity now cover both primary adapters. Legacy partial
  evidence eligibility and source continuity remain open.
  Next accounting priority is legacy provenance enrichment and
  reviewed promotion. Raw snapshot positions alone cannot prove independent
  quantities when counters are re-emitted or history is inherited.
- Official reads now use an explicit observed-source binding, guarded before
  and after the RPC; see [account binding](docs/architecture/official-account-binding.md).
  This fixes a forward attribution risk, not historical account calibration.
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

Implementation contract: [durable request evidence](docs/architecture/durable-request-evidence.md).

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

Resolved for receipt-tracked ingestion in batch 93:
`copied_log_sources_must_not_duplicate_sampling` now returns 100 after copying
the source and is no longer ignored. Schema 33 assigns a counting owner per
receipt and preserves source aliases. Untracked legacy overlap, preexisting
duplicate groups and source-reset/failover continuity remain open audit work;
no historical duplicates were silently deleted.

Resolved in batch 72: `exact_window_usage_must_survive_raw_compaction` now
passes (120 before/after) and is no longer ignored. Exact boundary queries
prefer raw events, supplement only raw-absent retained events with current
assignments, and respect effective sampling/reconstruction source choice.
This does not recover missing old detail or validate the source-choice policy.

Current integrated state after batch 59: schema 26 retains local request facts,
preserves explicit sampling turns without changing dedup hashes, and exposes
typed request-evidence pages to an on-demand session table. Observed attribution
is not yet reconciled to filtered effective account/project ownership.
Synthetic three-page interaction is partly verified; full responsive/installed
acceptance is not. Core remaining priorities are source-overlap correctness,
scoped coverage, filtered request/turn analysis, complete backfill/retention
receipts, and installed cross-platform acceptance. No live ledger upgrade or
application replacement has occurred. Earlier checkpoints below are historical.

Latest checkpoint: schema 25 replaces full-table source projection refresh with
durable dirty date/thread keys. Synthetic new/upgrade, restart, unchanged-key,
key-move, deletion, source-choice and rollback/retry checks pass. No retained
token facts or source-choice policy changed. Full source-identity/coverage
arbitration, durable turns, performance measurement, GUI acceptance and installed
upgrade remain open. The paragraphs below preserve earlier implementation context.

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
- Batch 16: full custom-window fixture proves summary, time buckets, project
  grouping and conversation totals all include exactly the two in-range events,
  excluding events just outside each boundary. Dedicated model navigation now
  displays local model trends and breakdowns; Web 20 tests and build pass.
  Multi-model comparison and browser acceptance remain open.
- Batch 17: account page now leads with its scoped time trend and filters its
  account cards to the applied account selection. All-account mode retains the
  full list. Multi-account overlay comparison and usability acceptance remain.
- Batch 18: backend returns model/account time series from local facts, with
  custom-window conservation checked. Model page exposes multi-series comparison
  on the shared calendar axis; demo fixtures include both dimensions. Browser
  acceptance and account comparison presentation remain open.
- Batch 19: all-account view includes a shared-axis local account comparison,
  explicitly distinguished from official multi-device totals. Single-account
  view avoids the redundant comparison. Visual acceptance remains open.
- Batch 20: accumulated regression passes 102 Rust library + 3 binary + 1
  schema tests, Clippy, Web 20 unit tests and build. Chrome still aborts before
  page assertions. In-app browser verified All conversations search reduces
  the synthetic directory from 5 to 2 title/ID matches while the scope total
  remains unchanged. Fixed local-only page captions found during inspection.
- Batch 21: overview primary values, trend and rankings now all use local
  activity. Official totals remain in Accounts and explicit reconciliation.
  Local composition no longer pairs its buckets with an official total; account
  capture alerts are confined to the account view. Build verification follows.
- Batch 22: selected chart buckets can open the matching conversation list in
  overview/project pages while preserving account/model/project scope. Month
  and week bounds are retained; hourly points explicitly open the whole day,
  without pretending minute precision. Calendar bucket tests cover leap month
  and cross-month week; browser interaction acceptance remains open.
- Batch 23: backend node pagination/search replaces the hard 800-node cap;
  root own/tree usage and event counts are independent of the page. A 901-node
  fixture checks late-page totals and ID lookup. UI pagination wiring remains
  required; this backend change is not installed or marked complete.
- Batch 24: task-tree page/search controls are wired to node query parameters;
  opening another node resets its page. Request totals use backend full-scope
  counts instead of summing visible rows. Web build passes; demo pagination,
  interaction verification and removal of redundant page-local controls remain.
- Batch 25: server tree search is no longer narrowed by stale hidden local
  search/selection. Page-only ordering is labeled explicitly and pagination
  copy no longer claims an 800-node cap. Demo responses now expose node paging.
  Web 21 unit tests and build pass; large-tree browser acceptance remains open.
- Batch 26: demo regression verifies node pages and full-tree search retain
  root totals and request counts even when the root is not on the visible
  page. Web 22 unit tests pass; this does not substitute for browser acceptance.
- Batch 27: returning to a parent clears child-specific node search/offset so
  a later child page cannot make the parent appear empty. Account/model/date
  scope remains intact; typecheck verifies the navigation update.
- Batch 28: daily local series powers an activity calendar with year selection
  and date-to-conversation filtering. Unavailable/out-of-scope days stay distinct
  from recorded zero. Web build passes; calendar interaction/visual acceptance
  and complete coverage semantics remain open.
- Batch 29: custom-window conservation now also asserts the independent daily
  calendar series equals summary/project/conversation totals at the boundaries.
- Batch 30: schema 25 queues dirty date/thread keys in the rollup transaction;
  refresh touches only queued keys and atomically clears completed work.
  Upgrade initializes existing keys without modifying retained facts. Synthetic
  tests cover restart, unaffected-row guards, key moves, deletes, source ties,
  previous-schema upgrade and failed-refresh retry. Initial regressions exposed
  SQLite outer-UPSERT conflict propagation into trigger INSERT OR IGNORE;
  explicit existence predicates fix duplicate queue entries. Rust 106 library,
  3 binary and 1 schema tests pass, with Clippy and repository policy checks.
  This is a derived-index optimization, not proof of source accounting accuracy
  or a deployed migration. No live ledger or installed application was changed.
- Batch 31: custom date validation now rejects nonexistent civil-date boundaries.
  Midnight clock gaps resolve to the first actual instant of the day rather
  than falling back to now. A 23-hour day and a skipped-date timezone fixture
  pass alongside all 107 library, 3 binary and 1 schema tests; Clippy passes.
  This does not close broader DST chart-bucket or coverage acceptance.
- Batch 32: synthetic conversation fixtures no longer display a project's full
  timeline under a fractional conversation KPI. Integer-conserving fixture
  partitions align project/root/own/node/tree/timeline token dimensions and
  request totals; nested child subtotals include descendants, and child curves
  retain multiple dates. These allocations exist only in demo data, never real
  accounting. Eight token dimensions are checked across all fixture projects
  and roots. Web 23 unit tests and production build pass. Four requested browser
  regressions fail before assertions because Chrome launch exits with SIGABRT;
  interaction and installed-app acceptance remain unverified.
- Batch 33: dashboard response callbacks are guarded against canceled requests,
  including transports that resolve or reject after abort. Obsolete responses
  cannot replace the applied account/date snapshot, error message or loading
  state. Current failures retain the previous snapshot. Three lifecycle tests
  cover late success, late failure and current success/failure; all 26 Web tests
  and production build pass. Browser rendering acceptance remains open; the
  immediately preceding Chrome startup failure is not treated as a GUI pass.
- Batch 34: versioned scoped JSON replaces raw dashboard-bundle export. JSON
  and CSV share source/date/session-own-or-tree selection, omit unrelated
  catalogs and diagnostics, and retain unavailable components as null/blank.
  Session ID mismatches fail closed with no rows. Eight exporter tests include
  scope conservation, unavailable fields, catalog exclusion and stale detail.
  Web 28 tests and build pass. Downloads still carry selected scope identifiers;
  anonymous sharing and browser download acceptance remain open.
- Batch 35: malformed change-stream notifications no longer throw from the
  dashboard listener or advance its revision. Only nonempty string revisions
  and safe nonnegative integer revisions are accepted; unsupported payloads
  are ignored without clearing current data. Two parser tests cover malformed
  JSON, null/array/object/boolean values and unsafe numbers. Web 30 tests and
  production build pass; real stream reconnection acceptance remains open.
- Batch 36: local refresh/retry no longer waits for official account sync.
  Accounts has a separate bilingual official-sync action with its own busy
  and failure states. Official-sync feedback stays on Accounts, and error
  styling no longer depends on the language of the message. Web 30 tests and
  build pass; button interaction and narrow-layout acceptance remain open.
- Batch 37: in-app browser reached the synthetic Accounts page and the separate
  sync action returned a completion message. Inspection found the local-refresh
  accessibility name still claimed official sync; source now has dedicated
  bilingual local labels. Web 30 tests and build pass. A subsequent browser read
  still reported the old label, so runtime delivery/cache validation remains
  unresolved and is not claimed passed. Owned preview tabs and server closed.
- Batch 38: an explicit mock-mode production build on a distinct preview origin
  exposes the corrected local-refresh accessibility name in the in-app browser.
  This verifies the built label, not the entire responsive/interaction matrix.
  The prior stale development-preview observation remains unexplained. Future
  visual acceptance should identify the build and explicit data mode. Owned
  preview resources closed and the default HTTP-mode production build restored.
- Batch 39: retention audit confirms both chunk compaction and direct old-event
  ingestion discard per-request timestamps/components, retaining keys and
  aggregates. Sampling overloads file identity with an optional turn ID.
  Durable-request design now specifies both transactional write paths, explicit
  request-versus-turn semantics, historical detail gaps, attribution alignment,
  migration receipts and size/performance gates. Implementation remains open;
  no retention or live data changed.
- Batch 40: schema 26 retains compact request observations on raw and direct
  old-event ingestion. Chunk compaction preserves pre-upgrade raw observations
  before deletion and rejects mismatching retained dimensions. Writes share
  the source-cursor transaction; a synthetic evidence-write failure rolls back
  compact keys and cursor. Upgrade, replay and retained totals have regression
  coverage. This table is not an additional aggregate source. Turn IDs remain
  null and reconstruction/coverage/query/UI support is not complete. No live
  ledger was upgraded.
  Rust 109 library, 3 binary and 1 schema tests plus Clippy pass; an additional
  corrupted-retained-total assertion confirms compaction preserves raw records
  on mismatch. Documentation, module and privacy checks pass.
- Batch 41: typed retained-request store pagination uses thread/time bounds
  and a compound time/event-ID cursor, capped at 500 rows. A regression checks
  equal timestamps, final pages, excluded end boundaries and thread isolation.
  Missing turn IDs remain null. Targeted test and Clippy pass; HTTP/UI exposure,
  late-arrival snapshot consistency and effective attribution remain open.
- Batch 42: retained-request query rows preserve typed quality plus observed
  account/project confidence. Verified, inferred and unknown attribution remain
  distinct; a fixture checks missing account identity is not promoted to
  verified. Targeted pagination test and Clippy pass. No GUI/accounting source
  integration is implied by these observation fields.
- Batch 43: retained-request queries reject reversed/empty windows, absent
  thread IDs, invalid page sizes and malformed/out-of-window cursors instead
  of presenting invalid requests as empty usage. Cursors use canonical UTC
  timestamps to preserve lexical ordering. Targeted paging regression passes.
- Batch 44: full current Rust integration regression passes after request
  retention/query changes: 110 library, 3 binary and 1 schema tests. Clippy,
  module boundaries, generated-file and version consistency checks pass.
  Existing selected-session HTTP DTO still exposes aggregate timelines, not
  retained request pages. The next integration must explicitly distinguish
  ingest-observed account attribution from the active effective-account filter
  and must not label partial retained observations as complete session usage.
- Batch 45: dedicated request-evidence HTTP handler exposes thread-own retained
  observations with explicit incomplete history and ingest-observed attribution.
  Paired cursors and bounded pages are supported; unknown parameters and invalid
  query scopes are rejected. Handler regression checks token totals, null turns
  and invalid-cursor HTTP 400. Targeted test and Clippy pass. Frontend client,
  typed browser DTO and visible request-table integration remain open.
  Correction/evidence sequence: the first handler run failed because an empty
  in-memory SQLite path was reopened as a disk database requiring WAL. Filtering
  empty paths keeps the existing in-memory connection; the handler regression
  then passes. Initial success wording was premature, not a separate pass.
- Batch 46: request-evidence responses now use a Rust wire DTO with independent
  generated JSON Schema and TypeScript definitions. Both contracts are checked
  by the shared contract gate. Request/retention regressions, schema comparison,
  Clippy and Web build pass. Numeric assertions compare values after the shared
  wire token type serializes as JSON numbers. Client and request table remain
  the next incomplete integration layer.
- Batch 47: frontend request-evidence client uses generated types, encodes
  compound cursors, forwards abort signals and rejects HTTP errors or mismatched
  thread/time/attribution scope instead of returning empty usage. Web 32 tests
  and build pass; additional time-scope guard passes targeted client tests.
  The helper is not yet wired to a visible request table.
- Batch 48: session pages now include an on-demand retained-request table:
  UTC timestamp, model, non-cache/unresolved input, cache read, observed cache
  write, output and quality, with bounded scrolling, next/first pages and retry.
  Scope changes reset the panel; obsolete fetch callbacks are ignored. Bilingual
  copy states own-only/incomplete observations and no additive accounting.
  Account/model-filtered views and mock mode explicitly remain unsupported,
  rather than displaying unfiltered observations under filtered labels.
  Web 32 tests and build pass. Real table interaction, filtered attribution,
  turn grouping and responsive visual acceptance remain incomplete.
- Batch 49: request-table cursor history supports previous/next/first navigation
  without guessing offsets. Equal-time distinct IDs remain separate, and a
  failed page cannot advance using the previous page's stale next cursor.
  Pagination-state regression and all 33 Web tests/build pass. Browser table
  interaction and snapshot consistency across late arrivals remain open.
- Batch 50: sampling preserves explicit source turn IDs as supplemental
  provenance and retained-request membership. Legacy event hashes exclude this
  optional field, allowing matching replays to enrich membership without
  recounting. Conflicting memberships fail atomically; two requests in one turn
  remain two requests. Rust 112 library, 3 binary, 2 schema tests and Clippy
  pass. Historical unknown turns and reconstruction memberships are not inferred;
  turn-level query/grouping and visual acceptance remain incomplete.
- Batch 51: request table displays explicit source turn IDs and a bilingual
  missing-turn label. It does not infer a turn from the current page or bucket.
  Web 33 tests and build pass; complete turn aggregation and browser layout
  acceptance remain open.
- Batch 52: synthetic browser tests may no longer reuse an existing developer
  server. Dev ports are strict, mock mode has no live API proxy, and static
  production preview never inherits the live-ledger proxy. Isolation tests
  verify both mock and HTTP configurations. All 35 Web tests passed; a config
  typing failure exposed by importing Vite config into tests was corrected,
  then targeted isolation tests and production build pass. This prepares safe
  acceptance but does not itself validate the request-table UI.
- Batch 53: mock mode has an explicit 205-request fixture for the retained
  table, spanning three pages with repeated explicit turns and unknown turns.
  Bilingual copy separates this test fixture from actual usage and the demo
  aggregate chart. Its three-page identity/count/token conservation test passes;
  Web 36 tests and build pass. This enables isolated table interaction acceptance
  without proxying a live ledger; browser interaction remains to be executed.
  Build sequence: initial fixture literals widened enum fields to strings;
  an explicit row return type fixes the TypeScript error. The rebuild passes.
- Batch 54: isolated production mock preview was exercised in the in-app
  browser: All conversations to a root session, expand request evidence, advance
  through 100/100/5-request pages, verify final next disabled, then return to the
  second page with its expected timestamp bounds. The final return-to-first
  readback was inconclusive after accessibility indexes changed, so it remains
  unverified. This is interaction evidence, not full layout or live-account
  validation. Owned tab/server closed; default production build restored.
- Batch 55: request-table page count and first/last UTC timestamps are visible
  and screen-reader announced. A browser regression encodes three-page traversal,
  previous-page identity and exact return-to-first-row checks using scoped
  accessible controls. Web 36 unit tests/build pass. The new browser regression
  has not passed execution; prior manual evidence remains limited as recorded.
- Batch 56: request pagination now uses a direct compound cursor predicate,
  with a separate first-page lower bound. A 100,000-row same-timestamp fixture
  verifies the final 100 records and EXPLAIN confirms compound-index search,
  no table scan or temporary sorting. Initial millisecond fixture timestamps
  were rejected by canonical nanosecond validation; corrected fixture passes.
  Existing paging regression and Clippy pass. This is query-plan evidence,
  not a full retention-size or production-load benchmark.
- Batch 57: the 100,000-request fixture now populates realistic-length synthetic
  hash/account and model/project/turn fields. SQLite page growth was 39,022,592
  bytes; one warm in-memory 100-row deep read measured 1,449 microseconds.
  Measurements are printed by the regression and documented with exclusions
  (WAL, backup, cold disk, full ingest). No production budget is claimed.
- Batch 58: request client rejects negative/unsafe integers, broken input/output
  or cache-bucket conservation, out-of-range cache-write coverage and reasoning
  exceeding output. Invalid successful responses become explicit errors, not
  apparently precise table values. Web 37 tests/build pass. These checks validate
  received dimensions; they do not prove source completeness or dedup accuracy.
- Batch 59: full integrated Rust regression passes with 113 library, 3 binary
  and 2 schema tests. Clippy, both generated API contracts, privacy, document
  links, generated-file and module-boundary checks pass. This verifies current
  synthetic/source contracts, not the outstanding accounting or device gates.
- Batch 60: source-overlap audit confirms sampling IDs are log-row identities,
  reconstruction IDs are rollout identities, and temporal candidate matches
  discard shared rollout positions. Equal-distance candidate ties previously
  picked the first and confirmed it. New ingestion now records such ties as
  unknown with an ambiguity reason; a two-candidate regression preserves the
  previous confirmed total. Targeted ingest test and Clippy pass. No historical
  counters were reset/reprocessed; identity-based overlap remains outstanding.
- Batch 61: unknown request usage renders as unavailable, not numeric zero
  from the storage placeholder. Observed zero remains zero and unavailable
  cache-write detail remains a dash. Display regression and all 38 Web tests
  plus production build pass; no stored amounts are changed.
- Batch 62: direct historical-compaction replay now has explicit regression
  coverage for late turn enrichment: zero inserted events, unchanged token
  total, persisted turn membership, and conflicting membership rejection.
  The subsequent cursor assertion verifies failed transactions leave the
  cursor at the last successful replay. Targeted regression passes.
- Batch 63: each request row includes total tokens and separately labeled
  reasoning-within-output, preserving unavailable values for unknown evidence.
  Display assertions and all 38 Web tests/build pass. Narrow table layout
  with these added columns still requires visual acceptance.
- Batch 64: request table keeps 14px type, bounded internal horizontal scrolling,
  sticky column headings and a keyboard-focusable labeled scroll region. Browser
  regression now specifies 560px page-overflow and keyboard-scroll checks.
  Web 38 unit tests/build pass; new browser assertions are not executed proof.
- Batch 65: sampling retains the matched rollout byte position and physical
  identity as a candidate reconstruction link in schema 27. Supplemental
  provenance is excluded from legacy dedup hashes; links commit with evidence,
  and conflicting links fail rather than relabeling silently. Ambiguous matches
  have no link. Full pre-final-test Rust regression, targeted link/hash/upgrade
  tests and Clippy pass. Links are not equality proof: in-place rewrites can
  reuse positions, so shadow overlap still needs time/component verification.
  No historical source reprocessing or live database upgrade occurred.
- Batch 66: read-only candidate audit distinguishes missing links/targets,
  invalid time, conflicting fields and consistent candidates. It compares
  thread/model/quality, every token component and integer timestamp tolerance.
  Regression checks matching boundary, component/time mismatches and zero DB
  writes; targeted test and Clippy pass. It does not establish one-to-one
  identity or change source selection; aggregate shadow reporting remains open.
- Batch 67: candidate audit flags many-to-one source links as shared candidates
  rather than consistent independent matches. Schema 28 indexes the reverse
  lookup; previous-schema upgrade preserves existing links. Shared-target and
  migration tests plus Clippy pass. No historical accounting selection changed.
- Batch 68: exact boundary-hour audit reproduced a real gap in a synthetic
  ten-minute window: 120 before compaction, retained evidence still 120, but
  exact query returns 0 afterward. A deliberately ignored known-failing
  regression preserves the required assertion; its explicit run FAILED.
  This is unresolved correctness evidence, not successful validation, and blocks
  claiming complete durable exact-window accounting.
- Batch 69: retained detail lacked the machine identity required by historical
  account-epoch reassignment. Schema 29 preserves that origin on writes and
  before compaction; previous-schema raw records are covered by an upgrade test.
  Targeted retention/upgrade tests and Clippy pass. This is a prerequisite,
  not a fix claim for the still-failing exact-window regression. No live upgrade.
- Batch 70: schema 30 introduces a distinct current-assignment projection for
  retained requests. Raw ingestion and compaction populate it; supplemental
  turn/link replay cannot overwrite revised assignments. Previous-schema raw
  capture and replay-preservation tests plus Clippy pass. Account/project
  mutation hooks and source-aware exact queries remain pending, so the known
  boundary-compaction regression remains unresolved.
- Batch 71: account remap and catalog project rebind update retained current
  assignments transactionally without changing observed attribution or tokens.
  Historical epochs update machine-matching unassigned complete-hour records,
  preserving the existing unresolved switch-boundary policy. Compacted remap/
  reproject and historical-epoch regressions pass. Exact query integration is
  still the next step; no claim that the failing boundary test is fixed.
- Batch 72: exact boundary queries consume raw-absent retained evidence using
  current assignments and effective source selection. The formerly failing
  ten-minute compaction regression passes and is unignored. Additional tests
  check revised account/project filtering and reconstruction selection without
  adding the retained sampling copy. Full Rust 121 library, 3 binary, 2 schema
  tests and Clippy pass. No persisted historical token values were rewritten;
  missing pre-retention detail and source-policy correctness remain open.
- Batch 73: compaction boundary regression now compares the complete usage
  aggregate, request counts and account/project/model/thread time series in
  another timezone, not only total tokens. Targeted regression passes with
  identical pre/post-compaction dimensions. Broader coverage gaps remain open.
- Batch 74: retained request pages support current-account and model filtering
  before pagination. API/generated types explicitly separate selection attribution
  from observed row attribution, and the client verifies returned selection.
  The session table no longer disables filtered views. A remapped-account fixture
  selects the new account while preserving the old observed account; model
  exclusion is covered. Full Rust 126 tests, Clippy, both API contracts, Web 38
  tests and build pass. Filtered browser/live-account acceptance remains open.
- Batch 75: backend retained-turn pages aggregate the full filtered request
  set before paging; null turn IDs stay independent. A 120-request turn spanning
  request pages sums correctly, and two missing-ID requests do not merge.
  Confirmed counts accompany usage subtotals so unknown-only groups are not
  presented as measured zero. Targeted test and Clippy pass. HTTP/UI integration
  and complete-turn coverage remain open.
- Batch 76: typed turn-evidence HTTP endpoint exposes paged scoped groups with
  distinct group IDs, total/confirmed request counts and nullable confirmed
  usage for unknown-only groups. Handler null-usage regression and Clippy pass.
  Generated browser contract, frontend integration and complete-turn coverage
  remain open; no totals are rewritten or added.
- Batch 77: retained-turn JSON Schema/TypeScript contracts are generated and
  included in the API gate. Frontend client validates scope, selected account/
  model, time window, request counts and nullable confirmed usage. Web 39 tests,
  build and three-contract comparison pass. Visible turn UI remains pending.
- Batch 78: session pages have a collapsed on-demand retained-turn table with
  current account/model filtering, paging, confirmed/retained counts and nullable
  component subtotals. Synthetic turn rows derive from the same request fixture;
  a two-page test conserves all 205 requests and 24,600 tokens while keeping
  missing turn IDs separate. Web 39-test suite/build plus new fixture test and
  typecheck pass. Browser turn-table and real-account acceptance remain open.
- Batch 79: in-app browser exercised the synthetic turn panel and next page,
  verifying explicit/missing turns and disabled next on the final page. Screenshot
  inspection exposed native unstyled toggle/pager controls; shared evidence
  controls now use consistent typography, spacing, surfaces and focus styling.
  Web 40 tests/build pass. Updated styling itself still needs visual readback;
  full narrow/zoom/live-data acceptance remains open. Owned preview resources closed.
- Batch 80: read-only installed base-file metadata confirms the published
  schema-24 upgrade starting point. Private size metadata remains outside the
  public repository. Writer-state inspection was denied, so no consistent live
  snapshot or live migration is claimed. Upgrade acceptance requires verified
  writer state, consistent backup, free-space headroom and isolated migration
  comparison. The installed application remains unchanged.
- Batch 81: genuine schema-24 synthetic database is built by executing only
  migrations 1–24, then seeded with a synthetic raw event and upgraded normally.
  All raw/rollup token dimensions and counts remain equal; no request detail is
  invented by migration. Test and Clippy pass. This validates the schema chain,
  not the real database, backup consistency or installed application.
- Batch 82: schema 31 adds durable bounded request-evidence backfill. Startup
  does one <=1,000-row chunk; daemon/serve ticks continue it. Completed targets
  skip without writes, and source cursor/evidence changes are transactional.
  Restart/resume fixture preserves all request facts and rollup totals; targeted
  test and Clippy pass. Live upgrade/backfill performance and UI progress
  communication still require acceptance.
- Batch 83: request/turn responses expose actual backfill completion independently
  of history completeness. Panels show a pending-only bilingual notice and
  reload on refreshed bundles even with fixed custom dates, without resetting
  pagination/open state. Backfill regression, three contracts, Web 40 tests and
  final build pass. Live progress transitions and GUI acceptance remain open.
- Batch 84: native build script produced a workspace-only arm64 macOS bundle.
  Rust release, locked Web dependency/build step and 11 Swift source compilation
  succeeded. Independent final-bundle deep signature, both executable architectures,
  executable permissions, Info.plist version and every file checksum verified.
  Bundle manifest SHA-256: 1223f45c5a75b993c24832d62e1f61806a59e5de88fc2221aa0fc242cfd66605.
  This is ad-hoc signing, not notarization or publication. The bundle was NOT
  launched or installed because default data paths still target the live ledger;
  real migration, lifecycle and device acceptance remain open.
- Batch 85: native packaging explicitly forces same-origin HTTP data mode,
  overriding inherited demo/external API environment values. A rebuild with
  deliberately conflicting synthetic values produced the identical complete
  bundle manifest hash recorded in batch 84. Final signature and checksums
  verified again. This protects build configuration; no application was
  launched/installed and no live ledger was touched.
- Batch 86: a forced second-record backfill failure verifies atomic rollback
  of the first retained row, origins, assignments and cursor. Removing the
  synthetic fault then resumes normally across restart with conserved totals.
  Targeted regression passes; this is not a live migration execution.
- Batch 87: turn client rejects nonadvancing/unsafe offsets, empty continuing
  pages and duplicate/missing group IDs so invalid responses cannot cause
  misleading repeated pagination. Web 40 tests and build pass. Previously built
  native artifact predates this frontend change and must be rebuilt for release.
- Batch 88: request/turn query execution now normalizes account/model selections
  consistently with returned metadata. Blank/all/padded values no longer
  execute a different scope than they advertise. Both endpoint regressions pass.
- Batch 89: frontend HTTP clients and synthetic fixtures share the same trimmed
  selection normalization. Encoding and returned-scope checks now agree with
  the backend for blank/all/padded inputs. Web 41-test suite/build, final
  typecheck and targeted client/fixture regressions pass.
- Batch 90: removed the invalid inference that a first observation implies
  complete local coverage (including empty ledgers). Local period coverage
  ratios/completeness are unknown; local metric ratio is nullable, and its
  completeness is not claimed. Earliest retained day is scoped to active
  account/project/model. Official coverage remains separate. New scoped/empty
  tests, full Rust 132 tests, Clippy, three contracts, Web 41 tests/build pass.
  Continuous interval coverage remains an open requirement, not solved by this
  correction. Initial date-prefix assertion was fixed to compare actual UTC time.
- Batch 91: minimal copied-source fixture demonstrates duplicate counting:
  one 100-token observation becomes 200 when root and migrated log databases
  overlap. Explicit regression run FAILED and is preserved as a labeled known
  failure. No production source or persisted usage was changed. This is a
  source identity defect, not evidence for any particular real-world total.
- Batch 92: sampling captures path-independent receipt digests when source
  process identity exists; exact row/time/thread/body fields distinguish requests.
  Schema 32 retains receipt associations without changing legacy hashes or
  existing amounts, and no receipts are invented for older records. Identity,
  hash-preservation and upgrade tests plus Clippy pass. Consolidation is not yet
  applied; the copied-source double-count regression remains unresolved.
- Batch 93: tracked receipt owners prevent copied log sources from adding a
  second usage row. The previously failing 100→200 regression now stays 100 and
  is unignored. Distinct requests remain counted; unknown raw owners can be
  resolved once; weaker copies, conflicting dimensions and compacted replay
  are handled without recounting. Legacy duplicate groups are not auto-selected.
  Full Rust 138 tests passed before final strengthened replay assertions;
  targeted receipt/replay/upgrade checks and Clippy pass afterward. No live
  migration or retrospective duplicate cleanup occurred.
- Batch 94: copied-source regression now appends a genuinely new request only
  to the migrated source. The duplicate stays excluded, the new 150 tokens are
  counted once (total 250), and a subsequent idle pass reads no new evidence.
  Targeted regression passes; source removal/reset continuity is still open.
- Batch 95: primary removal with unequal source high-water marks reproduced
  450 instead of expected 630. Existing namespaced sources now retain their
  own cursor regardless of list position; the regression is unignored and
  passes, alongside copied-source dedup and Clippy. Legacy first-source binding
  ambiguity and physical source replacement remain open, not implicitly fixed.
- Batch 96: newly committed cursors preserve their actual relative path binding.
  A migrated-first fixture then introduces a primary source with independent
  requests; the original binding stays put and the new source gets its own
  namespace. Both source-appearance/removal regressions and Clippy pass.
  Old cursors without binding metadata and physical replacement still need
  continuity audit; no legacy history is claimed repaired.
- Batch 97: new cursor metadata tracks physical identity and source generations.
  Detected replacement uses a new namespace and row-zero read; stable receipts
  suppress copied rows while genuine reset IDs remain distinct, including a
  reused turn ID. Replacement without receipt identity preserves the old cursor
  and fails explicitly. All sampling tests and Clippy pass. Same-inode resets,
  legacy identity adoption and live lifecycle handling remain open.
- Batch 98: the prior goal turn was concrete progress (physical-generation
  handling). A same-file row-ID reset then reproduced a missed request: 250
  instead of 430 synthetic tokens. Cursor metadata v4 now stores each committed
  batch's last-row digest. One read-only source snapshot checks that anchor and
  reads appended rows; a changed/missing anchor uses the receipt-guarded new
  generation path. The regression now passes, including an empty intermediate
  source, reused turn ID, idle replay and missing-identity rollback. A pruned
  anchor fixture also preserves existing receipts while adding only the new
  request. Full Rust 143 tests and Clippy pass; privacy, doc links, generated
  files, module boundaries and version checks pass. No schema bump, real source
  mutation, installed-ledger upgrade or application installation occurred.
  Legacy unanchored cursors, rewrites that preserve the anchor, source failure
  lifecycle, shadow accounting and complete GUI/native acceptance remain open.
- Batch 99: the previous turn was progress (same-file continuity regression and
  fix). Daemon sampling/quota errors no longer propagate directly out of the
  collection loop. Each sampling/quota/reconstruction step is attempted;
  failures and per-pass partial issues publish `degraded` with stable source
  codes. Subsequent successful idle collection clears the state, and identical
  status avoids repeated writes. Tests cover unavailable-source retry, retained
  counters/facts, no raw error exposure and actual empty-source recovery.
  Rust 145 tests, Clippy, Web 42 tests/build and contract/governance checks pass.
  A running isolated empty-ledger daemon remained accessible over multiple
  retry ticks; in-app browser AX and screenshot confirmed the failure notice.
  Initial screenshot exposed clipped 10px copy; final rebuilt 14px wrapping
  notice was visually read back. Test browser and daemon were closed cleanly.
  This is source/debug-daemon evidence, not installed-app or full responsive
  acceptance. Closed-enum compatibility requires a paired backend/Web release
  (ADR 0001). No real ledger/source or installed bundle was modified.
  Follow-up findings: empty-ledger request match still shows 100%, and absent
  collection displays zero-valued metric cards; these need explicit empty versus
  recorded-zero semantics. Initial auth/catalog failures, HTTP task supervision,
  repeated warning-log volume, full viewport/bilingual interaction matrix and
  shadow/live migration acceptance remain open.
- Batch 100: the previous turn made source-lifecycle progress and exposed an
  empty-ledger 100% match rate in the actual browser. A synthetic regression
  first reproduced that incorrect value. Summary match/cache rates and local
  daily average now return null when their denominator/sample is absent;
  the resolved local total is unknown/null without confirmed events. Recorded
  zero-token events remain numeric zero. Quality states use the same selected
  window aggregation path. Overview/recent token cards and composition avoid
  presenting an empty sum as measured zero; recorded request counts stay counts.
  Mock summary ratios now use actual fixture evidence counts rather than an
  invented extra denominator. ADR 0002 records the paired-release compatibility
  change. Full Rust 146 tests, Clippy, Web 43 tests/build and contracts/governance
  pass. An initial incorrect request-metric literal was caught by typecheck and
  corrected before the passing Web run. Isolated HTTP-daemon browser acceptance
  confirmed Chinese/English dashes and no false 100%; English metric notes were
  then changed from ellipsis to wrapping and visually read back in the rebuilt
  page. Browser and daemon were stopped; no real ledger or installed app changed.
  Remaining: propagate/verify no-evidence semantics across all detail/export
  surfaces, fix crowded English filter layout, full responsive/zoom acceptance,
  continuous coverage, legacy/source shadow accounting and safe live migration.
- Batch 101 (interrupted by user-requested storage cleanup): filter layout now
  removes fixed narrow caption columns and conflicting viewport overrides in
  favor of panel-width wrapping. Refresh keeps stable visible text with an
  accessible busy state. Web 44 unit tests and explicit mock build passed;
  new five-width/three-zoom English non-overlap browser tests are written but
  NOT executed. No visual acceptance or commit of this layout batch is claimed.
  The user then requested project garbage cleanup; preview was stopped and its
  mock Web output removed. Approximately 1.45 GiB of regenerable target,
  node_modules and app bundles inside three obsolete isolated audit copies
  were deleted, plus small generated Web/test outputs. Audit sources, current
  target/node_modules, all pending source edits, real ledger and installed app
  were preserved. No matching open files were reported by the available lsof
  check; process enumeration itself was sandbox-denied. Available disk space
  read 9.4 GiB afterward (not all free-space change is attributed to cleanup).
  Resume with the uncommitted filter layout, browser acceptance and production
  Web rebuild as needed; do not recreate deleted obsolete audit environments.
- Batch 102: resumed the pending filter layout after completed storage cleanup.
  Scope captions now occupy their own wrapping row; local selectors use one
  adaptive grid, and panel-width container rules replace narrow fixed caption
  columns/page-specific selector counts. Period buttons use a balanced grid at
  medium/narrow widths; visible refresh text stays stable while aria-busy and
  the accessible label expose activity. Web 44 tests and production build pass.
  Shell-launched Chrome again exited with SIGABRT before all five Playwright
  layout tests could execute: those tests remain NOT PASSED, not product proof.
  Added a development-only synthetic iframe harness excluded from the production
  bundle. In-app browser measured 30 cases (two locales, five widths, three CSS
  zoom levels), with 16 controls per case and no overlapping/outside controls.
  Separate real-viewport checks at 560/700/900/1280/1440 had no document overflow
  or control overlap, and project navigation produced no captured console error.
  Final period-grid refinement required another verification: the first attempt
  ran before component readiness; a later inspection revealed stale dev-server
  CSS. Added readiness waiting, restarted the isolated server, inspected loaded
  CSS, and reran all 30 cases successfully on the final rules. Final 560px English
  screenshot shows readable two-row periods. This is not native WKWebView zoom
  acceptance or full workflow approval: extreme narrow/160% layouts still use
  substantial vertical space. Test tabs/server closed and viewport reset.
  Production build excludes the harness; no real ledger, installed app or old
  audit environments changed. Full goal remains ACTIVE.
- Batch 103: browser workflow acceptance found a real root-scroll escape:
  opening retained evidence and paging at 560px moved the outer document by
  369px and hid the top navigation, despite a working inner workspace scroller.
  HTML/body/root now constrain the viewport, body is fixed to its bounds, and
  obsolete narrow-screen body-overflow overrides were removed. The first clip-
  only attempt did not fix it because those overrides still created a hidden
  but programmatically scrollable body; actual ancestor offsets exposed this.
  The workspace is now an explicitly labeled, keyboard-focusable region with a
  visible focus outline. Final in-app browser checks at five widths showed
  window/body offsets zero, header top zero and workspace bottom gap <= 0.5px.
  Retained-turn paging reached its 40-row last page, and End on its independent
  table scroller exposed the final row inside the viewport. New Playwright
  regression records the workflow but remains unexecuted under the existing
  Chrome launch restriction; manual in-app evidence is separate. Web 44 tests
  and final production build pass. Browser viewport reset, tabs and isolated
  dev server closed; no live ledger or installed application changed. Native
  zoom, full language/large-tree acceptance and the remaining accounting goal
  are still open, not certified by this scrolling fix.
- Batch 104: the previous turn made verified scrolling progress. Added an
  explicit UUID-only native isolated profile so future native acceptance can
  use temporary ledger/source paths and independent UI preferences. Normal
  launch paths remain unchanged; invalid/duplicate/misspelled profile flags and
  linked profile boundaries fail closed. Both serve/daemon arguments explicitly
  select the temporary Codex home and discard conflicting inherited path
  overrides. Toolbar marks the isolated context; bundled resource and fixed-port
  policies remain unchanged. Swift tests cover these branches and permissions;
  a local name-shadowing compile error was fixed before the passing tests.
  Standard macOS build, arm64 metadata/resources, nested/deep ad-hoc signatures
  and production Web build pass. Final bundle manifest SHA-256:
  `9acaacda034a603a630cbceebc3b47557ed74dec726740544bee8b93b443189a`.
  A direct native executable launch returned exit code 1 with no diagnostic;
  no isolated data directory or port listener was observed afterward. Native
  window/zoom/lifecycle acceptance therefore remains NOT ACHIEVED. This is not
  an installed-app update, notarized release or live-ledger migration. ADR 0003
  and the isolated-preview architecture document record the boundary. The pure
  test runner now removes its own temporary executable directory on exit.
- Batch 105: previous turn advanced native isolation but did not achieve native
  window acceptance. Returned to source-overlap correctness with a bounded
  `audit-overlap` CLI: explicit existing DB/thread/UTC window, exact account/model
  selection and paired keyset continuation. Read-only opening refuses missing
  files and unsupported schema versions without creating/migrating a ledger.
  One read snapshot per page groups retained observations into candidate states;
  shared candidates are checked beyond page boundaries. Confirmed token groups
  are explicitly page-local, unknown-only amounts stay null, and no corrected
  union/equality/completeness claim is produced. Existing source selection and
  usage are untouched. An initial read-only test exposed that ordinary aggregate
  queries refresh projections; the audit path therefore uses only retained
  evidence/link reads. Mixed-state pagination, no-write checks and actual CLI
  byte-for-byte database preservation pass. Full Rust 149 tests, Clippy and all
  three API contracts pass. No real ledger/source import or native installation
  occurred. Global overlap coverage, counterpart-level explanation, replacing
  max-per-thread/day source selection and historical repair receipts remain open.
- Batch 106: previous turn added a read-only audit command. Audit format v2 now
  includes counterpart ID/time/thread/model/token evidence, validity, global
  link count and signed timestamp delta, plus field-level differences across
  all token components and dimensions. Source models are explicit; dangling
  links keep their ID without invented counterpart usage. Shared-candidate
  classification remains distinct from numerical consistency, and unknown
  source placeholders are not compared as measured token amounts. Single-row
  and paged status logic now share one comparison implementation. Tests cover
  component changes, >250ms timing, malformed timestamps, shared cross-page
  links, nulls and CLI format v2. A synthetic invalid-total DB write was correctly
  rejected by existing triggers; invalid-counterpart testing was moved into the
  pure comparison function instead of weakening storage guards. Final full
  Rust 151 tests, Clippy, API contracts and governance checks pass. No persisted
  totals, source selection, real ledger or installed app were modified. Whole-
  history overlap measurement and validated replacement accounting remain open.
- Batch 107: previous turn improved counterpart explanations. A constructed
  partial-overlap scenario now proves loss in the existing aggregate policy:
  retained A/B=300, reconstructed B/C=500, known independent A/B/C=600; the
  application returns 500 and zero for A's 100-token model. The test explicitly
  records this defect, not correctness of the current max rule. Audit format v3
  adds full-storage-day policy context and a per-row retained-side-selection
  flag, computed read-only from rollups rather than refreshing cached choices.
  A one-second/model-scoped audit still exposes the full-day decision while
  keeping its own page subtotal at 100, preventing scope conflation. Unknown
  observations have no selection flag. Full Rust 152 tests, Clippy, contracts
  and governance checks pass; this validates the diagnosis, not a fixed main
  policy. No true usage or source selector was rewritten. The critical finding
  is now near the start of this goal; request/coverage partitioning, cross-scope
  shadow validation and a migration receipt are required before replacing max.
- Batch 108: prior turn verified the day-max counterexample. A bounded read-only
  local schema/key-presence check did not find request/response identity markers
  in the sampled post-sampling rows; this is not an all-log absence claim. Both
  adapters now capture a shared local source-record fingerprint (machine/file/
  thread/offset plus parsed-content digest). Schema 34 stores it separately by
  evidence side without changing old event IDs or hashes or inventing old keys.
  Genuine schema-33 upgrade, metadata enrichment, conflict preservation and
  cross-adapter equality tests pass. Test debugging exposed ordinary replay
  reinsertion after compaction; compacted-key checks now prevent new recounts.
  The adapter fixture was moved inside allowed Codex roots and its timestamp
  aligned with its log event; source boundaries/tolerance were not relaxed.
  Legacy backdated migration fixtures use the existing idempotent DDL convention;
  the new migration also has a genuine predecessor test. Final full Rust 155
  tests and Clippy pass. ADR 0004 records that this is local measurement evidence,
  not server equality or authorization to combine replayed history. No real
  ledger migration, main-policy replacement or installed-app update occurred.
- Batch 109: resumed the uncommitted shadow planner after the environment/date
  changed; prior source-record work was concrete progress. Added a pure local
  measurement union planner and read-only `shadow-union` command. Counterpart
  closure is loaded before time/dimension selection, with an explicit returned-
  observation cap (not yet a bounded-I/O guarantee). Shared keys collapse only
  with matching valid usage, dimensions, assignments and nearby times; missing,
  ambiguous or conflicting groups prevent a complete supplied-record total.
  Canonical sampling time is filtered after pairing. The existing A/B vs B/C
  fixture now yields 600 in shadow and restores the 100-token model, while the
  unchanged production policy still yields 500/0. Tests cover every component,
  ambiguity reasons, unknown/zero/overflow, duplicate IDs, cross-boundary account
  conflict closure, cap errors and real CLI byte-for-byte DB preservation.
  Full Rust 160 tests and Clippy pass. This is local measurement union, not proof
  against replay or of server request identity. History completeness and
  production-policy-changed flags remain false. No real ledger migration or
  installed app update occurred; performance, coverage/replay validation,
  cross-dimension reconciliation and a migration receipt remain required.
- Batch 110: previous turn implemented the shadow union. Replaced its whole-
  observations CTE with bounded per-side indexed thread/time seed reads, one
  indexed expansion per distinct record key, and primary-key payload lookups
  within the same read snapshot. No production/schema changes. A 100,000-row-
  per-side unrelated-history SQL fixture verifies zero full-scan steps, <100
  VM steps per one-row seed lookup and <150 for a two-record closure. These are
  index-behavior checks, not real-ledger latency guarantees. Full-store fanout
  tests preserve cross-thread/time conflicts and reject insufficient caps;
  initial fixture identities collided with an existing provenance uniqueness
  guard, so synthetic copy namespaces were corrected rather than weakening it.
  Full Rust 162 tests, Clippy, API contracts and governance checks pass. Stale
  links may require extra key-local checks; this remains diagnostic rather than
  a proven incremental production projection. No actual ledger migration or
  installed application update occurred. Remaining replay/coverage validation,
  main-policy replacement and full product/native acceptance keep goal ACTIVE.
- Batch 111: prior turn improved indexed shadow lookup. Synthetic RED tests
  exposed timestamp-watermark holes: SQL maturity filtering returned IDs 1/3
  while ID 2 was pending, and second-only comparison admitted a record one
  nanosecond too new. Reader now stops at the first pending ID using exact time.
  Invalid seconds/nanoseconds fail rather than clamp or become current time;
  valid rows preceding an invalid row do not advance the persisted checkpoint.
  A further reversed-time fixture matched only one of two mature observations;
  matching now sorts by timestamp while commits remain log-ID ordered, and
  report timestamps use extrema. All reproductions now pass, including total
  and cursor preservation. Full Rust 166 tests, Clippy and contracts pass.
  These are new-ingestion guards, not recovery of historical omissions. Far-
  future source clocks can defer later rows and need operational diagnostics;
  no live ledger, installed app or main day-max policy was changed. Remaining
  source coverage/replay, shadow-to-production migration and GUI/native goals
  stay ACTIVE.
- Batch 112: previous turn fixed incremental time-watermark defects. Began a
  real-data migration acceptance on an isolated SQLite backup, never the live
  ledger. The copy passed quick_check and upgraded from schema 24 to 34. Nine
  allowlisted usage fact/rollup tables preserved ordered row fingerprints and
  counts exactly, also after normal startup's first bounded request backfill.
  Original schema remained 24; no app install or source import occurred. Private
  before/after/startup receipts and remaining-copy location are recorded outside
  the public repo. Added a streaming read-only fact auditor with a synthetic
  regression for missing-file refusal, opaque hashes, no writes and changed-row
  detection. It requires a quiescent snapshot, not a live DB. WAL readback errors
  were resolved only on the idle copy via checkpoint/DELETE journal mode; no
  immutable shortcut was used. Full backfill/restart/native acceptance and
  accounting-policy correctness remain open; preserved hashes are not proof of
  correct historic usage. Retain one private copy for the next acceptance step.
- Batch 113: resumed the existing private migration copy, not a new backup.
  Added bounded `backfill-requests` maintenance using existing chunk logic and
  explicit current-schema DB selection, with missing/old-file rejection. A
  synthetic subprocess test covers pending-target completion, zero-work restart
  and batch bounds; its initial empty-schema assumption was corrected by
  explicitly constructing a pending empty target. On the private copy, bounded
  runs completed all existing raw-request detail and a fresh process attempted
  zero additional batches. Row-level raw/retained usage, quality and observation
  dimension comparison found no differences or missing assignment/origin rows.
  All nine pre-migration fact-table hashes/counts remain identical; quick_check
  passed and the original stayed schema 24. Full Rust 167 tests and Clippy pass.
  Private receipts updated outside the repo. This is existing-detail migration
  and process-restart evidence, not recovered deleted history, new source
  identity, main-policy correctness or installed/native acceptance. One isolated
  copy remains for data-query/native validation; full goal remains ACTIVE.
- Batch 114: previous turn completed isolated detail backfill/restart. Live
  process inspection showed the installed native app/service running; requested
  user approval before temporarily closing it, and did not interrupt it. Review
  exposed that earlier claimed health ownership was only service-name checking,
  and WebView was eagerly created before readiness. Added serving PID to health,
  required matching live child PID plus rechecked generation/child/mode after
  await, and gated WebView creation on verified readiness. ADR 0005 documents
  accidental cross-instance protection, not hostile-process authentication.
  Swift pure tests and standard Mac bundle build/deep ad-hoc signature passed;
  bundle manifest `d56c6a9ca4d6d707b65c31786d23fb47a3bf61e8d632b2f009996d4051323a8c`.
  Rust 168 tests passed; Clippy caught test-module placement, corrected and
  rechecked along with the health regression. Contracts/governance checks pass.
  No new native instance launched against the occupied port, no installed
  replacement or real-ledger write. Actual native startup/port-conflict/zoom/
  exit acceptance still pending, alongside main accounting policy and coverage.
- Batch 115: previous turn fixed native health/child binding. Mode-switch and
  health-timeout shutdown previously sent SIGTERM without an escalation deadline.
  Added an 800ms owned-child deadline, cancelled on termination/launch/app exit,
  with generation/ownership checks before SIGKILL. Signal failure cancels pending
  mode and remains a visible failure. Repeated mode requests while stopping now
  retarget the pending destination without extending the deadline. Removed the
  fixed historical-size claim from the bilingual collection prompt. Real helper
  subprocess tests cover graceful exit, explicit SIGTERM ignore, cancellation,
  unowned and later-revoked ownership; all children are test-owned. The first
  stopped-process fixture did not isolate SIGTERM behavior and failed; replaced
  it with an explicit signal-ignoring child and cleanup-on-error assertions.
  Final helper/state tests, native build, deep signing and governance checks pass.
  Bundle manifest: `0c928bcb40b8260633cff01ec9c34f0d04cd0d111382756a9349cbfed7330cf4`.
  No installed app interruption/replacement or real-ledger change occurred.
  This does not prove full UI mode-switch/zoom/export acceptance or guarantee
  exit time for kernel-stuck processes. Pending user approval for a temporary
  native test window is separate from remaining useful accounting work; goal
  remains ACTIVE.
- Batch 116: prior turn hardened native shutdown. Real-data query acceptance
  found the previous system-temp migration copy absent. A prematurely parallel
  service launch created an empty DB there; it was immediately stopped and
  excluded from results. Created a fresh SQLite snapshot under a durable private
  workspace directory, then upgraded/backfilled it with explicit empty Codex
  home before serving. Added GET-only bundle-scope checks; all seven presets and
  two selections each for account/project/model agreed across summary, curve
  and breakdown token components (13 queries). Rolling7 was notably slower and
  remains a performance follow-up. Synthetic test detects deliberate drift and
  verifies no private dimension IDs are output. Private timing receipt/snapshot
  location recorded outside the repo; isolated server stopped. This proves only
  tested API consistency, not source accuracy, all filters or native/browser
  usability. No production policy change or installed-app update; goal ACTIVE.
- Batch 117: prior turn measured real-copy rolling7 latency. Its retained time
  predicate performed an index scan rather than a bounded search. Schema 35
  adds a global effective-time index; precise union projects a single time
  column to enable predicate pushdown. Genuine predecessor upgrade and exact
  usage/index-plan regressions pass. Initial faster real-copy queries exposed
  a scope mismatch: bundle components independently resolved current time.
  Added an internal shared reference instant (not client-deserializable) and
  deterministic boundary conservation test. The cross-crate query struct needed
  a hidden public Rust field for existing CLI construction; HTTP remains skipped.
  Final 13-scope audit passed, and private before/after row fingerprints match
  for all nine fact tables. Final rolling7 samples improved from roughly 3.7–4.1s
  to 2.1–2.2s; this is observational, not a controlled latency guarantee. Full
  Rust 170 tests and Clippy pass. Server stopped; updated private receipts stay
  outside repo. Main day-max accuracy, interval coverage and native acceptance
  remain open; no original-ledger or installed-app upgrade occurred.
- Batch 118: previous turn aligned bundle clocks and indexed exact windows.
  A new boundary regression showed quality-page confirmed count 2 while the
  same rolling scope contained only 1; that page still used day rollups. All
  quality categories now use the shared selected-period aggregator, preserving
  unknown token semantics. Explorer recent/project/session activity uses the
  anchored bundle clock, and recent windows no longer include a future second.
  Added historical-reference/future-boundary tests and expanded the HTTP audit
  to quality confirmed count/components, with a deliberate quality-drift negative
  test. Full Rust 172 tests, Clippy and synthetic audit pass; 13 private-copy HTTP
  scopes also passed with the expanded checks. Unused old aggregate import was
  removed. Isolated server stopped; no installed app, original ledger, schema
  or main day-max source policy changed. Remaining source correctness, coverage
  and native acceptance keep the goal ACTIVE.
- Batch 119: previous turn aligned rolling quality and recent activity scopes.
  Bundle assembly now uses one SQLite read snapshot after refreshing the
  derived selector. A dirty-selector race retries preparation up to three times
  rather than doing nested writes inside the read snapshot. Conversation count/
  page queries reuse the outer snapshot, preserving standalone transaction use.
  A second WAL connection committed usage and registry changes between reads;
  the active snapshot retained old values, the next saw new values, and error
  paths returned to autocommit. Full Rust 173 tests and Clippy pass. All 13
  private-copy HTTP scope audits passed (rolling7 approximately 2.3s). This is
  per-response consistency, not cross-request pagination pinning or source
  accuracy. No original-ledger or installed app modification; production union,
  coverage/replay verification and native acceptance remain outstanding.
- Batch 120: resumed after user-requested cleanup of stale own-crate codegen
  objects (about 4.75 GiB reclaimed; current incremental/dependency caches,
  pending source edits and private acceptance ledger retained). Rolling7
  conversation ranking had used whole-day totals: a boundary-excluded root
  ranked ahead of an included child. Detail independently returned 480 instead
  of 240 synthetic tokens by including an outside and a future event. Explorer
  now shares one exact thread/time projection across pre-pagination root
  ranking, displayed own/tree totals, detail nodes and own/descendant curves.
  A timezone regression also exposed mixed Shanghai/interior and UTC/boundary
  labels. Non-Shanghai rolling series use exact timestamp evidence, including
  fractional offsets; this is not acceptance of every calendar timezone path.
  Synthetic tests cover ordering/pagination/model-project filtering, child
  detail, four curve grains, all token components and three non-Shanghai zones.
  Initial RED failures were observed; final Rust 176 tests, Clippy, API contract
  and governance checks pass. Private-copy HTTP audit passed 13 scopes; two
  selected roots across four grains also matched list/detail, full visible node
  own sums and both timelines component-by-component (8 checks). Rolling7 bundle
  measured about 3.2s during concurrent audit traffic; this is not a controlled
  latency comparison. Isolated server stopped. No DTO/schema, source policy,
  original ledger or installed-app update. Main day-max undercount/overlap,
  continuous coverage and native acceptance remain open; goal stays ACTIVE.
- Batch 121: prior turn corrected rolling explorer scopes. Returned to the
  critical source-overlap prerequisite: shadow-union report v2 now projects
  the resolved canonical measurement set into UTC-day/account/model/project/
  thread buckets with checked counts and all token components. Unknown keys
  stay null, distinct from an identifier literally named unknown. Any unresolved
  group, empty set or canonical-outside-only window leaves both usage and
  aggregates null; recorded zero retains a counted bucket. Cross-dimension
  tests preserve the 600-token A/B versus B/C fixture and its sampling-only
  100-token model while explicitly leaving the production max policy unchanged.
  CLI contract tests check version/null fields and unchanged database bytes.
  Full Rust 177 tests, Clippy and governance checks pass. The retained private
  acceptance copy predates source-record-key ingestion; aggregate metadata
  inspection found no keys, so it cannot validate a corrected historical union.
  Three narrow real-copy CLI samples correctly returned missing-key ambiguity,
  no numeric correction and unchanged DB bytes. Read-only SQLite initially
  failed because the idle copy retained WAL mode without sidecars; checkpointed
  only that isolated copy into DELETE mode before byte-preservation checks.
  No live ledger/source migration or installed-app change occurred. Next policy
  acceptance needs keyed forward-ingestion/replay fixtures and a measured shadow
  migration, not extrapolation from missing legacy evidence. Goal remains ACTIVE.
- Batch 122: tested the forward ingestion path rather than only supplied shadow
  measurements. Reproduced reconstruction emitting an ancestor increment as
  child usage when a foreign-history timestamp gap exceeded two seconds. A
  persisted optional foreign-replay flag now keeps those records as baseline
  until the shared canonical-task-start predicate allows resumption. Foreign
  model/cwd are ignored; rewritten outer timestamps and stale task-start data
  do not independently resume counting. Synthetic checkpoint round trips and
  legacy missing-flag decoding pass. The disk-backed source fixture now runs
  native catalog sync, copied sampling-log ingestion, reconstruction and shadow
  union, then reopens the ledger (zero new bytes/observations) and appends one
  request (one observation per source, one new shadow measurement). Final full
  Rust 178 tests, Clippy and governance checks pass; an initial Clippy nesting
  warning was fixed without suppressing the lint. This is a forward parser
  correction, not repair of already consumed foreign history. Formats lacking
  explicit foreign metadata and the existing task-start heuristic still need
  broader evidence. No stored historical facts, database schema, day-max source
  policy, live ledger or installed app changed. Historical shadow migration,
  accounting owner review and native product acceptance remain open; ACTIVE.
- Batch 123: prior turn fixed an explicit foreign-replay counting path. Native
  app/process and fixed-port checks found no running ledger instance, but CUA
  reported the Mac locked and unable to unlock. Requested manual unlock;
  no UI launch/interaction was attempted through an alternate mechanism.
  Continued non-UI work: App preference writes could throw from React effects,
  and restored JSON was blindly merged. Guarded optional storage reads/writes,
  whitelisted fields/types/enums and page bounds, and validated restored custom
  dates. Unknown preference payload fields are dropped, never mixed with usage;
  in-memory navigation survives persistence failures. Supported presets and
  long valid identifiers retain their values. Frontend 63 tests/typecheck/build
  passed. Three browser navigation scenarios are registered but NOT executed;
  native window/zoom/scroll/bilingual acceptance remains unproven. Swift pure
  state and owned-process stop tests passed. Final standard arm64/ad-hoc bundle
  manifest: `ea3419fdfde18c921558fa261437a5dd98726f413f7c2895d799473ae018f475`.
  No notarization, installed-app replacement or live-ledger modification. Goal
  remains ACTIVE; lock-screen dependency affects UI acceptance, not remaining
  useful accounting/coverage work. Resume native interaction after manual unlock.
- Batch 124: previous turn hardened preferences while native UI awaited unlock.
  Reproduced non-Shanghai UTC-today returning 360 instead of 240 synthetic tokens.
  Shared exact-window routing now covers local calendar totals, quality, project
  and conversation scopes, date labels and previous-period comparisons. Initial
  raw-timestamp-only queries were rejected by real-copy acceptance: around 20s
  for today, timeouts and comparison drift where old hourly facts outlived request
  detail. Hour-aligned timezone relabeling now preserves durable hours, open
  lifetime bounds avoid raw full scans, and scalar groups use complete hours.
  Fractional-offset/within-hour transitions fall back to precise evidence only
  when per-dimension counts/components conserve; otherwise HTTP 422 explicitly
  reports unavailable precision rather than returning a partial curve. Synthetic
  UTC/New York DST/Kathmandu fixtures cover five calendar presets and comparisons.
  The legacy-hour shape moved to a disk-backed integration test so API modules
  retain their no-SQL boundary; its real loopback HTTP responses prove UTC 200
  and an unprovable split 422. Full Rust 180 tests, Clippy, API contracts and
  governance gates pass. Final nine private-copy scopes conserve all audited
  fields; UTC/New York samples were about 0.6–1.0s, Kathmandu month still about
  13s and remains a performance follow-up. Private receipt stays outside repo.
  Test server stopped. No schema/source-policy/history migration or installed
  app change; prior native bundle predates this batch. Accounting completeness,
  precision-aware GUI, broader performance and native acceptance remain ACTIVE.
- Batch 125: previous turn added precision-aware calendar queries. Error responses
  now include stable codes alongside the legacy diagnostic string. The HTTP
  client carries typed precision/invalid-parameter/generic failures, validates
  code/status pairs, and does not display raw response bodies. App error state
  retains the cause so switching languages translates an existing error; failed
  requests keep both accepted filters and data. First-load precision failure
  offers an explicit today-range action without resetting account/project/model/
  session scope. Existing-data notices are accessible alerts, not fabricated
  zero responses. HTTP code, client/cancellation, bilingual component rendering
  and preserved-snapshot tests pass: Rust 180, Web 69, Clippy and contract/
  governance gates. CUA recheck still reported the Mac locked; no alternate UI
  mechanism was used and browser/native task acceptance is NOT claimed. Final
  arm64/ad-hoc bundle manifest:
  `fc76a5131c3f1a6153fdf19ffa0307f294f481a545686205271e6e1f0f00c212`.
  No live ledger or installed-app change, no notarization. Source accounting
  migration/coverage, fractional-offset performance and native acceptance remain
  open; full goal ACTIVE.
- Batch 126: previous turn localized precision failures. Added bounded reuse of
  identical exact-series queries only inside one consistent database snapshot.
  Keys include all query arguments and the reader connection's change counter;
  subsequent snapshots start empty. Results are cloned for callers, successes
  only are retained, and scope drop clears memo state on success/error/unwind.
  Budget is 64 entries/about 8 MiB estimated retained payload, not an RSS promise.
  Key-isolation, positive-hit, concurrent writer/next-snapshot, error cleanup and
  size/entry-limit tests pass. Initial compilation exposed four direct test-store
  constructors; initialized their new ephemeral field without changing migration
  fixtures. Full Rust 182 tests, Clippy, API and governance checks pass. Private
  Kathmandu month bundle measured 6.54s versus prior observed 13.16s (not a
  controlled latency guarantee); six curve collections matched a separate
  non-memoized timeseries request. Standard 13-scope audit also passed. Private
  receipt is outside repo, test server stopped. No persisted facts, schema,
  source policy or installed app changed; current native artifact predates this
  batch. Remaining counting/coverage migration, performance and native acceptance
  keep the complete goal ACTIVE.
- Batch 127: previous turn reduced duplicate snapshot calculations. Reproduced
  a counting-source defect: the first cumulative 1100/last-sample 100 snapshot
  emitted 1100 at one instant. Reconstruction now emits only a valid last sample
  when no prior counter exists; missing/invalid last samples establish a baseline
  without a confirmed event. Optional initial-counter-prefix bookkeeping is
  persisted separately and never allocated to time/model/account/project totals.
  Incomparable cache-write coverage leaves that prefix unknown while preserving
  a valid last sample's known fields. Full-component, unchanged-repeat, missing/
  invalid sample and old-checkpoint tests pass. The reconstruction file fixture
  now closes/reopens a disk ledger with a 1000 counter prefix and remains
  incremental; the dual sampling/reconstruction shadow fixture includes the
  same prefix and still pairs only identifiable samples. Full Rust 184 tests,
  Clippy and governance gates pass. Existing event-conflict protection remains,
  so differently reconstructed old event identities are not silently overwritten.
  No historical rescan/repair, schema/source-selection change, live-ledger or
  installed-app update occurred. Main day-max overlap, historical migration and
  native product acceptance remain open; full goal ACTIVE.
- Batch 128: previous turn separated initial counters from samples. Added a
  bounded read-only reconstruction-prefix audit against stored facts/hashes,
  explicit byte/row limits, canonical/root checks and unknown source-key handling.
  Optional device-only drift comparison is diagnostic, never an identity rebind
  or migration receipt. A private two-project cohort had existing main files and
  matching thread records but no full identity-string matches; metadata showed
  unchanged inode components and changed device components. A bounded prefix
  preview under the stored namespace found unchanged token fields, not a proved
  project correction; detailed counts stay in the private receipt. Ledger bytes
  remained unchanged. Code review traced identity mismatch to automatic derived-
  fact deletion/restart. A RED regression reproduced unwanted rereading; the
  collector now preserves facts/rollups/cursors, marks identity verification
  required, and excludes those checkpoints from generic failed-source cleanup.
  Both repeated device drift and actual replacement preserve previous facts.
  Bilingual collection copy explains review rather than futile automatic retry.
  Full Rust 190 tests, Web 70 tests, Clippy/contracts/governance and standard
  arm64/ad-hoc build pass. Bundle manifest:
  `c2e566fad1f26c0d384769bc1a8792a8e0f04f2a898537d9a6d2a913fde280e3`.
  CUA still reported the Mac locked, so native-window acceptance is not claimed.
  No live ledger/source modifications, historical correction, automatic rebind
  or installed-app update occurred. Safe continuation binding, day-max overlap
  replacement, historical receipts and native acceptance remain open; ACTIVE.
- Batch 129: prior turn prevented automatic replacement on identity drift.
  Reproduced a remaining bypass: in-place truncation reached generic error
  cleanup and removed the saved cursor. Typed read-continuity failures now hold
  the source for review and preserve its checkpoint, including read-time identity
  conflicts, unavailable checkpointed reads, truncation and detectable post-read
  changes. Missing/unsupported/malformed parser state or inconsistent tail/cursor
  offsets, line counts, identities or partial extents cannot restart at zero.
  Storage failures/conflicts propagate without deleting a potentially newer
  committed cursor. Review survives temporary file disappearance/return; normal
  incremental append/restart regressions remain green. Bilingual copy now refers
  to source/checkpoint continuity rather than assuming every issue is replacement.
  Full Rust 195 tests, Web 70 tests/build, Clippy/contracts/governance checks pass.
  No live source/ledger or installed application was changed; native bundle was
  not rebuilt in this batch. This is preservation/validation, not approved safe
  rebinding, full race-proof source snapshots, or historical counter correction.
  Continuity recovery, source-union migration and native acceptance keep ACTIVE.
- Batch 130: resumed the pending late-source recovery patch; its prior tool
  handle was no longer available and no compiler was running, so reran scoped
  validation. A source indexed before its file exists previously stayed
  unavailable forever. First binding now resumes automatically only after one
  immediate transaction verifies empty identity, no events/cursor/progress and
  no review marker. Inconsistent unbound history is held for review, and generic
  unavailable-source cleanup preserves its cursor. Missing indexed files now
  report a collection issue rather than silently appearing healthy. Disk reopen/
  arrival/idle tests and seven separate history/progress/review gates pass.
  Full-suite execution exposed two old fixtures crossing the seven-day raw
  retention deadline; raw-upsert test time and replay-test rollup reads were
  corrected separately without relaxing production retention/conflict rules.
  Final Rust 197 tests, Clippy, API contracts and governance checks pass.
  No original ledger, source files, native bundle or installed application changed.
  This fixes first discovery only; safe rebinding of previously observed sources,
  historical union/migration and native acceptance remain unfinished. Full goal
  remains ACTIVE.
- Batch 131: dashboard/data-quality skill guidance focused the conversation page
  on one scope/denominator. Added local own/tree account and model distributions
  from stored usage dimensions, not catalog starting-model labels or current
  login. Available lists conserve all components/counts with the corresponding
  detail total, independent of node pagination; missing exact-window detail is
  nullable instead of borrowing global values. An additive DTO/schema and paired
  bilingual table show total, uncached input, cache read/write, output and records.
  Unknown IDs and cache-write coverage remain distinct from measured zero.
  Session chart data also discards inherited unused global auxiliary series.
  Rust 198 tests and Web 72 tests/build pass, with account/model/root/child and
  old aggregate-only HTTP coverage; Clippy/contracts/governance pass. Mac UI was
  accessible again. The private-copy service exited at the existing compaction-
  verification guard, so no real-ledger page acceptance occurred and that guard
  was not bypassed. Explicit synthetic component harness was visually checked in
  the in-app browser at 900/360px panel widths, switching language and own/tree;
  narrow tables scrolled to the final output/count columns. Harness tab/server
  stopped; failed connection tab cleanup hit the browser error-page URL policy.
  This is component acceptance, not full native/zoom/large-tree acceptance or
  evidence of inference accuracy. No installed app or original ledger change.
  Source-union migration, safe source continuity and private-copy startup failure
  remain priorities; full goal ACTIVE.
- Batch 132: traced private-copy serve startup failure to automatic compaction,
  not inability to query the ledger. Metadata-only paired checks showed hash
  differences without differences in compared token/time/model fields; this
  does not justify ignoring or rewriting hashes. Dashboard-only mode no longer
  compacts raw history, while explicit/collection compaction retains strict
  checks. Retained/raw mismatch now has a distinct rollback error instead of
  misleadingly saying the rollup was never verified. A RED subprocess regression
  reproduced startup exit; final HTTP process test reaches idle and preserves
  raw rows, compacted-key count and conflicting retained hash. Hash/token
  mismatch store regressions prove compaction still rolls back. Rust 200 tests,
  Clippy/contracts/governance pass. The real isolated service starts, all 13
  scope audits pass, and the in-app browser reached conversation detail and
  switched own/tree. Four local distribution scopes conserve components/counts.
  Private findings stay in the external receipt. Tab/server stopped; no installed
  app or original-ledger modification, no native rebuild in this batch. Historical
  hash provenance, safe source rebinding and union/migration accuracy remain open;
  this is a dashboard availability fix, not evidence repair. Full goal ACTIVE.
- Batch 133: data-quality guidance separated metadata identity from token-field
  agreement. A RED synthetic SQL-project-projection/backfill fixture proved the
  old backfill recomputed retained hashes from current projected fields instead
  of preserving raw ingestion hashes, blocking later guarded compaction. Missing
  request detail now copies persisted raw fields/hash directly; filling absent
  companion origin/assignment rows no longer overwrites retained observations or
  reviewed assignments. Existing mismatches are not repaired automatically.
  Added a bounded read-only paginated hash-provenance CLI. On the idle private
  copy, every previously mismatching pair matched current-row serialization on
  the retained side, consistent with the reproduced mechanism; whole-file bytes
  remained unchanged. Exact counts and invocation evidence are in the private
  receipt, not public repo. This does not prove source identity/history or grant
  bulk-repair authority. Full Rust 202 tests, Clippy/contracts/governance pass;
  an initial test fixture reused a unique source position and was corrected to
  distinct positions without changing production constraints. No source/schema/
  installed-app change or historical hash repair occurred. Safe receipt-controlled
  remediation, source-union migration and native acceptance remain open; ACTIVE.
- Batch 134: user approved account-first ADR 0006 and activated its extension
  within this goal. Added a token-only million-unit formatter, retaining raw
  API/storage values and exact title values. Unknown/non-finite/invalid values
  remain unavailable, observed zero stays zero and sub-thousand observations
  cannot round to zero; request counts retain their independent format. Wired
  conversation model/account tables, local composition and shared breakdown
  rankings first. RED formatter tests failed before implementation; final Web
  75 tests/typecheck/build pass. A real Chrome synthetic component test passes
  bilingual unit stability, exact titles, own/tree switching, 900/560px viewport
  and 360px table scrolling to final columns without page overflow. Documentation
  links, boundaries, generated-file, privacy and version checks pass. Source
  accounting, schemas and the original ledger were not modified. Existing native
  language edits remain separate and uncommitted. No native build/install or
  whole-app visual acceptance is claimed. Remaining E01 surfaces, bottom-left
  account navigation, quota history and accounting migration remain open; ACTIVE.
- Batch 135: moved reporting-account selection to a bottom-left dialog trigger,
  with the same control in the narrow-window toolbar. Removed the old filter-row
  selector, retained applied account scope beside the page title and kept account
  changes independent from observed login. Source `active` account metadata,
  plan labels and official observation timestamps are reused; no auth action,
  inference assignment, schema or ledger change. Account selection preserves
  project/chat/model/time scope and resets only pagination. Native dialog focus,
  Escape and read-only explanatory labels are bilingual. Initial browser tests
  exposed a 620/900px breakpoint gap; screenshot review additionally found a
  compressed vertical title. Both were fixed and assertions now cover toolbar
  height and page overflow. Test baselines wait for loaded account options rather
  than comparing pre-load unknown login against later data. Web 76 unit tests,
  typecheck/build and focused real-Chrome account/navigation/filter tests pass;
  four widths and 80/100/160% filter layouts are exercised. Synthetic 560/1280px
  screenshots were inspected. Module, link and privacy checks pass. Native
  language edits remain untouched; no installed-app acceptance or accounting
  correctness claim. E01 remaining surfaces, native account acceptance, source
  policy migration and complete quota history remain open; ACTIVE.
- Batch 136: completed the remaining browser Token-unit conversion across
  overview/account/quality cards, sidebar and project rankings, own/tree nodes,
  quota samples, calendar labels, trend axes/readouts/tables and retained turn/
  request evidence. Metric-aware formatting retains request counts; raw exports
  and exact detail titles remain integers. Signed diagnostic residuals have a
  separate formatter rather than being erased as invalid negative usage. Axis
  gutters accommodate longer M labels without reducing type size. Regression
  expectations now distinguish recorded `0 M` from absent evidence; request
  pagination compares column values and exact titles instead of browser-specific
  row whitespace. Final Web 78 unit tests, typecheck/build and 19 real-Chrome
  responsive/unit/navigation/pagination scenarios pass. Additional focused M
  rerun passes; synthetic narrow trend screenshot inspected with visible axes,
  date readout and navigation. Links, boundaries, privacy, generated-file and
  version checks pass. No accounting/source/schema/native installation change;
  existing native-language edits remain separate. Native acceptance, incomplete
  source-union/migration work, shared model exploration and complete quota-cycle
  history still require implementation and verification. Full goal ACTIVE.
- Batch 137: data-quality workflow identified that the quota-cycle view kept
  only the newest deadline run from a capped snapshot read. Added a pure window
  observation segmenter and integrated a bounded older-interval preview: changed
  deadlines/durations, decreases under unchanged deadlines and conflicting equal-
  time observations remain distinct evidence, not verified reset causes. Token
  samples clip to reporting dates and snapshot clock; uncertain transition gaps
  are not allocated. Account/window stream keys and first-snapshot identities
  separate preview rows; cross-window account samples are never a summable pool
  ledger. Optional DTO fields expose observation end, boundary kind/preceding
  observation and possible truncation; schema/TypeScript generated together.
  Preview caps (20 intervals / latest 1,000 snapshots per account) are explicit
  in contract and bilingual UI, NOT a substitute for the full-history goal.
  Tests cover duplicate ingestion, old runs, same-deadline decreases, unknown
  metadata, account/pool separation, preview bounds and future snapshot exclusion.
  No-record composition remains unavailable; exact observed dates replace stale
  countdowns for past deadlines. Screenshot review found tiny truncated boundary
  text; rows now wrap at readable size and use a three-column layout on wide
  screens. Rust 209 tests, Clippy and generated API checks pass; Web 79 tests,
  typecheck/build and two real-Chrome bilingual wide/narrow quota-page tests pass.
  Synthetic screenshots inspected. Governance checks pass. No production data,
  schema migration, source-union policy, native bundle or installed app changed.
  Full-history incremental storage/paging, independent code-owner review before
  release, verified grant semantics, source accuracy and native acceptance remain
  open. Existing native-language edits preserved. Full goal ACTIVE.
- Batch 138: added schema-36 quota-window projection and a captured historical
  high-water cursor. New normalized snapshots and index rows commit atomically;
  the daemon/dashboard writer advances historical work in bounded 200-snapshot
  chunks without resetting it on startup. A completed tick performs no writes.
  The preview uses indexed windows after completion and retains the direct path
  before completion; values, stream identities and observation order agree.
  Existing projection conflicts reject instead of overwriting. Synthetic 1,005-
  snapshot upgrade/reopen tests preserve original snapshot digests and prove live
  late-timestamp appends do not skip history; forced failures roll back partial
  rows/cursor. Shared-bundle transaction tests avoid nested read transactions.
  Full tests exposed old fixtures that lowered the schema version while keeping
  future tables: test-only cleanup now models true legacy state rather than
  weakening production migration guards. Rust 213 tests, Clippy, generated API
  and governance checks pass. ADR 0007 records storage/public-maintenance bounds.
  No original ledger/private audit copy or installed application was migrated;
  only synthetic temporary databases were used. No frontend/native changes in
  this batch; existing native-language edits remain separate. The UI still has
  its explicit preview limit: full-history seek paging, interval projection,
  health presentation, real-shadow migration/code-owner review, source-union
  accuracy and native acceptance remain open. Full goal ACTIVE.
- Batch 139: implemented schema-37 versioned quota boundaries with the existing
  predecessor rule, atomically updating only a new window and its successor.
  Backfill readiness gates a minimum readable revision; an issued view fixes
  account(s), ledger instance, revision and observation cutoff. HMAC-authenticated
  seek cursors preserve old boundary versions and window membership across late
  appends and disk reopen, without holding a long-lived database transaction.
  The read-only quota-history CLI traverses all retained intervals beyond the
  preview cap, including an all-account ordered view with account ownership on
  each row. It returns observation metadata and safe Token-sampling ranges, not
  copied Token totals or claims of quota grants/source completeness. Synthetic
  traversal covers 1,105 snapshots / 553 intervals; old-view rows/counts remain
  stable when a late insert moves a boundary. Rollback, upgrade, cursor tampering,
  foreign-scope, all-account and read-only subprocess checks pass. Rust 219 tests,
  Clippy and existing generated API checks pass. ADR 0008 documents semantics
  and review gates. No original ledger/private audit copy, native bundle or
  installed app was migrated. Existing native language edits remain separate.
  The HTTP/UI still uses its explicit preview: wiring the full-history reader,
  per-cycle Token/model/project drill-down, real-shadow review, source-union
  correction and native acceptance remain unfinished. Full goal ACTIVE.
- Batch 140: connected full retained quota history to a read-only HTTP endpoint
  and generated response contract, with scope/order/uniqueness/count/range/view
  validation in the client. Missing databases are not recreated; bad cross-account
  cursors are rejected. Duration metadata stays decimal text. Added all/single-
  account history paging, cached previous pages, fixed-view refresh, pending-index
  polling and cancellation. HTTP failure or index waiting retains accepted rows;
  real requests never fall back to demo data. No history rows/cursors are stored
  in browser preferences. Browser inspection showed the old calendar filter
  obscured the history title and implied the wrong scope. Account usage and quota
  history now have separate keyboard-accessible tabs; history hides calendar
  filters, unrelated diagnostics and official-sync actions. Tab selection alone
  persists; scope changes reset navigation, and successful paging focuses the
  first row. Bilingual year/timezone dates and explicit unconnected Token-detail
  wording preserve the metadata-only boundary. Rust 222 tests, Clippy and all
  generated contracts pass; Web 82 tests/build and 24 real-Chrome responsive,
  paging, account/language/keyboard/reload and failure/pending scenarios pass.
  Synthetic wide/narrow screenshots inspected; governance checks pass. The old
  summary preview remains for compatibility but is no longer the history UI.
  No production database migration, installed-app replacement or native rebuild
  occurred. Existing native language edits remain separate. Per-interval Token/
  model/project detail, source-overlap repair, real-shadow review and native
  acceptance remain unfinished. Full goal ACTIVE.
- Batch 141: added signed per-row interval references and an on-demand read-only
  local-usage endpoint. Bounds and the concrete account come from the retained
  history view, not caller dates or quota percentages. Available totals/models/
  projects conserve every additive component and event count in one read snapshot;
  metadata view time and current Token-query time remain separate. Dirty selectors,
  missing samples, unsafe ranges and legacy two-source thread/day overlap return
  explicit statuses with null amounts. The overlap guard includes cross-account
  and recorded-zero cases; it is not a repair of the production max policy.
  Clients validate scope/conservation and never fall back to demo after failure.
  Detail loads only on expansion; pending refresh retains prior available data,
  while newly detected overlap withdraws values. Shared M-unit breakdown tables
  now serve conversation and interval views, retaining exact titles, unknown
  writes and unsplit-input labels. Bilingual identity/time context remains visible
  within expanded detail. Rust 226 tests, Clippy and generated contracts pass;
  Web 84 tests/build and 23 Chrome responsive/history/dimension/table scenarios
  pass, with additional focused history acceptance. Synthetic wide/narrow Token
  detail screenshots inspected. No production database, native bundle or installed
  app changes; existing native-language work remains separate. Source-union
  correction/receipts, real two-account reconciliation and native acceptance
  remain required. Full goal ACTIVE.
- Batch 142: implemented schema-38 durable local-measurement union staging, using
  the existing shared-record planner instead of day-max or unrestricted addition.
  Migration captures indexed source high-water IDs without scanning historical
  facts. Seek cursors, dirty groups, selected observations, unresolved reasons and
  diagnostic counts persist atomically; late source/assignment/key changes enqueue
  only related groups. Explicit conflict handling prevents outer SQLite upserts
  from breaking queue deduplication. Total counterpart work is budgeted across
  each batch; intact over-budget groups defer, oversized first groups roll back.
  The CLI is read-only by default and requires explicit advance on an existing
  current-schema ledger; it never enables production consumption. Synthetic
  tests preserve all nine checked source/selector tables, all Token components
  and dimension sums; the original 600/100 overlap fixture now also passes the
  persisted candidate. Genuine schema-37 upgrade, reopen/no-write ticks, cursor
  continuation with late inserts, cross-month canonical time, account conflicts,
  key/assignment changes, recorded zero, missing identity/thread and failure
  rollback pass. Rust 232 tests, Clippy, API contracts and governance checks pass.
  No original/private audit database or installed/native app was modified; prior
  native-language edits remain separate. Active dashboard accounting remains the
  old max selector: legacy identity/coverage validation, candidate query semantics,
  real-shadow migration/review, two-account reconciliation and native acceptance
  are still required. Full goal ACTIVE.
- Batch 143: implemented bounded-memory whole-file reconstruction comparison,
  sharing the prefix diagnostic's resolver/parser/field comparison and preserving
  its strict interface. A separate read-only source-audit opener explicitly
  accepts unchanged evidence tables in schemas 35–38 without migrating them.
  Streamed reports distinguish EOF, canonical metadata, source changes, all
  stored positions, missing keys, changed/suppressed/new records and nullable
  component totals. Suppression reasons expose the active parser rule; no sum
  becomes a production correction or source-identity proof. The existing private
  audit copy's key/link inventory showed historical promotion is not currently
  possible. Two large real root files were streamed: one preserved every old
  compared measurement but changed during reading; another was stable through
  EOF and contained a material retained cohort suppressed only by the foreign-
  history guard. A second full pass reproduced the latter's framed-record digest
  and categories. Exact private counts/components/commands remain outside the
  public repository. The evidence requires per-record validation, not a blanket
  scaling factor or automatic deletion. Rust 235 tests, Clippy, API contracts and
  governance checks pass, including multi-chunk/limit/orphan/nullable/suppression
  and CLI preservation cases. No real-ledger schema/facts, source bindings or installed
  app were changed in this batch. Prior native edits remain separate. Historical
  correction/identity manifests, union promotion, real two-account reconciliation
  and native acceptance remain unfinished; full goal ACTIVE.
- Batch 144: validated fork boundaries against native-indexed parent files and
  discovered that arbitrary hexadecimal/v4 identifiers supplied a false v7 clock.
  Captured failing tests, then fixed whole-UUID shape/version/variant validation
  in the shared live/reconstruction predicate. Explicit embedded task start time
  remains a supported fallback. Checkpoint regression preserves foreign state
  for old v4 tasks and resumes only subsequent current work. Added a bounded,
  read-only declared-parent Token-info prefix audit with strict ordered equality,
  source-change/limit/mismatch diagnostics and separate zero-write-default
  compatibility that never creates observed zero cache writes. A real sample's
  first eligible boundary moved after correction; almost all prefix observations
  corresponded to the declared parent, with a separately inspected trailing
  rollback snapshot rather than a silently discarded mismatch. A fresh full-file
  audit under the repaired predicate supersedes the previous suppression amount;
  its private receipt retains exact counts, components and source offsets. Rust
  242 tests, Clippy and contract/governance checks pass. No original/isolated
  ledger facts, schemas, source bindings or installed app were changed. Existing
  native edits remain separate. This repairs forward interpretation, not prior
  persisted facts: reviewed per-record historical correction, union promotion,
  real two-account reconciliation and native acceptance still remain. ACTIVE.
- Batch 145: added streamed review-only correction JSONL drafts with a policy/
  identity header, each source position and digest, nullable old/proposed facts,
  suppression reason, coverage completion and a checksum seal. Output is exclusive
  and outside the explicit source home, private mode on Unix; interrupted files
  cannot pass the seal check. Bounded verification checks entry order, counts,
  duplicate positions, identity/key derivation, action consistency and every
  Token invariant. Optional old-ledger verification compares bindings, whole-
  source row count and full old facts/expected absence in one read-only snapshot,
  rejecting stale metadata even when ingestion hashes are unchanged. Generated
  a private full-source draft and verified all represented old values against the
  existing isolated ledger; exact counts/components/digests remain in the private
  receipt. Isolated-ledger SHA-256 and schema are unchanged; no originals, source
  bindings, production selectors or installed/native app were changed. Rust 247
  tests, Clippy and contract/governance checks pass, including private-output,
  non-overwrite, partial scan, tampering/resealed contradictions, truncation,
  stale-fact and CLI cases. This exports and verifies a draft, not an apply path:
  reversible correction projection/review, union promotion, two-account
  reconciliation and native acceptance remain unfinished. Full goal ACTIVE.
- Batch 146: built a separate private SQLite correction-preview artifact with
  old/candidate facts, source-position actions and draft provenance. A single
  input snapshot revalidates the sealed stream; output rows/readiness commit
  together only after verification and full-source coverage checks. Failed seals
  roll back all candidate rows, existing outputs cannot be overwritten, and
  unready/foreign files are rejected. The read-only preview reader opens no
  original ledger/logs and provides exact half-open filters, timezone-aware
  day/Monday-week/month/year buckets and full account/project/model/thread
  distributions. Null identifiers, observed zero and empty results remain
  distinct; every component conserves on each alternative separately. A real
  single-source preview passed all four grains and all dimension/component
  checks; an unaffected subsequent calendar month had equal old/candidate
  records and amounts. Exact private totals and artifact paths remain outside
  the public repository. Rust 251 tests, Clippy and contract/governance checks
  pass, including CLI, source preservation, rollback, timezone, subsecond and
  parameterized-filter cases. The original audit-ledger hash is unchanged;
  no production schema/facts/selector or installed app changed. Prior native
  work remains separate. Source-union comparison/promotion, reviewed historical
  application, two-account reconciliation and native acceptance remain open.
  Full goal ACTIVE.
- Batch 147: integrated the shared M-unit Token component table into overview,
  project and model breakdowns, with identity selection for model drill-down.
  Replaced the seven-row ranking cutoff with complete twenty-row presentation
  pages. Shares retain the full returned scope as denominator; rank indices
  continue across pages. Table/list navigation clamps after shrinking results,
  resets on applied scope/metric changes, and ignores advancing refresh clocks.
  Missing confirmed evidence remains unavailable rather than measured zero;
  partial/missing cache-write observation is distinct. Fixed-height table
  scrolling and sticky headings retain column context, while both languages
  expose row counts and paging controls. A synthetic 46-dimension fixture checks
  last-page selection, exact values, missing/zero semantics, sorting, refresh,
  scope changes and 560/700/900/1280 layouts at 80/100/160 percent zoom. Actual
  overview/project routes are exercised in explicit mock mode; no production
  accounting or native-install acceptance is implied. Broader storage-failure
  regression initially found an outdated expected synthetic conversation title;
  aligned the exact assertion with the unchanged fixture and reran the checks.
  Pending source-union and native-language edits remain separate from this UI
  batch. Typecheck/build, 85 Web unit tests and 33 real-Chrome browser checks
  pass. Inspected the rendered synthetic model table and retained fixed column
  headings while scrolling. Documentation/module/privacy/generated/version and
  reachable-history checks pass. No source accounting, real ledger or installed
  native bundle was changed; the full goal remains ACTIVE.
- Batch 148: completed coverage-only reconciliation in the shared source-union
  planner (CLI report version 3) and schema-39 bounded invalidation of prior
  candidate groups. Matching six consumed amounts retain their values once;
  conflicting observation weights become unknown, not inferred complete input
  coverage. Real write-amount conflicts still withdraw selection. Added genuine
  previous-schema upgrade/source-preservation and all-dimension tests. The
  read-only candidate/sampling comparator uses exact 250-ms neighbors, expanded
  reverse context, a combined observation cap and explicit ambiguity/identity
  classes; it never emits a merged total or invents keys from proximity. A real
  existing preview/audit-copy comparison reproduced a cohort with identical
  consumed amounts/dimensions, many differing coverage weights and missing
  identities, alongside unconfirmed sampling. Exact private evidence is outside
  the repository. Both database hashes and the audit-copy schema remained
  unchanged. Rust 258 tests, Clippy and API/governance checks pass. No original
  ledger, active selector, installed app or account state changed. Legacy
  identity/correction eligibility, candidate query parity, controlled promotion
  and real-account/native acceptance remain unfinished; full goal ACTIVE.
- Batch 149: closed the UI/export consequence of coverage reconciliation. A
  positive recorded cache-write amount remains visible even when its observation
  coverage is zero/uncertain; zero with no coverage remains unavailable and
  observed zero remains zero. Shared display logic now covers model tables,
  composition, project/chat summaries, turns/requests, quota samples, chart
  details and CSV/JSON export. Partial coverage is not rounded up to complete
  at 99.9 percent. Regression checks conserve the displayed four buckets, retain
  exact positive write values and export partial status in both languages.
  Typecheck/build, 88 Web unit tests and 26 browser checks pass; the updated
  positive-write/zero-coverage fixture also passed all three dedicated browser
  checks after its final edit. The installed/native bundle and real ledger are
  unchanged. This is presentation of existing facts, not reconstructed missing
  amounts or account calibration. Full goal remains ACTIVE.
- Batch 150: added same-snapshot reads of the persisted union candidate with
  explicit pending/unresolved/no-records/available states and exact parameterized
  account/project/model/thread filters. Reads do not import source files, advance
  staging or fall back to old aggregates. Summary and hour/day/Monday-week/month/
  year buckets conserve all components across account/project/model/thread;
  NULL and a literal unknown identifier remain distinct. The known A/B versus
  B/C counterexample returns 600 and the 100-token sampling-only model through
  the new reader, while production deliberately remains 500. Exact subsecond
  boundaries, cross-month canonical pairing, repeated DST hours, unknown/zero,
  out-of-scope conflict guards and combined filters pass. Schema 40 adds an
  all-account time index; genuine schema-39 upgrade preserves every compared
  source/candidate field. A synthetic 10,001-model result explicitly exceeds
  the bucket budget rather than truncating; an empty date-range query performs
  zero full-scan steps. CLI read-only/error/old-schema behavior, Rust 264 tests,
  Clippy and API contracts pass. No original or private audit database was
  migrated, and no dashboard reader or installed/native bundle was switched.
  Prior native-language work remains separate. Remaining gates are legacy
  eligibility/correction, controlled shadow promotion, real-account parity and
  native acceptance; full goal ACTIVE.
- Batch 151: completed the pending native language-bootstrap correction and its
  Swift/controller regressions. Language changes now refresh the next document's
  initialization script without reloading the current React scope; unchanged
  updates preserve script objects, and CSP/main-frame/document-start restrictions
  remain intact. Standard arm64 native build and deep ad-hoc verification passed.
  A new isolated profile launched the source bundle, showed empty-evidence state,
  preserved English across native reload and quit/reopen, and returned to Chinese
  across a further native reload. Command-plus/minus/reset, Today without a white
  screen, and 700-pixel/160-percent overview scrolling to the bottom were checked
  in the actual WKWebView. Both final app/service shutdowns released the port.
  The collection-dialog exercise observed enabled state without establishing the
  cancellation path; it was explicitly returned to read-only and is not counted
  as a cancellation pass. Final isolated preferences were collection disabled,
  Chinese and 100 percent; the isolated source directory and all three usage
  evidence tables were empty. See the updated native-preview contract for the
  artifact hash and limits. No installed application or real ledger was replaced.
  Populated native journeys, modal/export/retry acceptance, historical accounting
  promotion and real-account parity remain open; the full goal stays ACTIVE.
- Batch 152: resumed real two-account baseline inspection in the unchanged
  private schema-35 copy. Official, retained sampling and raw reconstruction
  differ in both amount and temporal coverage; retained requests cover only a
  recent interval. An inferred login interval overlaps later verified intervals,
  directing the next historical-attribution review. No historical relabeling or
  claim of cross-account misuse follows merely from those differences. The only
  exact official payload shared across identities was empty zero usage, not a
  reused nonzero total. Private queries, source counts and limitations are saved
  outside the repository, and the audit-copy hash is unchanged.
  Code review found an independent forward bug risk: implicit app-server home
  plus cached account labels could attach a response to the wrong source after
  switching. Implemented shared observed-account scope for automatic, manual and
  thread reads; metadata-only source stamps bracket existing identity observation
  and the fetch, scope revisions reject A/B/A changes, and the child explicitly
  selects the same file-backed Codex home. Account metadata reads bracket usage
  without forcing refresh; mismatched account/thread responses are rejected.
  RPC exchange now has one fixed deadline and errors omit raw provider details.
  Missing/stale/failed scope preserves prior official data rather than zeroing or
  relabeling it. Rust 272 tests, Clippy and API contracts pass, including synthetic
  stdio exchange and no-scope persistence checks. Official documentation and the
  installed protocol schema were inspected; no authenticated real-account query,
  real credential inspection, quota reset, source migration or installed-app
  replacement was performed. Full historical parity and the goal remain open.
- Batch 153: tested the long-inferred-epoch hypothesis instead of assuming it
  caused the account discrepancy. The unchanged private audit copy had zero
  stored reconstruction or retained-assignment mismatches against a unique
  matching verified epoch; current resolvers already prioritize verified
  intervals. Preserved that behavior. A source trace then established that
  retained-sampling quantities are copied from rollout last-usage records, not
  independently emitted input/cache/output counters in the sampling log. Prior
  equal-value comparisons remain consistency checks, not independent accuracy
  proof. Corrected the associated greedy matching defect: a consumed nearest
  candidate cannot force another anchor onto a farther unused row. New batches
  require mutual unique nearest neighbors within the supplied mature thread
  cohort, mark ambiguity unknown without candidate links, use exact wide-integer
  timestamps and record the policy in cursor metadata. Synthetic ingestion
  rejects the spurious leftover quantity/context counter and remains zero-read
  on the next unchanged tick. This does not repair or retroactively certify old
  associations. Legacy enrichment must use source occurrences and actual
  rollout positions with frozen-value validation before reviewed migration.
  Private findings remain outside the repository. Full goal remains ACTIVE.
  Also rejected null/partial/malformed standard usage fields as unknown rather
  than synthesizing confirmed zero. Invalid nearest snapshots retain their place
  in association checks so they cannot be skipped in favor of older quantities.
  Rust 276 tests, Clippy and API contracts pass, including wide-date association
  and invalid-neighbor regressions. No original/audit ledger,
  source binding or installed app was rewritten. The shared-normalization gap
  remains an explicit production-switch blocker, not a completed calibration.
- Batch 154: reproduced an unchanged cumulative snapshot being accepted as new
  sampling consumption, then shared numeric counter normalization between
  sampling and reconstruction. Valid increases use counter deltas instead of
  stale last-usage amounts; unchanged consumed components do not create another
  quantity, and coverage-only changes cannot trigger a false counter reset.
  Candidate cursor JSON version 2 persists the baseline/continuity state; an
  offset-only legacy checkpoint establishes a baseline without replaying history.
  Invalid/missing totals break cumulative continuity and cannot silently fall
  back to an older snapshot. Unavailable nearest candidates retain explicit
  reasons rather than measured zero or a spurious source link. Sampling facts,
  log cursor and candidate counter cursors now commit atomically per supplied
  source cohort; injected cursor-write failure rolls back all three and retry
  succeeds once. Restart, unchanged-tick zero-read, stale-last and shared-origin
  component conservation across account/project/model/thread/calendar tests pass.
  Rust 281 tests, Clippy and generated API contracts pass. This is a numerical
  normalization step, not complete stream normalization: canonical/foreign
  history boundaries, reset/rollback eligibility, legacy provenance, bounded
  cohort performance and controlled union promotion remain open. Repeated
  anchors are withheld as unknown; this does not recover the missing request
  identity or certify history. No original/audit database, installed app or
  account state was changed. Full goal remains ACTIVE.
- Batch 155: reproduced both inherited sampling being accepted as child usage
  and a real fast child sample being suppressed after an eligible own-task start.
  Sampling and reconstruction now share canonical/foreign/prefix/live boundary
  decisions in one private module. Foreign history remains protected across
  long gaps, repeated child metadata and serialized restart; an eligible start
  also ends the initial child prefix without resetting already-live model context.
  Candidate checkpoint version 3 persists boundary state with numeric state.
  Numeric-only upgrades remain protected; intervening counters establish only
  baselines, never usage assigned to the later resumed request. A one-time
  64-KiB-capped matching header read restores a creation timestamp when needed
  without declaring the stream live or re-reading whole history. Future/partial
  records do not advance parser state, including spaced JSON timestamps.
  The shared-origin fixture now includes foreign history and still conserves
  all component/dimension totals. Rust 286 tests, Clippy and API contracts pass.
  A private complete-source read-only comparison reached every stored position
  and reproduced the previous correction preview's unchanged/suppressed cohorts;
  both the framed source digest and audit-copy hash remain unchanged. Strict
  source identity selection first refused device drift; the existing diagnostic
  mode was used only after verifying device-only difference, not for rebinding.
  Counts/identities remain in a private receipt outside the public repository.
  This does not certify the initial-prefix heuristic, malformed-field parity,
  legacy provenance, reset/rollback eligibility or complete account history.
  Controlled union promotion, historical migration, real-account/native acceptance
  and the full goal remain open. No live ledger or installed app was changed.
- Batch 156: reproduced missing-field synthesis and broken cumulative continuity
  assigning a gap to a later timestamp. Sampling/reconstruction now share one
  presence-aware source parser: five standard unsigned fields are required,
  numeric strings remain supported, null write detail is unknown, and all three
  write aliases must agree. Complete non-conserving diagnostic evidence remains
  quarantinable. Reconstruction now persists continuity loss; damaged counters
  or malformed JSON require a new baseline rather than allocating the gap to a
  later model/account/time. A later baseline preserves original-prefix metadata.
  Undated valid counters advance only the baseline. Skipped JSON is validated
  without constructing prompt trees and first-line BOM handling is aligned.
  Metadata-only token_count quota notifications are explicitly excluded from
  quantity candidates and do not break counters or create matching ties.
  A cross-adapter fixture verifies inherited history, stale last usage, a quota
  notification, malformed JSON, restart and resumed increment; component totals
  and all five aggregate dimensions agree without allocating the gap. Existing
  synthetic fixtures now state intended zero fields explicitly, while dedicated
  missing-field tests require unknown. Audit/draft output explains invalid
  cumulative, undated and gap-baseline suppression separately. New drafts use
  reconstruction_shared_stream_v2; old sealed policy-v1 drafts stay verifiable
  without being relabeled, and new reasons cannot masquerade as old policy.
  Rust 291 tests, Clippy and generated API contracts pass. No historical facts,
  live ledger, installed app or account state was changed. Legacy partial-field
  eligibility, reset/rollback interpretation, source-occurrence linkage and
  controlled union promotion still require review; this is not real-account
  calibration or permission to delete partially known history. Full goal ACTIVE.
- Batch 157: reproduced candidate read-ahead being lost when the associated log
  observation crossed the maturity cutoff into the next poll. Added a durable
  mutual association window with one clock per tick, candidate look-ahead of
  one tolerance and reverse-anchor context of two tolerances. Only the mature
  contiguous log-ID prefix commits. Candidate checkpoint version 4 retains
  normalized pending rows, claimed flags and bounded finalized-anchor context;
  a restart matches the pending row with zero new JSON bytes. Read horizons
  follow supplied observations instead of consuming all currently available
  history. Previously claimed records and out-of-order observations cannot
  rewrite prior confirmed associations. Source metadata checks reject observed
  in-place/shrink/read-time changes before stale-window reuse. Capacity overflow
  fails the cohort before fact/cursor commit rather than truncating the window.
  Synthetic clock-controlled tests cover cutoff/restart, look-ahead competition,
  reuse refusal, same-size source mutation, capacity rollback and all Token
  components across account/project/model/thread/calendar aggregates. Rust 296
  tests, Clippy and generated API contracts pass. Legacy missing context and
  arbitrary late arrivals are not retroactively certified, metadata checks are
  not immutable-prefix proof, and the window cap is not a bounded-bootstrap
  guarantee. No live/audit ledger, installed app or account state was changed.
  Historical replay completeness/eligibility, source-occurrence linkage,
  controlled union promotion and real-account/native acceptance remain open.
  Full goal remains ACTIVE.
- Batch 158 (2026-09-08): added parser-policy tagging and lazy, resumable
  reconstruction checkpoint requalification. A valid legacy checkpoint is not
  trusted merely because its byte offset is valid: the shared parser rebuilds
  state through the old committed prefix in bounded slices, discarding proposals
  instead of inserting historical events. A separate compact cursor preserves
  progress and a framed hash chain across restart; no large partial-line buffer
  is stored in that lane. Completion atomically records a receipt and replaces
  only parser state at the same main byte/line boundary. Old partial records
  remain uncommitted for normal tailing. Compact pending-cursor lookup keeps
  upgrades scheduled without file growth; healthy progress is not a source error.
  Main/progress compare-and-swap and source checks preserve concurrent changes,
  and existing history extending into the resume range requires review rather
  than automatic requalification. Unsupported policies preserve the cursor and
  report policy review separately from identity review. Tests demonstrate the
  unsafe old-state continuation, retain the complete preexisting fact/hash, and
  add only the legitimate subsequent child increment; all five dimensions and
  components conserve. Bounded multi-slice/restart, idle no-repeat, partial-line,
  source-change, transaction rollback, overlap and future-policy tests pass.
  Rust 304 tests, Clippy and API contracts pass; the strengthened counterfactual
  checkpoint assertion was also rerun with the focused suite. No original/audit
  ledger or installed app was changed. This qualifies future resumption only,
  not old usage rows or source continuity beyond the stated metadata checks.
  Historical correction/union promotion, real-account parity and populated native
  acceptance remain open; the full goal remains ACTIVE.
- Batch 159: [separated history/tail scheduling](docs/architecture/reconstruction-scheduling.md)
  replaces the hot-source-first rule that could fill every
  file slot indefinitely while new historical sources remained pending. Separate
  history/tail lanes preserve history service and rotate eligible live tails;
  a one-file budget alternates across restarts through a compact allocation
  cursor. Admitted historical files still finish ahead of new admissions, and
  requested budgets are bounded by available work. Invalid out-of-root targets
  are excluded before allocation. A synthetic ingestion sequence now collects
  a cold source despite enough growing hot sources to fill the entire old budget,
  then finishes the remaining tail after reopening the ledger. Empty sources
  are inspected once; idle partial EOF preserves incomplete evidence state without
  reporting endless actionable backfill, and later completion counts once.
  `pendingSources` is documented as actionable work, not historical completeness.
  Rust 310 tests, Clippy and generated API contracts pass.
  No accounting arithmetic, historical rows, main selector or installed app was
  changed. Real-account reconciliation, history repair/union promotion and native
  acceptance remain open. Full goal remains ACTIVE.
- Batch 160 (2026-09-08): connected the resolved request union to the existing
  product query chain through a [read-only main-query preview](docs/architecture/union-main-query-preview.md).
  This goes beyond the separate candidate query DTO: actual async dashboard
  bundles, conversation details, account/project/model breakdowns and curves now
  run on the same selected occurrences in this explicit acceptance lane. Fixed
  two integration hazards: exact-time queries must not append retained-only
  sampling a second time, and API dispatch must not reopen an ordinary connection
  and silently lose the preview selection. Temporary event/day/hour views exist
  only on a READ_ONLY connection; stored views, facts and active policy remain
  unchanged. The synthetic overlapping-source counterexample produces 600 rather
  than 500 across the product chain, with sampling-only models surviving raw
  compaction. Three timezones, component conservation, no-write/file-byte parity,
  pending/unresolved refusal and normal-reopen behavior pass. A new
  `preview-union-bundle` command returns the actual DTO with an explicit preview
  wrapper; HTTP schema/defaults are unchanged. Rust 313 tests, Clippy and API
  contracts pass. A separate private copy of the previous real shadow was migrated
  to the current schema and advanced for bounded union staging; inspected legacy
  sampling groups lack record keys and the preview correctly refuses them. The
  original shadow hash is unchanged; the installed ledger/app were not migrated.
  Next critical path is reviewed historical correction and occurrence linkage,
  then real-account/product reconciliation and populated native installation.
  This is not production promotion or historical completeness; goal stays ACTIVE.
- Batch 161 (2026-09-08): implemented [legacy sampling requalification](docs/architecture/legacy-sampling-requalification.md)
  using retained observation anchors and the same counter/replay/association
  mechanism as live sampling. Missing identity tables no longer end the
  investigation: full-prefix parsing can locate candidate source occurrences
  without requiring expired raw logs to remain present. Unknown anchors and
  invalid/inherited/unchanged candidates remain in the mutual-nearest competition;
  amount equality never selects the candidate. Bounded CLI diagnostics distinguish
  matching amounts, changed amounts, unavailable/ambiguous candidates and original
  unconfirmed records, with write-field coverage changes separate from consumption.
  Optional private links expose offsets/digests for the subsequent reviewed
  migration, not automatic source-key writes or quality upgrades. Synthetic stale
  last-value/re-emit, competing-unknown, read-only, missing-log and budget/schema
  tests pass; Rust 316 tests, Clippy and API contracts pass. Real retained anchors
  were also checked against available primary logs, and one complete retained
  rollout prefix was requalified without source changes. Matching legacy amounts
  and previously unconfirmed proposals were separated; no original/shadow usage
  facts, account labels, installed application or production policy were changed.
  Historical correction application, namespace/occurrence linkage, two-account
  reconciliation and populated native delivery remain open. Full goal ACTIVE.
- Batch 162 (2026-09-08): implemented and exercised [isolated shadow correction application](docs/architecture/review-shadow-corrections.md),
  not just a report of proposed values. The new CLI creates a marked private copy
  and only accepts sealed, explicitly bound corrections there; ordinary ledgers,
  new historical insertions and account/project/thread relabeling are refused.
  Every original row/key is archived before correction. Facts, restored source
  keys, rebuilt day/hour rollups and a post-image receipt commit atomically.
  Reapplication verifies the affected post-image and returns already-applied;
  injected failure rolls back the complete operation. Synthetic nonzero component,
  dimension, rowid, original-file and union-reader checks pass. Rust 319 tests,
  Clippy and API contracts pass. On a newly generated private real shadow, a
  previously sealed source snapshot was corrected successfully; repeat execution
  did not change it again. The retained month's unaffected buckets remained
  identical, while inherited-prefix contributions were removed in the affected
  month. Original rows remain archived; account/project/model labels of surviving
  rows did not change, and raw/day/hour components agree. A newer draft observed
  source growth and was not applied. Installed/original ledgers and policy remain
  unchanged. Next: sampling occurrence-key restoration against corrected facts,
  scope-level union/coverage acceptance, real-account reconciliation and native
  delivery. Full goal remains ACTIVE; shadow success is not production acceptance.
- Batch 163 (2026-09-08): restored sampling occurrence links in the corrected
  [review shadow](docs/architecture/review-shadow-corrections.md) using retained
  anchors, exact source digests/offsets, the sealed correction's file namespace and
  a unique primary-machine binding. The operation rechecks corrected post-images
  and anchor amounts/times under one transaction; it does not update Token facts,
  quality or account/project/model assignments. Review-artifact version 2 adds
  per-link receipts with original keys/hashes. Reapplication is idempotent, failure
  rolls back the complete metadata upgrade, and metadata disagreements remain
  visible union conflicts rather than being relabeled. Synthetic tests cover
  component conservation, coverage uncertainty, conflict preservation and ambiguous
  machine refusal; Rust 323 tests, Clippy and API contracts pass. Real matching
  associations were restored and repeated without additional writes; all retained
  fact hashes/totals match the source copy. A real hour's two-source rows collapse
  one-to-one and all five aggregate dimensions agree. The same hour still returns
  pending through the materialized reader because readiness is currently global:
  scope-local readiness and treatment of nonconfirmed observations are now the
  next explicit product-integration gate, not another arithmetic guess. Installed
  policy, original ledger and account state remain unchanged. Goal stays ACTIVE.
- Batch 164 (2026-09-08): replaced the diagnostic reader's global readiness gate
  with [scope-local resolution](docs/architecture/source-union-query.md). Relevant
  raw groups and stale cached selections are checked together in one snapshot;
  ready scopes stream cached data despite unrelated backfill, and bounded dirty
  scopes resolve directly from database facts without reading files or writing
  progress. Counterpart closure precedes filtering, so cross-account/model conflicts
  cannot be hidden. Unkeyed nonconfirmed observations remain separately reported,
  never added to usage or converted to measured zero; keyed conflicts remain gated.
  Empty/unknown-only/confirmed-zero states stay distinct. Version-2 diagnostic DTO
  exposes resolution and scoped cache state; main HTTP DTO/schema are unchanged.
  Tests cover unrelated work, stale moved rows, filtered conflicts, counterpart
  limits, read-only preservation and nonconfirmed quality transitions. A previously
  blocked real hour now reads available; the wider retained interval also resolves
  with all five aggregate dimensions equal while its unconfirmed observations
  remain visible. Bounded staging in the private shadow then made its complete
  corrected thread snapshot readable from cache despite unrelated global work,
  including a scope larger than the immediate-read cap. Rust 326 tests, Clippy
  and API contracts pass. This fixes the reader prerequisite, not the installed GUI or
  full product-query promotion. Full goal remains ACTIVE.
- Batch 165 (2026-09-08): connected the resolved union query to the existing HTTP
  and React stack through an explicit [read-only preview](docs/architecture/union-http-preview.md).
  Writes and official refresh are disabled; collection is not started. Additive
  policy/model fields make source explanations and actual scoped model labels
  consistent with the data, without using catalog models as usage evidence or
  calling record counts independent requests. Synthetic populated browser journeys
  exercised account, month, model and child drilldown. A real UI regression was
  reproduced: returning from a child changed the parent's Own scope to Subtree.
  Navigation now retains parent view, node filters and scroll; the repeated
  browser journey returns to the original Own total and matching trend. This is
  software QA with synthetic facts, not proof of real-account totals. Production
  query promotion, remaining historical reconciliation, controlled migration and
  populated native acceptance remain open. Rust 328 tests, Web 92 tests, production
  web build, formatting, Clippy, API contracts, current-tree privacy, documentation
  links, module boundaries, generated-file and version checks pass. The synthetic
  browser tab and its owned preview server were closed after verification.
  Full goal stays ACTIVE.
- Batch 166 (2026-09-08): added native `--isolated-union` behind the existing
  UUID-only isolated profile. The bundled service uses read-only union queries;
  saved collection preference cannot start a collector, and all native collection
  controls are disabled. Invalid/duplicate/missing-profile cases are covered by
  Swift tests. Rebuilt the arm64 application through the normal build script;
  metadata, architecture, deep ad-hoc signature and owned-process tests pass.
  A populated synthetic overview rendered in the real WKWebView and its isolated
  database hash stayed unchanged. An earlier unqualified desktop app selection
  briefly launched normal saved collection mode; its owned app/child were stopped,
  and it is explicitly excluded from read-only evidence. See the operational
  caveat in the [preview contract](docs/architecture/union-http-preview.md).
  Explicit isolated launch was then independently checked by process arguments.
  No installation, publication or intentional production migration was performed.
  Full interactive native matrix and real-account migration remain open; ACTIVE.
