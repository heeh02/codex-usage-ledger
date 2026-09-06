# Bundle scope acceptance

`node scripts/audit-bundle-scopes.mjs http://127.0.0.1:<port>` performs GET-only
scope consistency checks against an isolated, quiescent ledger server. The
origin must be plain HTTP on 127.0.0.1 and redirects are refused. Use an explicit
private database copy and empty Codex source directory; do not point it at a
live writer or assume that a temporary snapshot still exists before startup.

Seven period presets and at most two available account/project/model selections
are checked. Each compares summary token components to curve and three breakdown
sums. Reports contain scope labels, elapsed time and mismatching field names,
not dimension identifiers or token values. `sourceAccuracyProven` remains false:
internally consistent output can still reflect an incorrect source policy.
This is neither all-filter coverage nor end-user browser acceptance. Dynamic
current-time windows can shift between calls; use a stable dataset away from
calendar boundaries or add fixed custom ranges for boundary verification.

The synthetic test deliberately alters a curve total and requires nonzero exit
and an explicit field mismatch. Run `node --test scripts/audit-bundle-scopes.test.mjs`.
