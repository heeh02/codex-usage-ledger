# Account-first delivery review

Reviewed against [ADR 0006](../adr/0006-account-first-usage-experience.md).
This is current delivery evidence, not a claim of complete recovered history.

| Requirement | Evidence inspected | Current result |
| --- | --- | --- |
| M-only Token formatting; input/cache/output distinctions | `web/src/shared/tokenDisplay.test.ts`, `cacheWriteDisplay.test.ts`, `web/e2e/token-millions.spec.ts` | Passing; missing write fields remain unavailable, reasoning stays within output |
| Viewed account independent of login | `web/src/components/AccountSwitcher.tsx`, `web/e2e/account-switcher.spec.ts` | Passing across 560/700/900/1280px; choosing a filter does not change login |
| Project/chat/model rankings and own/descendant drilldown | `web/e2e/complete-breakdowns.spec.ts`, `responsive.spec.ts`; installed native project/session checks | Core navigation implemented, paginated ranks retain denominators |
| Calendar ranges, trends, keyboard and bilingual/responsive use | `web/src/charts` tests and responsive E2E; installed Today/scroll/zoom verification | Passing; Today renders, missing dates are not fabricated as zero |
| Historical quota intervals with account/pool/window isolation | `src/store/quota_history_repository/tests.rs`, `tests/quota_history_cli.rs`, `web/e2e/quota-history.spec.ts` | Pagination, frozen views, ownership, late appends and interval detail implemented |
| Quota remaining step plot aligned with Token bars | `QuotaHistoryPanel.tsx`, `QuotaIntervalUsage.tsx`, `QuotaPanel.tsx` | **Missing**: history currently exposes interval summaries/tables, not the aligned chart required by ADR section 6.3 |
| Reviewed source-union migration and restart continuity | Exact-match transfer tests/receipt, union projection tests, formal installation and live queue observations | Migrated; live queue converges; unresolved evidence remains separate |
| Installed application usable while collecting | Native app owns daemon; actual Today bundle returned 200 in ~8.3s; native Today rendered | Verified on Apple Silicon macOS; not notarization or cross-platform package proof |

## Current validation

- Rust all-target/all-feature tests and Clippy passed after snapshot-rollup change.
- Web: 100 unit tests and all 33 E2E tests passed in this review.
- Native bundle manifest: `7ec24059efdbe795cf6e043f38a27ab2e3cb117fd214d2c93f0470d1c4184c0e`.
- Original application and ledger remain recoverably backed up. No raw Codex
  log recovery, login switching, reset redemption or publication was performed.

## Remaining bounded delivery work

Add the quota interval visualization using actual observations and exact interval
Token buckets. Use separate percentage and Token axes with a common time domain.
Do not interpolate unobserved resets or allocate account Token facts to a pool.
Keep the existing interval tables as accessible equivalents and retain unavailable
states where evidence does not support a chart. Add synthetic visual/contract
tests, build and verify the installed interval view. This is an existing ADR
requirement, not authorization for more historical reconstruction.

Historical gaps and unknown pool attribution are accepted evidence limits, not
items to fix by guessing. Exhaustive per-session review is explicitly out of the
remaining work. Goal closure should follow the bounded missing feature and a
delivery check; it must not claim unrecoverable history or unobserved resets.
