# Phase B Replay-safe Reconstruction Verification

> Versioned v0.1.0 evidence. It does not override the current
> [accounting contract](../../contracts/accounting.md).

Verification date: 2026-09-01 (Asia/Shanghai)

## Scope

The production algorithm was exercised against all 609 retained rollout files
attributed to the `project-alpha` project. The run used an isolated ledger database;
it did not mutate Codex state or the installed application ledger.

## Replay reconstruction

| Measure | Result |
|---|---:|
| Indexed rollout sources | 609 |
| Completed | 609 |
| Unrecoverable | 0 |
| Bytes read | 18,035,312,217 |
| Dense inherited-prefix token events excluded | 3,494,331 |
| Unchanged cumulative re-emits excluded | 3,114 |
| Counter epochs restarted | 2 |
| Reconstructed positive-delta events | 82,442 |
| Reconstructed Total | 19,025,073,744 |
| Composition identity error | 0 |

The earlier independent audit found 3,494,279 inherited-prefix records and
3,113 unchanged re-emits. The new general-purpose implementation differs by
only 52 prefix records and one unchanged record because the live project grew
between snapshots. Reconstructed Total is 0.37% above the earlier 18.954B
snapshot for the same reason.

## Thread-day source selection

After importing the independent post-sampling whitelist into the isolated
ledger:

| Measure | Token |
|---|---:|
| Sampling only | 3,081,465,767 |
| Reconstruction only | 19,025,073,744 |
| Naive sum — forbidden | 22,106,539,511 |
| Effective `choose(thread, day)` | 19,049,682,952 |

The effective ledger selected Reconstruction for 417 thread-days and Sampling
for 252 thread-days. It avoided 3,056,856,559 tokens of overlap. The selected
four-bucket composition remained exact:

`input 19,011,461,235 + output 38,221,717 = total 19,049,682,952`.

## Durability and storage

- Cursor, parser state, emitted events and source status are committed in one
  SQLite transaction.
- A restart with no appended bytes emits zero events and advances zero files.
- Each daemon slice reads at most 4 MiB per selected file and persists progress;
  startup never rescans completed content.
- Peak resident memory during the full-project audit stayed below 206 MB.
- The isolated database containing the complete reconstruction, source status,
  post-sampling cross-check and indexes compacted to 137 MB; integrity check was
  `ok` and the freelist was zero after `VACUUM`.
- The signed installed app migrated its existing ledger to schema 20 with
  `integrity_check=ok`. After an app quit/reopen, collector state remained
  `live` and reconstruction advanced from 661,912,072 to 695,466,504 processed
  bytes instead of restarting from zero or presenting a false backfill phase.

## Query performance

On the complete isolated ledger, warm core endpoints measured approximately
23–34 ms for Summary, 22–42 ms for Timeseries, 23–26 ms for Breakdowns and
8–10 ms for Quality. Explorer remains the heaviest response because it includes
multi-period project ranking and Session tree summaries; it improved from about
1.5 seconds to roughly 0.3–0.8 seconds after materializing source selection and
thread-root membership, but does not yet satisfy the `<100 ms` stretch target.

## Product contract proven

1. Child replay prefixes establish a baseline and never become usage.
2. Unchanged counters are zero; regressions start an explicit counter epoch.
3. Sampling and Reconstruction remain physically separate.
4. Effective attribution chooses one whole source per thread/day.
5. Pending and Unrecoverable are durable source states, not inferred zeroes.
6. Project totals, model dimensions and Token composition come from the same
   selected source row.
