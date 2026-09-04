# Independent blind usability test — v15

> Versioned v0.1.0 evidence. It does not override the current
> [accounting contract](../../contracts/accounting.md).

Date: 2026-09-01, Asia/Shanghai

The tester received no repository, design, metric, or prior-conversation
context. It was told only that the local macOS product analyzes Codex usage by
account, project, Session/Subagent, time range, and account quota cycle. It was
not allowed to save settings or modify data.

## Test runs

1. Native macOS accessibility launch was inconclusive: the automation bridge
   did not return a UI state within the bounded run. A separate primary-agent
   check acquired the same installed window in 2.8 seconds, so the failure is
   retained as an automation/accessibility risk rather than attributed to the
   product.
2. A second blind run used the application's own loopback dashboard and real
   ledger. It completed account/time filters, project/model composition, Token
   dimensions, development-intensity ranking, account calibration, and quota
   inspection without reading source or documentation.

## What the tester understood correctly

- Four accounts are user-confirmed, two identities and two official profiles
  are currently captured, and the two missing accounts contribute no invented
  Token values.
- All-account totals are lower bounds while identity or official daily coverage
  is incomplete.
- Official account Total and local project/model/session attribution are
  different ledgers.
- Input, cached input, uncached input, output, and reasoning are separate; the
  reasoning value is a detail of output rather than an additional Total term.
- Project trends, model mix, local activity, active sessions, and high-use
  projects are discoverable.

## Findings and dispositions

| Priority | Finding | Disposition |
|---|---|---|
| P0 | A Session click briefly rendered “not found” while the new bundle was still loading. Direct API and bounded reproduction proved the Session tree existed. | Fixed: render an explicit loading state until the requested Session response arrives. |
| P0 | Long raw prompts and delegation payloads appeared as Session names and accessibility labels. | Fixed: long, multiline, markup-like, and unlabelled subagent titles are replaced with pseudonymous `Session/Subagent <short id>` labels. |
| P0 | No trustworthy quota snapshot existed, so weekly allowance cycle, next reset, and reset history were unavailable. | Fixed daemon wiring: a bounded root-rollout quota tail populated real per-account cycles without modifying Token totals. Small stale percentage dips are rejected as resets. |
| P1 | Account cards showed official bucket-only natural-week values while the overview showed official plus local tail, making two “本周” values look contradictory. | Fixed: account cards use the same reconciled display total and show `≥` when lower-bound. |
| P1 | “最近 15 分钟” sorting still displayed calendar-period totals. | Fixed: the sidebar switches both ordering and displayed values to the same 15-minute window. |
| P1 | Refresh completion was hard to observe. | Fixed: button shows `同步中` and a persistent success/failure receipt with local completion time. |
| P1 | Project composition omitted a direct uncached-input number. | Fixed: Uncached is shown next to Input, Cached, Output, and Reasoning. |
| P1 | Some automation could not reliably address controls by visible label. | Primary reproduction could address the project selector by its label; retain for broader VoiceOver/keyboard audit. |
| P2 | Dense 1280×720 layout, small secondary text, clipped project names, and mixed Chinese/English reduce scanability. | Partially improved wording; density, contrast, and truncation remain in the visual-polish backlog. |
| P2 | Native `⌘−`/`⌘0` behavior was not observable in the browser-only blind run. | Native shell shortcuts remain covered separately; add an explicit zoom-level indicator in a later GUI pass. |

## Remaining acceptance gates

- Switch to each of the two currently unobserved accounts, including the Plus
  account, and verify stable identity, plan type, official profile, and cycle
  separation without changing historic totals.
- Run VoiceOver/keyboard traversal on the signed installed app.
- Re-run the external blind-user task against the final signed build. Primary
  installed-app evidence now shows a 2.5-second cold readable dashboard (down
  from an 8.1-second blank first paint); warm overview/project/session API
  responses are approximately 53/35/36 ms on the current ledger.

## Post-blind runtime evidence

- The dedicated quota tail found official `rate_limits` in every one of the 11
  recent root-session tails considered during the bounded bootstrap.
- The real ledger now exposes weekly and short-window cycles for both captured
  accounts, with plan type, reset boundary, used percentage, local Token sample,
  and coverage kept separate.
- Quota ingestion has its own cursors and leaves `usage_events` and daily Token
  rollups unchanged by construction and test.
- Native `Command-minus` changed the persisted page zoom from 120% to 110%;
  `Command-plus` restored 120%.

## Final blind revalidation

A fresh no-context tester used the final real dashboard after quota wiring. It
reported no P0 failures: startup, refresh, all/current account windows, project
and model drill-down, five Token dimensions, Session/Subagent counts, both
captured accounts' weekly quotas, Spark short/weekly pools, scheduled reset
times, and the 15-minute ranking all worked. It observed a readable loading
state followed by the complete dashboard in roughly 1.5 seconds and a refresh
receipt after roughly 2.1 seconds.

Two findings were fixed immediately after that run:

- Privacy mode now unmounts the data dashboard and exposes only a neutral
  return control. Installed-app DOM verification found no project name,
  pseudonymous account id, or local path while privacy mode was active.
- Session fallback labels now use a 13-character distinguishing prefix and
  reject otherwise-short titles containing `/Users/` or `/home/` paths.

The only remaining account-completeness failure is external evidence: two of
the four user-confirmed accounts have not yet been switched into the running
collector.
