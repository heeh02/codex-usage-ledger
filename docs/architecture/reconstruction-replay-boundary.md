# Reconstruction foreign-history boundary

Reconstruction previously treated a gap longer than two seconds in a child's
initial token stream as the start of new work. A retained ancestor replay can
contain such gaps. The synthetic regression reproduced an ancestor increment
being emitted as confirmed child usage before any child task-start evidence.

After canonical metadata has been established, a different `session_meta` ID
now enters a persistent `foreign_replay` state. Foreign token totals update
only the inherited baseline and prefix count; their timestamp gaps cannot end
replay. Foreign model/cwd contexts are ignored. A `task_started` record passing
the shared canonical-task predicate exits replay; subsequent local context and
counter deltas resume normal interpretation. Duplicate canonical metadata alone
does not exit replay.

The predicate is shared with the existing live replay guard: it examines the
task's UUIDv7 time relative to the canonical rollout ID, or embedded task start
time relative to canonical creation with the existing two-second tolerance.
It does not use a rewritten outer record timestamp as evidence of new work.
This remains a format-specific heuristic, not a server-issued request identity.
UUID clock extraction now validates the full version-7 UUID and RFC variant;
random version-4 IDs are not timestamps. See the
[task identity correction and parent-prefix audit](inherited-prefix-audit.md).
Formats without explicit foreign metadata still use the older dense-prefix
handling; they require additional replay/coverage validation.

## Persistence and compatibility

The parser checkpoint adds optional `foreign_replay`, default false when reading
older checkpoints. Fresh checkpoints persist it atomically with facts and the
file cursor through the existing transaction. An old checkpoint cannot reveal
foreign metadata that was consumed before this guard existed; accepting it does
not certify that prior history was replay-free. Downgrading the parser does not
preserve this new guard and is not an accepted migration path.

This is a forward-ingestion correction, not a retrospective rescan or deletion.
Already persisted reconstructed events, source-record IDs, the database schema
and `max_thread_day_v1` selection policy are unchanged. Historical correction
still requires shadow verification and a migration receipt, and accounting
changes require code-owner review before release.

## Evidence

- A long-gap foreign prefix produces no child events across checkpoint JSON
  round trips; an old task-start with a new outer timestamp does not release it.
- A matching task-start restores child delta usage and does not import foreign
  model/cwd. An old checkpoint lacking the optional flag is readable.
- The disk-backed dual-source fixture runs native catalog sync, real synthetic
  SQLite sampling logs, JSONL reconstruction and source-key shadow union.
  Copied sampling logs do not duplicate the two shared measurements. Reopening
  the ledger reads no new bytes or observations; appending one request yields
  exactly one observation per source and one extra shadow measurement.

These are synthetic source-path and persistence checks. They do not establish
full independent inference usage, all historical replay formats or installed
application acceptance.

## Initial cumulative counter is not a current request

When no previous counter is available, the first cumulative snapshot is not
emitted wholesale. Reconstruction emits only valid `last_token_usage`; an
earlier counter prefix must not be assigned to that record's time, model or
account. If the last sample is missing/invalid, the total establishes a baseline
without a confirmed usage event, and later valid deltas can resume from it.

The optional checkpoint field `initial_counter_prefix` preserves a representable
total-minus-last difference for diagnostics. With no valid last sample it holds
the initial total. It is counter bookkeeping, not independent inference usage
or a quantity to add to lifetime/daily/project totals. If component coverage is
incomparable, the prefix is null; a valid last sample still preserves its known
cache-write observations rather than being discarded or forced to zero. Older
checkpoints lacking the field remain readable and do not trigger rescanning.

The synthetic 1100-total/100-last fixture previously produced 1100 at one instant;
it now produces 100 while retaining the 1000 prefix separately. Full token
components, subsequent increments, unchanged re-emits, missing/invalid samples
and incomplete cache-write coverage are checked. Disk-backed restart and the
sampling/reconstruction shadow fixture include an older counter prefix and
remain incremental and conserved.

Already stored reconstruction events are not rewritten. The existing conflict
guard refuses a different payload for the same event identity, so a missing
cursor cannot silently replace older facts under this parser. Historical
correction still requires an explicit shadow migration/receipt. These checks
do not establish full history or repair the separate day-max overlap defect.
