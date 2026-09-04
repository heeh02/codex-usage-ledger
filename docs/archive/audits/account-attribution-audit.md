# Account attribution audit

> Archived evidence, not current authority. See the current
> [accounting contract](../../contracts/accounting.md).

Audit date: 2026-08-31 (Asia/Shanghai)

## Finding

The previous dashboard joined two incompatible scopes:

- the official lifetime/month total for the currently signed-in account; and
- local project samples whose historical rows were mostly stored with an empty
  account key and included activity from two accounts.

The resulting 610.5 亿 versus 68.7 亿 comparison was not an account share. It
was one account's official history compared with a short, mixed-account local
coverage window.

## Independent evidence

Codex's read-only log contains two distinct workspace fingerprints and these
switch boundaries (UTC):

- primary account observed before 2026-08-24 18:07:10 logout;
- secondary account observed in the following interval and logged out at
  2026-08-27 15:58:05;
- OAuth succeeded at 2026-08-27 17:25:40 and the primary account was observed
  again afterward.

The official primary-account series independently supports the boundary: its
daily activity is almost zero while the secondary account is active. The app
does not use that correlation to assign tokens; it is only a cross-check.

## Reconstructed local split

On the isolated verification snapshot, after account reconstruction and before
any scaling:

| Scope | Confirmed local Total | Share of attributed local sample |
| --- | ---: | ---: |
| Primary account | about 66.5 亿 | about 92% |
| Secondary account | about 5.72 亿 | about 8% |
| Genuine signed-out gap | 13.2 万 | below 0.01% |

The figures continue to grow while Codex is active. The invariant is the
important result: event count, input, cached input, output, reasoning and Total
are unchanged by reassignment; only the account dimension changes.

## Implemented algorithm

1. Tail every retained Codex `logs_2.sqlite` shard with a durable log-id cursor.
2. Extract only `account_seen`, `logout`, and `login_success` markers.
3. HMAC the raw workspace identifier in memory and discard the raw value.
4. Build non-overlapping login epochs. OAuth waiting gaps remain unknown.
5. Link a historical workspace to a canonical person+workspace account only
   when that workspace is observed in the active read-only auth snapshot.
6. Reassign retained raw events exactly. For compacted history, move only full
   hours/days wholly contained by one epoch.
7. Preserve provisional identities for accounts not active since installation;
   merge them automatically on the next account switch.

After the first scan, a no-change account-history pass reads only rows beyond
the cursor and skips epoch rebuilding. The current verification run completed
an incremental pass in about 1–2 seconds while Codex was actively producing new
sampling rows.

## Official total completeness

`account/usage/read` is scoped to the active ChatGPT account/workspace. The app
currently has an official profile for the primary account only. Therefore:

- primary-account views may use its official Total as authoritative;
- secondary-account views show local samples and "待官方校准";
- all-accounts views show a monotonic real-time lower bound: official totals for
  synchronized accounts, plus each synchronized account's local tail after its
  official coverage, plus selected-period local samples for accounts whose
  official profile is still missing.

The second profile will be captured automatically the next time that account is
active while the app is running.

## Token composition contract

The official profile supplies Total and daily Total buckets, but not an
input/cached-input/output decomposition. The GUI now treats these as separate
facts:

- **Official Total**: account-level backend value, subject to account coverage.
- **Observed composition**: matched local input, cached input, uncached input,
  output and reasoning values.
- **Project/session/model ranking**: local attribution samples only.

Observed composition is never multiplied to fill the official Total. Reasoning
is an output detail and is never added to Total a second time.

## Remaining evidence limits

- A migrated June log shard adds a small amount of independently matched local
  evidence. There is still no retained local post-sampling ledger for much of
  the official May–August history.
- Deleted subagent detail is not restored and cumulative
  `state_5.threads.tokens_used` is never used as a total.
- The official thread billing route was unavailable for every sampled local
  root session, so no project or session value is relabeled as official.

These gaps stay visible as coverage metadata; they are not converted into a
synthetic "官方未归因" project and are not distributed proportionally.
