# Data contract

There are three non-interchangeable Token views:

1. The official account ledger: one backend daily bucket plus the account
   lifetime summary returned by Codex `account/usage/read`.
2. The local attribution ledger: Sampling and replay-safe Reconstruction remain
   separate facts. `effective(thread, local_day)` chooses the more complete
   whole source row; it never sums both. A thread cumulative total is context,
   not a new event.
3. The missing-account residual estimate: a derived, conservative floor built
   only from captured account-days where both official and local evidence
   exist. It is neither an official total nor a confirmed local identity.

## Accounting invariants

- Every newly confirmed row must satisfy `total = input + output`,
  `reasoning <= output`, `cache read + cache write <= input`, and cache-write
  coverage weight `<= input`. Replay violations are quarantined and Sampling
  violations are excluded from the request whitelist. Schema 23 guards raw and
  reconstruction facts; schema 24 audits persisted facts and guards confirmed
  durable rollups as the final persistence boundary.

- The four displayed buckets are mutually exclusive:
  `uncached_input = max(input - cache_read - cache_write, 0)`, cache read is
  `cached_input_tokens`, cache write is `cache_write_input_tokens`, and output
  is `output_tokens`. Their sum equals `total = input + output`.
- `cache_write_observed_input_tokens / input_tokens` is an evidence-coverage
  ratio, not Token usage. Legacy events whose source omitted cache-write detail
  remain in uncached input and are explicitly labeled unresolved; the ledger
  never invents a zero cache write for them.
- Account KPIs and the main trend use official daily buckets. Local aggregates
  sum only `confirmed` event deltas; `quarantined` and `unknown` remain visible
  but do not change the official account total.
- The first canonical `session_meta` fixes a rollout identity. Replayed or
  foreign metadata never replaces that identity.
- Repeated unchanged totals produce zero usage. Counter regressions open a new
  epoch or quarantine the ambiguous event; they are never converted into a
  large positive delta.
- A child rollout's dense post-creation Token sequence is an inherited-prefix
  candidate. Its final cumulative value establishes the child baseline; the
  prefix emits no usage. Only later positive deltas enter Reconstruction.
- Pending Reconstruction and Unrecoverable are durable source states. Neither
  is a zero and neither may be replaced with `threads.tokens_used`.
- Model and working directory are attributed from the nearest preceding
  `turn_context` in the same non-replayed stream.
- Account attribution is temporal. A current `auth.json` snapshot never claims
  historical ownership by itself. Historical ownership may be restored only
  from Codex's own account-reload, logout and OAuth-success markers; the ledger
  persists only HMAC workspace fingerprints and timestamps.
- A daily rollup must reconcile event count plus every token dimension before
  raw facts are eligible for compaction. Compaction keeps an immutable event
  key, so replaying an old rollout is idempotent.

## Source priority

1. Codex app-server `account/usage/read` for account lifetime and daily totals.
2. Every retained `logs_2.sqlite` shard, including a migrated
   `sqlite/logs_2.sqlite`, for the local post-sampling request whitelist.
3. Same-thread rollout `last_token_usage` within 250 milliseconds for uncached
   input, cache reads, cache writes, output, reasoning, and total dimensions.
4. Retained rollout cumulative counters for an independent incremental
   Reconstruction ledger after inherited-prefix, unchanged and reset checks.
5. `state_5.sqlite` for the rollout directory, thread lineage, and native
   project/session metadata.
6. Codex auth-log markers plus read-only auth snapshots for historical and
   future account epochs.

Sampling and Reconstruction have equal local-attribution authority only after
validation. The selected row is whichever has the larger internally conserved
Total for the same thread/day; ties prefer Sampling. Project, model, account and
all Token components are taken from that same selected source.

Official profile responses do not include project or model dimensions. The UI
must label local composition and project/session rankings as attribution. It may
compare the two ledgers only when their account, time coverage, and filter scope
match; otherwise the reconciliation is explicitly unavailable.

`project_attribution_coverage_v1` makes that mismatch visible instead of hiding
it behind a generic scope disclaimer. It reports the account Total, named
project evidence, directory-backed standalone-conversation evidence, locally
unmatched evidence, and the remaining amount with no local project evidence.
The four local/official buckets are mutually exclusive. The gap is partitioned
into official usage before the first
local sampling fact, official dates with no local sampling evidence, and the
remaining net difference (including overlapping-day differences, official
summary-versus-day-bucket corrections, and local-tail corrections). Those
buckets reconcile exactly to
`account_total - local_attributed_total`; none may be proportionally allocated
to projects.

Official and local windows are returned independently. For example, an official
lifetime beginning in April must not be labeled with a June local-ledger start
date. The UI shows both ranges and keeps the local project ranking denominator
separate from the official account denominator.

Official Total also does not imply an input/cache-read/cache-write/output split.
Those composition cards and the reasoning detail always describe matched local
samples and display their sample scope. They are never scaled to make their sum
look like the official account Total.

The residual estimate has a separately versioned definition,
`missing_accounts_residual_v1`. For each captured account and local day with an
official bucket:

```text
residual(account, day) = max(local_attributed(account, day)
                             - official_total(account, day), 0)
```

Positive residuals are allocated only inside that same account/day over the
observed project, model, cache-read input, cache-write input,
uncached/unresolved input, reasoning output, and other output weights.
Largest-remainder allocation guarantees that every
dimension sums exactly, `total = input + output`, cached remains a subset of
input, and reasoning remains a subset of output. A missing official day is
excluded rather than converted to official zero.

This result is a conservative floor for all still-unobserved accounts combined.
It cannot distinguish one missing account from another, cannot be added to the
official account KPI, and cannot relabel the underlying confirmed local facts.
The API therefore exposes `canSplitByMissingAccount=false`, aligned and excluded
account-day counts, exact coverage dates, allocation delta, and separate
project/model/day breakdowns. Project or model filters select a slice of the
estimate while leaving its all-project total available for conservation checks.

In an all-accounts view, official totals are authoritative only after every
locally observed account has a successful official profile snapshot. Until
then, the primary KPI is an explicit real-time lower bound calculated per
account: synchronized accounts contribute their official Total plus locally
observed usage after that account's `coverageThrough`; accounts without an
official profile contribute their locally observed usage for the selected
period. A workspace-only provisional identity bounded by its own login epoch is
an observed account scope: its local usage contributes to the lower bound and
is labeled pending calibration until the account is active again. Truly
unknown or signed-out rows with no account scope are not added because they
could already belong to a synchronized profile. This guarantees that an
all-accounts KPI cannot be smaller than any included account without inventing
missing history.

An optional official thread response is a separate calibration object. Its
groups may include model, reasoning effort, speed, input, cached input, output,
total, and estimated credit fields. It is never required for the account
profile to remain authoritative, and an unavailable thread billing route must
not erase or rescale the local session tree.

## Coverage and zero semantics

- Missing dates between the official profile's first and last covered day are
  materialized as covered zeroes.
- Time before `coverageStart` and after `coverageThrough` is unavailable, not
  zero, and is rendered as a hatched region.
- A local comparison is available only when the previous window starts inside
  local coverage. Otherwise its delta and average remain unavailable.
- Official Total can be compared through the daily profile. Input, cache,
  output, reasoning, sampling requests, projects, and models remain local
  attribution unless successful thread calibration explicitly supplies them.

Every primary metric is exposed with a stable `ResolvedMetric` contract:
`value`, `source`, `status`, exact window, timezone, account scope, machine
scope, coverage, and a versioned definition id. Supported states are `exact`,
`lower_bound`, `local_sample`, and `unknown`. Project, model, and session
filters never change the definition of the account Total; they affect only the
local attribution metric.

Multi-account daily reconciliation is performed at `account × local_day`.
Each day is classified as `exact_official`, `local_tail`,
`local_only_account`, or `unknown`, and retains official, local-tail, and
local-only token components separately. The all-account exact window is the
intersection of every observed account's official daily coverage. The latest
date from any one account is freshness metadata and never proves common
coverage. Current-versus-previous percentage changes are unavailable whenever
either reconciled period contains a lower-bound or unknown day.

`userConfirmedAccountCount` is a user-provided completeness target, not a
usage source. The effective known scope is
`max(observedAccountCount, userConfirmedAccountCount)`. An unobserved account
only increments `unobservedAccountCount` and keeps the all-account result at
lower-bound status; it never creates a synthetic identity, daily bucket, or
Token value. A plan label such as Pro or Plus is bound only after the official
quota payload for that observed identity supplies `plan_type`.

Ordinary application startup resumes durable cursors while retaining the last
trusted snapshot. It does not advertise a history backfill. Progress UI is
reserved for a first import, an incomplete rollup migration, or compaction that
is actually running.

## Quota-cycle ledger

Quota is a separate, non-token ledger. Every normalized server snapshot remains
scoped by pseudonymous account, auth epoch, dynamic `limit_id`, window role,
window duration and `resetsAt`. Natural week/month periods continue to answer
project-activity questions; quota cycles answer allowance questions and must not
replace them.

The current cycle is keyed by account, stable server window identity and the
server-provided reset boundary. A roughly 10,080-minute window is labeled
weekly, but no model name is assumed to identify a quota pool. The UI reports
local confirmed Token dimensions observed since the first trustworthy snapshot
inside that cycle, together with sampling coverage. A Token-per-percentage-point
ratio is an empirical correlation over that bounded local sample only; it is
never a billing or quota conversion rate.

A material decrease in server-reported `usedPercent` is recorded as an observed
reset. If it occurs at the previous scheduled boundary it is a scheduled
rollover. If it occurs early, it is an observed official reset whose trigger is
unattributed. Bank Reset, Tibo, account-side credits, or another external reset
must not be named unless the official payload itself provides that provenance.
Future `resetsAt` values are displayed separately as scheduled events.

The daemon captures quota evidence through a dedicated `quota-rollout:*`
cursor namespace. It considers only recently active root sessions, excludes
known and delegation-shaped Subagent rollouts, bootstraps from at most the last
4 MiB per file, and then reads appended complete lines only. A quota line is
bound through the auth epoch covering its own timestamp; if that boundary is
not yet known, the quota cursor stops before the line and retries after account
history reconciliation. This path never emits or changes a Token usage event.
Identical normalized account/time/payload snapshots deduplicate across root
rollouts.

Observed reset events require a decrease of at least five percentage points, a
changed official reset boundary, and a confirming subsequent snapshot within
ten minutes. This rejects small stale fluctuations from concurrent sessions.
Only future reset times are presented as scheduled events.

## Codex directory lifecycle

`thread_catalog` mirrors current Codex membership with `present_in_codex` while
retaining historical labels and hierarchy. A native catalog refresh marks
missing threads historical instead of deleting them. Current Session/Subagent
counts include only rows still present in Codex; historical counts are shown
separately. No Token event, replay key, daily rollup, project attribution or
session tree aggregate is deleted when Codex removes a local thread.

The virtual `__standalone_conversations__` project scope is defined by a
projectless root row remaining after native/root/Git/parent resolution and
confirmed by the native `state_5` directory
(`parent_thread_id IS NULL`, depth zero) and every descendant reachable through
`parent_thread_id`. Sampling-created catalog rows alone do not qualify. It
includes current and historical native roots and their subagents, even when a
child later exposes a working directory that resembles a configured project.
The reserved `unassigned`
scope is the complement: locally sampled facts that are in neither one of
those indexed trees nor a concrete project. These scopes must never be merged
or displayed under the same label.

During the seven-day raw window, every fact retains machine, source, rollout,
file identity, byte offset, and quality state. Older facts retain their complete
token dimensions in the daily grain plus an immutable event-id/hash key for
replay rejection; prompt content is never stored.

## Account reconstruction

The account-history reader has its own durable cursor per Codex log shard. The
first run scans account markers once; later runs query only rows beyond the
stored log id. A logout closes the current interval. An account reload inside
the next interval identifies that workspace, and an OAuth-success marker keeps
the signed-out gap unassigned. Workspace-only identities remain provisional
until that account is active again. They remain distinct observed account
scopes, but never make the all-account result exact. When the verified auth
snapshot arrives, it merges the provisional key without changing any token
dimension.

Reassignment updates raw events exactly and moves only complete compacted hours
or days. Boundary periods stay unknown unless their raw events still exist. The
sum of event count, input, cache read, cache write, cache-write coverage weight,
output, reasoning and Total must remain unchanged before and after reconstruction.

## Project and session explorer

The explorer deliberately joins two independent ledgers:

- `thread_catalog` mirrors only lightweight, read-only Codex metadata such as
  project, title, model, timestamps and the `parent_thread_id` encoded in a
  subagent source descriptor. Dashboard-only mode may refresh this catalog
  without scanning rollout JSONL.
- Token values come from matched post-sampling facts: the last seven days retain
  `usage_events`, while older facts are served from verified daily rollups.
  Codex `threads.tokens_used` is never used for any total.

For every session tree, `ownUsage` is the sum of confirmed deltas whose
`thread_id` is that exact node. `treeUsage` is the sum of `ownUsage` for that
node and all descendants recovered through `parent_thread_id`. A parent total
is therefore never added to a child's total, and a child never inherits an
ancestor's cumulative counter.

Directory counts and token counts have different evidence boundaries. A
zero-token session or subagent can appear in the catalog, while its trusted
usage remains zero until a confirmed sampling delta exists. The explorer API
does not expose first-user-message text or absolute working directories.
Subagents whose source metadata lacks a provable parent remain visible as
`orphan_subagent`; the UI never invents a session relationship for them.
