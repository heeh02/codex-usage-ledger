# Final Release Verification — 2026-09-04

> Versioned v0.1.0 evidence. It does not override the current
> [accounting contract](../../contracts/accounting.md).

## Release gate

- Final independent narrow-window retest: **P0=0, P1=0, P2=1 — GO**.
- Prior full independent retest after accounting/page fixes: P0=0, P1=1; the only
  P1 was the native 757px minimum width, fixed before the final retest.
- Non-blocking observation retained: the native popup can expose its previous
  accessibility value briefly while closing. The compact in-page refresh label
  reported by the retest was fixed and independently read back afterward.

## Accounting invariants

- Reconstruction sources: 2,433 reconstructed, 0 pending, 0 unrecoverable.
- Reconstruction bytes: processed equals total.
- `sum(input + output - total) = 0` across reconstruction facts.
- For today, week, month, rolling 7 days and rolling 30 days:
  selected KPI = time-series sum = project sum = model sum = account sum.
- Sampling and reconstruction remain source-selected per thread/day and are
  never added together.

## Performance

Measured against the installed-data corpus after schema 22:

| Filter | Cold bundle | Warm bundle |
|---|---:|---:|
| Today | 0.246s | 0.005s |
| Week | 0.255s | 0.005s |
| Month | 0.243s | 0.005s |
| Rolling 30 days | 0.328s | 0.008s |
| Rolling 7 days | 1.665s | 0.006s |

Five cold period requests in parallel completed in 0.90–2.59 seconds. Exact
rolling windows use durable complete-hour rollups plus raw first/last partial
hours, so the speedup does not weaken the selected-period conservation rule.

## Installed macOS application

- Installed path: `/Applications/Codex Usage Ledger.app`.
- Bundle identifier: `com.heeh02.CodexUsageLedger`.
- Signature authority: Apple Development; deep strict verification passed.
- Database schema: 22; `PRAGMA integrity_check = ok`.
- Collector after restart: `daemon/live`.
- Reconstruction byte waterline and cursor timestamps advanced monotonically
  across restart.
- 900×768 and 620×559 were independently exercised. At 620px the desktop
  sidebar is removed, the mobile page selector appears, all eight period
  controls are visible and keyboard reachable, and no horizontal scrollbar is
  exposed.
- Command zoom was read back at 100%, 110% and 125%; the compact in-page refresh
  control exposes `刷新官方账号用量` in the accessibility tree.

## Independent user-path evidence

The independent tester exercised overview periods, natural week/month crossing,
projects, standalone conversations, project Sessions, large Subagent trees,
own/tree scope, search ancestry, empty search, model and four-bucket composition,
accounts and quota cycles, data quality, privacy mode, Command zoom, native
refresh, 900px and 620px layouts. The final blocking counts were P0=0 and P1=0.

## Responsive and localization hardening — 2026-09-04

- Removed the project/session `overflow: hidden` layout trap. Project overview,
  project Sessions, and Session/Subagent trees now use the same predictable
  workspace scroller and can reach their final row at any supported window size.
- Project-tab and page navigation reset the workspace to the top; period and
  language changes keep the current page context.
- Charts switch between 920×300 desktop geometry and 640×320 compact geometry
  at the same responsive breakpoint. The geometry listener reacts while the
  window is resized; charts no longer depend on a fixed max-height.
- At 700×600 and the minimum 560×560 window, the sidebar is replaced by the page
  selector, all eight periods remain visible in two rows, the status strip wraps,
  and the page scroll reaches charts, rankings, and the footer without horizontal
  overflow.
- Chinese and English were exercised against the installed application. Static
  UI, number/date formatting, generic standalone/unmatched labels, accessibility
  labels, Swift toolbar, menu-bar commands, startup states, and export errors
  follow the selected language. The preference survives application restart and
  switching languages does not issue a ledger query.
