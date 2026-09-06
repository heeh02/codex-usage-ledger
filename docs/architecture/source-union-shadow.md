# Local measurement union shadow

Status: diagnostic implementation, not the production accounting policy.

```sh
codex-usage-ledger shadow-union --db ./synthetic-ledger.sqlite3 \
  --thread synthetic-thread --start 2026-01-01T00:00:00Z \
  --end 2026-01-02T00:00:00Z --limit 1000
```

The command uses the read-only opener and one SQLite snapshot. It loads retained
and reconstructed observations in the half-open window, plus every counterpart
with the same source-record key even outside the window/thread. No account or
model filter is applied before identity resolution: that could conceal a
conflicting counterpart. A maximum of 10,000 returned observations is supported;
exceeding the chosen limit fails instead of returning a silently truncated union.
Seed IDs are now sought separately through each side's thread/effective-time
index and stopped at the remaining observation budget plus one. Distinct record
keys then expand through the source-record index, and measurement payloads are
fetched by primary key. Each key expands once in the read snapshot. This avoids
materializing the entire dual-source history before filtering. The observation
cap includes counterpart closure; a large shared group fails before planning
rather than being truncated. Stale links can still require extra checks within
that key, so this is not a fixed I/O/latency guarantee or a production incremental
projection.

The pure planner groups shared local source-record keys. One observation per
side with matching dimensions, valid/equal token components and timestamps
within 250ms selects the sampling observation once. A single keyed observation
is retained as a local measurement, not proof of an independent model call.
Duplicate IDs are errors. Multiple observations on one side, absent keys,
unconfirmed/invalid usage, absent assignment rows, conflicting account/project/
model/thread or time differences are unresolved. Unknown account values remain
unknown; an existing unassigned projection is distinct from a missing projection.

The sampling observation's timestamp is canonical for a shared pair. The window
is applied after pairing; this prevents the same measurement being assigned to
two adjacent windows merely because its two source timestamps straddle a boundary.
Unresolved counterparts outside the window still block a complete result.
Only fully resolved, nonempty supplied measurements yield `usage`; unknown and
empty are not fabricated zero. Confirmed zero records remain zero. All additions
are checked for overflow, including cache-write coverage weight and reasoning.

`completeForSuppliedRecords` describes the supplied observation set only.
`historyComplete` and `productionPolicyChanged` are false. The report's selected
records can be inspected across model/account/project/time dimensions but must
not be presented as exact inference usage or a replacement dashboard total.
Source replay, copied files with different identities and absent legacy keys
still require independent coverage/replay evidence and migration receipts.

## Evidence

The A/B versus B/C synthetic fixture yields 600 in the shadow and retains the
100-token sampling-only model; the unchanged production max policy still yields
500 and zero respectively. Tests cover full component conservation, ambiguity
classes, counterpart closure across a window boundary with conflicting accounts,
stable canonical time, zero/empty/overflow and read-only CLI byte preservation.
No live-ledger replay, global reconciliation or historical migration is claimed.

An isolated SQL-plan fixture with 100,000 unrelated rows per evidence side
checks the actual seed/closure statements: zero `FullscanStep`, fewer than 100
VM steps per one-row seed query and fewer than 150 for a two-row closure.
These verify index-seeking behavior with matching index layouts, not production
wall-clock performance. Full-store tests cover cross-window/thread counterpart
fanout, exact cap rejection, preserved conflicts, no writes and the 600-token
partial-overlap case.
