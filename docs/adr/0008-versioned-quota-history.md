# ADR 0008 — Stable full-history quota interval pages

Status: backend implemented; HTTP/UI integration, real-shadow acceptance and code-owner release review pending.  
Date: 2026-09-07  
Parent: [account-first experience](0006-account-first-usage-experience.md),
[normalized window index](0007-quota-window-index.md).

## Context

A capped preview cannot answer historical-cycle questions. Paging over mutable
boundaries also fails when a late observation inserts or removes an earlier
interval start while the user is reading another page. Rebuilding all snapshot
JSON on every page, retaining a long-running SQLite transaction, or forcing
every reader back to page one whenever collection advances are poor defaults.

## Decision

Schema 37 versions the derived boundary starts and gives newly indexed windows
a creation revision. The local predecessor rule is shared with the preview.
Appending a window updates its own boundary and its immediate successor's
boundary in the same transaction as the source snapshot/index append. Unchanged
boundaries do not acquire new versions; duplicate appends do not advance the
revision. Late arrivals can close a previous boundary version or add a new one
without destroying the version used by an earlier view.

Preexisting schema-36 windows receive boundary projections in bounded batches.
A minimum readable revision is established only after both the normalized
window and boundary backfills are ready. Failed batches roll back cursor,
revision and boundary changes together. Source snapshot bytes are not changed.

Each read view fixes a logical ledger instance, revision, account scope and
observation cutoff. Pages seek by timestamp, snapshot identity and window
ordinal, not offset. Equal-time ordering is deterministic, not proof of causal
ordering; conflicting observations remain marked as such. Current-view partial
indexes skip closed historical boundary versions; older views select versions
valid at their captured revision. Detail reads use only windows created by that
revision and calculate sample counts/endpoints from that same view.

Continuation cursors carry an HMAC over their scope, view and boundary key. The
key is application-generated database metadata, not a Codex credential, and is
never returned. Altered and cross-instance/account cursors are rejected. Cursors
survive reopening the same ledger. Simultaneously writable forks of one copied
ledger, deletion of quota facts, or manual rowid/version rewriting are outside
this contract and require an explicit new-instance/repair procedure.

## Read surface and semantics

The package command is read-only and requires an already migrated database:

```bash
codex-usage-ledger quota-history --db ledger.sqlite3 --account all --limit 20
```

Use a concrete account fingerprint for a single account, or `all` to merge the
retained accounts into one ordered view. Each row preserves its account and pool
stream. Pass the returned `next` object as the JSON `--cursor` value for the next
page. Limits are 1–100 rows per page; there is no latest-1,000-snapshot or 20-row
total-history cap. Missing databases are not created, older schemas are not
migrated by this read command, and pending indexes return `indexReady=false`.

Rows are **observed intervals, not verified quota grants**. They retain boundary
kind, observations, percentage values, nominal/reported deadlines and the safe
Token-sampling time range. Uncertain boundary gaps are not allocated; an empty
safe range is null. `sourceHistoryComplete` remains false: complete traversal
of retained observations cannot prove that every real usage/reset was captured.

Token totals are intentionally not copied into versioned quota records. A later
drill-down must query Token evidence for the selected fixed interval and clearly
identify its own ledger version/source. Across-pool samples remain nonsummable;
this work does not equate percentages with fixed Token capacity or identify
reset causes. The old HTTP/UI preview remains capped until it is replaced by
the new history surface; backend pagination is not full product acceptance.

## Compatibility and verification

Original Token ledgers and normalized snapshots remain unchanged. Schema-37
boundary/revision metadata is additive; older binaries must not open it or lower
its schema version. Public response DTOs for the existing dashboard are unchanged.
The history types are exposed through package CLI support, not a new Codex SDK.

Synthetic tests cover more than 1,000 snapshots across many pages; counts and
identities conserve over the full traversal. A late append moves/removes a
boundary while older cursors retain the original rows and sample counts after
disk reopen. All-account pages preserve account ownership. Altered/foreign
cursors fail. Schema-36 backfill and live-update failures roll back without
rewriting snapshots. The subprocess CLI leaves existing database bytes unchanged
and does not create a missing database. Real-shadow migration, HTTP/UI delivery,
Token drill-down, source-union accuracy and native acceptance remain separate.
