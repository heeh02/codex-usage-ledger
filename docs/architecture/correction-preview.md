# Isolated before/after correction preview

This review artifact materializes one [sealed correction draft](reconstruction-correction-draft.md)
into a separate SQLite file. It is not an installed-app ledger, production source
selector, or authorization to correct history. Original and candidate facts are
stored as alternatives and must never be added together.

## Sampling consistency comparison

```sh
codex-usage-ledger compare-correction-sources \
  --preview ./correction-preview.sqlite3 --against-db ./synthetic-ledger.sqlite3 \
  --thread synthetic-thread --start 2026-01-01T00:00:00Z \
  --end 2026-01-02T00:00:00Z --limit 10000
```

This read-only comparison opens a ready preview and a supported schema-35–39
audit ledger. It creates neither database and never migrates either. The two
read transactions are independent snapshots, not a common revision; the preview
manifest hash references its creation evidence, not revalidation against current
source files. `previewRevalidated`, `identityProven` and
`productionPolicyChanged` remain false. There is deliberately no merged total.

For each sampling observation in the exact half-open interval, search candidate
timestamps within an inclusive 250 ms, with nanosecond-precise comparisons.
Candidate context extends 250 ms outside the interval and sampling context
extends 500 ms, so reverse neighbors outside the selected interval can invalidate
apparent uniqueness. Thread/time index seeks load at most the combined requested
observation budget (1–10,000); exceeding it fails rather than silently truncating.
The budget includes both context margins, not just records eventually reported.

Groups distinguish unconfirmed/invalid sampling, missing assignments, absent or
ambiguous neighbors, conflicting dimensions/amounts, different record keys,
compatible shared keys and compatible values without identity. Amount equality
compares six consumed fields; differences in the cache-write observation weight
are counted separately. Group usage sums its sampling side only, with unknown
usage remaining null. Candidate records without sampling neighbors are counted
but are not declared independent inference. `--include-rows` exposes bounded
event/time diagnostics for private inspection; omit it for a compact summary.

Use this result to investigate conflicts before any reviewed union promotion.
Mutually unique neighbors and equal values alone never create a shared key or
authorize deleting, relabeling or adding historical facts.

```sh
codex-usage-ledger create-correction-preview \
  --manifest ./correction-draft.jsonl --against-db ./synthetic-ledger.sqlite3 \
  --output ./correction-preview.sqlite3

codex-usage-ledger read-correction-preview --preview ./correction-preview.sqlite3 \
  --grain month --timezone Asia/Shanghai \
  --start 2026-01-01T00:00:00+08:00 --end 2026-02-01T00:00:00+08:00
```

## Creation and preservation

- The output is exclusively created, private mode 0600 on Unix. Existing files
  and symlinks cannot be overwritten. Choose a dedicated review location; never
  use the preview as the application's `--db`.
- The format has its own application ID and schema version 1. It contains
  `preview_meta`, `preview_changes` and separate `old`/`candidate` rows in
  `preview_facts`, with time/account/project/model indexes. A 256 MiB page limit
  bounds database growth. It is not a full copy of the historical ledger.
- Creation rereads the sealed draft, not Codex rollouts. The source ledger is
  read-only in a single snapshot; each expected old fact/absence and source count
  must still match. Records written before the seal is reached remain inside a
  rollbackable output transaction.
- Only a fully verified draft from a complete, stable source scan can publish
  `ready=1`. Missing seals, changed old facts, partial scans or insertion failures
  roll back all facts/changes. A newly created unready file can remain for
  diagnostics; the preview reader rejects it. No original data is removed.
- Readiness and both branches commit together after source verification returns.
  Provenance retains the draft body hash. This hash is a reference to the draft,
  not a signature or integrity certificate for arbitrary later file edits.

The source files/ledger can change after preview creation. An immutable review
snapshot does not certify their current state; revalidation is required before
any future application. No apply, revert-original, or production-switch action
is provided. Reversibility here means keeping both alternatives outside the
original ledger, not modifying the original then attempting to undo it.

## Scope and dimensions

`read-correction-preview` opens only the preview, read-only and in one snapshot.
It does not open the source ledger, discover authentication, rescan files or
advance collectors. Supported filters are exact account/project/model/thread
identifiers and UTC half-open timestamps (`start <= at < end`). Each alternative
uses its own fact timestamps/dimensions, so a proposed correction that moves a
record is reflected in the appropriate scoped before/after result.

`--timezone` defaults to Asia/Shanghai and affects calendar bucket labels, not
the absolute filter instants. Grains are `day`, Monday-start `week`, calendar
`month`, and calendar `year`. Invalid timezones, reversed/equal bounds and
unsupported grain values fail; there is no fallback to the current day.

Both alternatives return counts, nullable Token usage, and full distributions
by time, account, project, model and thread. Every distribution conserves its
own alternative's count and all seven raw components. Input includes cache;
reasoning is within output. Counts and Token fields are not display-scaled.
The eventual UI applies the shared M-token formatter.

An empty selection has `usage=null`; an observed zero row remains a counted
`usage=0`. Missing dimension keys remain JSON null, distinct from an identifier
literally named `unknown`. More than 10,000 buckets in any dimension rejects the
query rather than silently returning top-N rows. Narrow the filter in that case.
Grouping iterates the selected preview rows; this explicit review query is not
the planned incremental live-dashboard aggregate policy.

The response scope is `single_source_correction_preview`; it includes the exact
applied filter and manifest hash, with `productionPolicyChanged=false` and
`migrationAuthorized=false`. Amounts do not represent a whole project/account,
official usage, quota grants or an already-approved historical correction.

## Evidence and remaining work

Synthetic tests cover real draft-to-preview creation, seal-failure rollback,
non-overwrite/source preservation, all component/dimension sums, four grains,
UTC/Shanghai/New York timezones, subsecond range boundaries, literal/NULL keys,
observed zero versus empty, parameterized filters, invalid/incomplete/foreign
files and read-only CLI behavior. Private real-source previews/receipts remain
outside the repository.

This enables scoped review before promotion. Code-owner review, source/ancestor
identity eligibility, cross-source union, stale-source checks at application,
and production/UI integration remain separate gates.
