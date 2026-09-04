# v16 verification receipt

> Versioned v0.1.0 evidence. It does not override the current
> [accounting contract](../../contracts/accounting.md).

Verified on 2026-09-01, Asia/Shanghai, against the installed application and a
live 62+ MiB local ledger. Values below are an evidence snapshot, not fixed
product constants.

## Automated gates

- Rust library tests: 82 passed.
- Rust local HTTP tests: 3 passed.
- Frontend TypeScript check and production Vite build: passed.
- Clippy with all targets/features and warnings denied: passed.
- `git diff --check`: passed.
- App bundle metadata, arm64 binaries, nested signatures, and final bundle
  signature: passed.

## Runtime and data invariants

- Installed path: `/Applications/Codex Usage Ledger.app`.
- Ledger schema: 16; `PRAGMA integrity_check`: `ok`.
- User-confirmed account scope: 4; observed/official profiles: 2; two accounts
  remain intentionally unobserved and receive no synthetic Token values.
- Native page zoom changed from 120% to 110% with Command-minus and returned to
  120% with Command-plus.
- Warm API latency on the real ledger: overview about 53 ms, project about
  35 ms, and a 574-node Session about 36 ms.
- A live matrix queried both captured accounts and the all-account scope over
  today, natural week, rolling seven days, natural month, rolling thirty days,
  twelve weeks, twelve months, and lifetime. All 24 account/period requests
  returned non-null display totals, each single-account response reported a
  one-account scope, and every all-account lower bound was greater than or equal
  to each included single-account value for the same exact window.
- The same 24-request matrix now verifies six additional conservation laws per
  response: named projects plus local-unassigned equals local attribution;
  project breakdown and project trend totals each equal the summary; the main
  trend equals the summary; attribution-gap buckets equal account Total minus
  local attribution; and the displayed non-negative gap equals that residual.
  All checks passed.
- The matrix exposed a real rolling-seven-day defect: the KPI and project
  breakdown used an exact 7×24-hour boundary while the trend included the full
  first calendar day. The trend was 757M tokens too high on that snapshot. The
  rolling-seven summary, breakdown, main trend and per-project trend now all use
  retained raw events at the same exact timestamp boundary; a dedicated
  regression test protects it.
- Overlapping historical inferred and verified auth epochs are resolved by
  evidence confidence before recency. Dedicated tests prove that a newer
  inferred interval cannot override a verified interval for either Token events
  or quota snapshots.
- Before WebView prewarming, the native window appeared in about 1.25 s but its
  first frame was blank and the readable accessibility tree arrived around
  8.1 s. After prewarming and retaining the branded placeholder until
  `didFinish`, the first state containing the complete dashboard arrived in
  about 2.5 s; a later installed run was readable in the second bounded state
  fetch without a blank application surface.

## Quota-cycle evidence

- The earlier live ledger contained zero quota snapshots because daemon mode
  did not call the rollout quota parser.
- The dedicated bounded quota tail found `rate_limits` in all 11 recent root
  rollouts selected on its first real run, created independent
  `quota-rollout:*` cursors, and populated both captured accounts.
- The live recovery snapshot contained 2,001 unique quota snapshots while Token
  usage events and daily rollups remained separate.
- Both accounts expose Codex main weekly cycles; the captured plan type is Pro.
  Dynamic Spark short/weekly windows and Credits are retained as independent
  pools. A future Luna-named pool will be displayed from its official
  `limit_name` without code changes.
- Concurrent sessions produced one-percentage-point stale dips. The reset
  algorithm now requires at least a five-point decrease, a changed official
  reset boundary, and a confirming subsequent snapshot. False early-reset
  events disappeared; only future scheduled resets remained in the live
  timeline.

## Missing-account residual evidence

- The user-confirmed scope is four accounts; two have official profiles and two
  remain unobserved. The estimator is enabled only in the all-account view and
  reports `canSplitByMissingAccount=false`.
- On the lifetime real-ledger snapshot, 13 captured `account × day` cells had
  both local and official evidence. Three cells contained positive local
  over-attribution; two local account-days without an official bucket were
  excluded rather than treated as official zero.
- The resulting combined conservative floor for the two unobserved accounts was
  1,792,290,563 Total tokens: 1,787,736,108 input, 1,747,745,810 cached input,
  39,990,298 uncached input, 4,554,455 output, and 2,023,769 reasoning tokens.
  Reasoning remains an output subset.
- Project allocation reconciled exactly: project-alpha 738,275,764; project-beta
  586,777,959; project-gamma 268,295,400; project-delta 156,104,362; and
  unassigned 42,837,078. Project, model and day sums each equal the estimator
  total; allocation rounding delta and component invariant mismatch were both
  zero.
- Eight time windows (`today`, natural week, rolling seven days, natural month,
  rolling thirty days, twelve weeks, twelve months and lifetime) passed account,
  input/output, cache/reasoning subset, project/model/day and residual-delta
  invariants. Every project and model filter returned exactly its allocated
  slice while preserving the all-dimension parent total. A single captured
  account correctly marks the estimator not applicable.
- Visual checks proved the overview, project-filtered estimate, single-account
  suppression, no-false-zero state for an uncovered current day, and 900 px /
  620 px layouts without horizontal overflow.

## Project-attribution coverage evidence

- A live lifetime snapshot showed an account lower bound near 95.0B tokens and
  only about 12.3B of retained, confirmed local project evidence. The product no
  longer presents those as if they had the same coverage.
- The 82.7B difference reconciled exactly into 5.02B of official usage before
  the first retained local sampling fact, 49.38B on official dates with no local
  sampling evidence, and 28.31B of remaining net difference. The latter can
  contain overlapping-day account/local differences, official summary versus
  day-bucket corrections, local-tail corrections, other devices, deleted
  detail or unmatched requests; it is never assigned to a project.
- The overview displays the explicit equation `named projects + standalone
  conversations + locally unmatched + no project evidence = account Total`,
  separate official and local date ranges, and a project-attribution coverage
  ratio. The sidebar uses
  the same displayed account lower bound as its denominator and labels the
  ranking as local project evidence.
- Live visual checks at 1280×900, 900×700 and 620×760 found no horizontal
  overflow. Selecting `今日` retained the complete application and coverage
  panel with no inline error or blank screen; browser console warnings/errors
  were empty.
- Native projectless root trees are now shown as `独立对话`, while null-project
  sampling rows outside those trees remain `未匹配记录`. A real copied-ledger
  check reclassified a same-Git project-alpha worktree into project-alpha without changing
  the 12.52B local Token total; the remaining standalone scope contained 90
  current roots, five with retained sampling evidence, and about 0.79B tokens.
- Session titles containing long API-key-like patterns are replaced by an
  anonymous short Session id before entering the GUI or accessibility tree.

## Cache read/write composition evidence

- OpenAI's current usage schema separates uncached input, cached reads,
  cache-write input and output. Current Codex rollout `last_token_usage` also
  exposes `cache_write_input_tokens`; retained local samples observed the field
  with zero values, while older schema rows may omit it.
- Schema 16 adds cache-write tokens and an input-token-weighted observability
  column to raw, hourly and daily ledgers. A v15 copy migrated with its original
  Total unchanged and `PRAGMA integrity_check=ok`. New cache-aware events update
  both rollups through replaced triggers.
- On the real 12-week copy, the local sample was 11,932,452,298 Total tokens:
  219,315,319 input/legacy-unresolved, 11,688,164,352 cache reads, an unavailable
  cache-write split at 0% historical field coverage, and 24,972,627 output.
  The four displayed buckets reconcile exactly to the local Total.
- The 89.6B account Total remains a separate all-device official metric. The
  top KPI now displays `本机组成样本` instead of cache hit rate; the composition
  panel explicitly says it is an 11.9B local sample, never a split of the
  official account Total. Unknown cache-write history renders as `—`, not zero.

## Recovery drill

An earlier online SQLite backup was restored into an isolated temporary
directory and opened with the installed binary. `doctor` and a weekly `summary`
both succeeded. That pre-cache-write receipt reported:

- schema 15 and integrity `ok` (the schema-16 migration receipt is recorded
  above);
- account completeness target 4;
- 2,001 quota snapshots;
- 36,176 retained raw usage events and 1,219 daily rollup rows;
- 11,795 compacted replay keys;
- complete rollup state;
- `writesCodexAuth=false` and `oauthRefresh=false`.

## Remaining external evidence

- The two unobserved accounts, including the user-identified Plus account, do
  not need to be switched in for the current combined-floor acceptance target.
  Switching them in later is still required to split their individual totals,
  capture plan types, and establish independent quota cycles.
- App Store/Developer ID notarization is outside the local-development evidence;
  the installed build is signed with the machine's Apple Development identity.
