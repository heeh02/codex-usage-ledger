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

Blocking correctness regression (not a goal-status blocker): explicit run of
`exact_window_usage_must_survive_raw_compaction` returns 0 after compaction
instead of 120, despite retained request evidence containing 120. The test is
marked ignored only to label a known unresolved regression, not a pass.
Next accounting integration must restore boundary evidence while respecting
effective source choice and revised attribution; do not simply add both tables.

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
