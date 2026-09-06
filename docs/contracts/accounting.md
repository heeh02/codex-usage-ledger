# Data contract

There are three non-interchangeable Token views:

1. The official account ledger: one backend daily bucket plus the account
   lifetime summary returned by Codex `account/usage/read`.
2. The local attribution ledger: Sampling and replay-safe Reconstruction remain
   separate facts. `effective(thread, local_day)` chooses the more complete
   whole source row; it never sums both. A thread cumulative total is context,
   not a new event.
3. The unexplained account difference: a diagnostic over comparable account-days.
   It does not establish any missing account's usage or a lower bound.

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
- Physical identity changes are not permission to delete reconstruction facts
  or restart history. Retain events and checkpoints pending identity verification;
  device-only/inode-only matches are not automatic rebinding proof. See the
  [bounded preview and identity-review contract](../architecture/reconstruction-prefix-audit.md).
- Without a prior counter, reconstruction must not assign the first cumulative
  snapshot wholesale to the current timestamp. Only a valid last sample enters
  usage; absent/invalid last samples establish a baseline without a confirmed
  event. Any representable older counter prefix is diagnostic only, never a
  lifetime/project total. See [initial counter boundaries](../architecture/reconstruction-replay-boundary.md).
- Model and working directory are attributed from the nearest preceding
  `turn_context` in the same non-replayed stream.
- Account attribution is temporal. A current `auth.json` snapshot never claims
  historical ownership by itself. Historical ownership may be restored only
  from Codex's own account-reload, logout and OAuth-success markers; the ledger
  persists only HMAC workspace fingerprints and timestamps.
- A daily rollup must reconcile event count plus every token dimension before
  raw facts are eligible for compaction. Compaction keeps an immutable event
  key, so replaying an old rollout is idempotent.

## Incremental source projection

The earliest retained date is not a continuous-collection guarantee. Period
metadata now derives that date from the active account/project/model scope,
and leaves local completeness, coverage ratio/offset and comparison coverage
unknown. Local resolved metrics use complete=false (not proven) and ratio=null,
not fabricated 100% or measured 0%. Official coverage remains independently
evaluated from official observations. Actual continuity still requires a
source-interval ledger; this change does not establish one.

Exact boundary queries prefer raw evidence. Once raw events are absent, they
may use retained request evidence joined to current assignments. Event-ID
exclusion prevents summing raw and retained copies; effective queries still
use the thread/day selected source and do not add retained sampling when
reconstruction is selected. Rows without a retained assignment are not guessed.
Synthetic acceptance: a ten-minute window remains 120 before and after raw
compaction; account/project remapping is honored; selected reconstruction yields
220 rather than 220 + 120. This query change rewrites no persisted token facts
and does not prove complete historical detail or source-policy accuracy.

Schema 25 changes only the maintenance of the effective source projection.
Rollup inserts, updates (both old and new keys), and deletes persist a unique
date/thread dirty key in the same write transaction. Refresh recomputes those
keys and clears their queue atomically; unchanged keys are not rebuilt.
Upgrade seeds existing keys once, including stale projection keys so deletion
can be reconciled. Retained event and rollup facts are not changed. The existing
thread/day source-selection policy remains unchanged; this optimization does
not establish that policy's accounting accuracy or complete source coverage.
A failed refresh retains the previous projection and queued work for retry.
Opening the upgraded ledger with an older binary is unsupported; deployment
must retain the pre-upgrade backup until upgrade acceptance.

## Source priority

Sampling maturity is checked at full timestamp precision in log-ID order. The
reader stops at the first not-yet-mature sampling row; it must not filter that
row out and commit a later ID. Invalid seconds/nanoseconds fail without advancing
the checkpoint, rather than clamping nanoseconds or substituting current time.
Within a committed batch, observations are sorted by timestamp for candidate
matching, then returned to log-ID order for durable cursor advancement. Report
first/last timestamps are extrema, not assumed to follow row order. These guards
prevent new gaps; they do not recover requests skipped by previous versions or
prove complete clock/source coverage. A far-future row can defer later rows and
needs operational clock diagnostics rather than silent skipping.

The [local measurement union shadow](../architecture/source-union-shadow.md)
resolves explicit source-record groups independently of the active day-max
projection. It is a read-only diagnostic, not a new official/local total. Missing
or conflicting evidence blocks a complete supplied-record result; source-record
identity is not by itself proof against replay or of server inference identity.

Schema 34 retains [shared local source-record evidence](../adr/0004-source-record-evidence.md)
without changing event identities or accounting selection. A key includes the
physical occurrence and parsed-content digest; copied interpretations of that
record can be compared without treating time proximity as proof. Old absent
keys are not fabricated. This does not prove independent inference usage or
authorize merging inherited source history. Ordinary writes must also respect
compacted event keys, preserving identical replays rather than reinserting them.

The [read-only overlap audit](../architecture/source-overlap-audit.md) classifies
bounded retained-request pages without refreshing source selection or rewriting
history. Category amounts are page-local confirmed observations, not corrected
accounting totals. A consistent rollout candidate is not proven request equality.

An already namespaced sampling source keeps that source/cursor identity when
another source path disappears. Source list position must not replace an
established independent high-water mark. Legacy unnamespaced first-source
bindings still require separate continuity validation; source ordering does
not infer a binding for those cases.
Newly committed sampling cursors record their actual relative source path.
That binding preserves the first source's identity when another higher-priority
path later appears. Existing namespaced cursors remain authoritative for their
own paths. Unbound legacy cursors and physical replacement are not retroactively
proven by this metadata.
New cursors also persist physical file identity, generation and effective event
namespace. A detected physical replacement is read from zero in a new generation
only when observations carry stable receipt identities; copied receipts stay
deduplicated and reused row/turn IDs do not collide with prior generations.
Identity changes during reading or replacement without receipt identity fail
without advancing the source cursor.

Cursor metadata version 4 additionally retains a digest of the last committed
sampling row. An indexed lookup checks this anchor and reads appended rows in
one read-only SQLite snapshot. A changed or missing anchor starts a new source
generation even when the physical file identity and row numbers are unchanged.
Remaining copied receipts are not recounted; missing receipt identity prevents
replay and preserves the checkpoint. An empty replacement leaves the previous
checkpoint intact until observations become available. Each committed batch
stores its own final-row anchor atomically with usage, not the end of a later
uncommitted batch. Ordinary idle reads inspect the anchor but do not rescan
rollouts. The anchor digest is not a cross-source receipt or an account identity.
This detects mutations affecting the anchor, not arbitrary rewrites that leave
that row intact. Older cursors without anchors have no retrospective continuity
proof; missing legacy receipt identities and prior omissions still need audit.

Schema 33 assigns one counting owner per tracked sampling receipt. Copies with
matching immutable request fields and dimensions become receipt aliases without
new usage rows; weaker unknown copies cannot replace confirmed evidence.
A confirmed copy can resolve an unknown owner only while its raw fact is
available for transactional rollup correction. Conflicting dimensions or
unavailable resolution evidence fail without advancing the transaction cursor.
Migration seeds only unambiguous single-owner receipts; preexisting duplicate
groups and records without receipt identity require a separate audit/receipt,
not silent deletion. This does not solve untracked legacy source overlap.

Schema 32 records path-independent sampling receipt keys from machine,
nonempty source process UUID, log-row ID, exact source timestamp, thread and
the sampling log body. Only the digest is retained; missing process identity
produces no key. Receipt evidence is supplemental and excluded from legacy
event hashes. Distinct legacy event IDs may currently share a receipt key:
this is audit evidence, not yet an automatic consolidation policy. Existing
events receive no invented keys during migration.

Schema 27 retains sampling-to-rollout candidate links using the reconstruction
record identity derived from physical file identity and byte position. The
method remains `unique_nearest_timestamp`, not proven request equality.
Supplemental links do not change legacy event hashes or effective totals.
Ambiguous matches have no link. Existing history receives no invented links.
Before any shadow dedup decision, compare effective time and token dimensions:
in-place source rewrites can reuse physical positions and identities.

The read-only single-request candidate audit reports unavailable request,
unlinked, unavailable target, unverifiable time, differing evidence, or consistent
candidate. Consistency requires same thread/model, confirmed sampling quality,
all token components equal, and timestamps within 250 milliseconds using integer
duration comparison. It does not prove one-to-one mapping, complete coverage,
or independent request equality, and never changes effective source selection.
Schema 28 indexes candidate targets. Multiple sampling links to one target are
reported as shared candidates before consistency is considered; they are not
treated as independent one-to-one matches. The index does not rewrite evidence.

Post-sampling timestamp matching must have a unique nearest unused candidate
within its tolerance. Equally near candidates are unknown with an explicit
ambiguity reason, not arbitrarily confirmed. This guards new ingestion only;
it does not retroactively revise stored usage or establish shared request
identity across sampling and reconstruction. Historical policy changes still
require shadow validation and a migration receipt.

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
`unexplained_account_difference_v2`. For each captured account and local day with an
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

This result is not a conservative floor for unobserved accounts. Source replay,
timing, scope and attribution errors can also create a positive difference.
It cannot distinguish one missing account from another, cannot be added to the
official account KPI, and cannot relabel the underlying confirmed local facts.
The API therefore exposes `canSplitByMissingAccount=false`, aligned and excluded
account-day counts, exact coverage dates, allocation delta, and separate
project/model/day breakdowns. Project or model filters select a slice of the
diagnostic allocation while leaving its all-project total available for
conservation checks. Legacy allocation fields are compatibility diagnostics,
not project/account usage evidence; the UI must not present them as an inferred
missing-account composition. `isConservativeFloor` is false.

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

Charts position civil bucket keys on a common calendar axis. A missing bucket
breaks a series instead of being squeezed out or converted to zero. Comparisons
align by calendar date offset from their declared windows, not row index;
nonexistent month dates have no counterpart. The chart labels the grain of the
actual displayed source: official daily buckets cannot be labeled hourly. A
local overlay is unavailable when its grain differs from the official series.
Container-sized drawing preserves readable axis text and keyboard inspection.

The local historical start includes both confirmed Sampling and validated
Reconstruction daily evidence. Unknown/quarantined rows cannot establish that
start. Model choices include effective Reconstruction models even when those
models have no retained Sampling rows. This corrects v1 metadata/catalog
behavior without changing persisted token facts or the wire schema. A first
evidence date alone is not proof of uninterrupted collection; scoped interval
coverage remains an explicit follow-up in the active product goal.

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
local account activity observed since the first trustworthy snapshot inside that
cycle, ending at the earlier of now or the cycle reset. Complete hours use
durable rollups and partial hours use retained request evidence. Missing compacted
boundary evidence can leave the sample incomplete. This activity is not pool
usage: the source does not associate individual requests with a pool.

The nullable v1 fields `localCoverageRatio` and `empiricalTokensPerUsedPercent`
remain present but return null. Elapsed time alone does not establish collection
coverage, and all-account activity cannot establish a pool-specific correlation.
This semantic correction changes no persisted Token fact or quota observation.

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
