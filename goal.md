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

Current critical accounting finding: `max_thread_day_v1` loses independent
requests when the two sources cover different portions of a day. The verified
A/B versus B/C fixture yields 500 instead of 600 and suppresses a 100-token
model entirely. **This defect is not yet fixed in the main aggregate policy.**
The next policy work must partition request/coverage evidence, shadow all
dimensions, then record a validated migration. Neither max nor an unqualified
sum is an acceptable proof of complete usage. See the
[counterexample and read-only diagnostics](docs/architecture/source-overlap-audit.md).

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
