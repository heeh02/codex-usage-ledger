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
