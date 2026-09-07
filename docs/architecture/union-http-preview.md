# Read-only union HTTP preview

Status: implemented validation path; not production promotion.

`serve --union-preview --db <review-ledger> --web-root <built-web-root>`
serves the existing React application and HTTP query stack using the resolved
occurrence-union projection. Both paths must be explicit. The existing global
projection-readiness gate still applies; partially resolved real ledgers cannot
use this entry point merely because one diagnostic scope is ready.

The mode opens the selected ledger read-only, bypasses collection and identity
initialization, and refuses mutation methods with HTTP 403. Loopback Host/Origin
restrictions remain active. The frontend displays a validation banner and does
not request official thread refresh during drilldown. This is not an installed
application migration, a server-usage claim, or an official-account calibration.

## Additive API compatibility

- Optional `collection.usagePolicy` identifies `request_union_v2` or the normal
  `max_thread_day_v1` policy. Old payloads retain the legacy explanation; unknown
  future policies get neutral text rather than an invented accounting rule.
- Optional `ExplorerSession.actualModels` lists models with records in the
  selected account/time/model/subtree scope. A null element is an unknown model;
  an empty list means no scoped model records. An absent field is unavailable.
  Existing `model` remains catalog metadata, not a statement of actual usage.
- Generated schema and TypeScript types preserve optionality. Contract tests
  cover multiple models across descendants and preview mode/policy markers.
- Record counts are labeled usage records, not unique inference requests.

## Browser regression evidence

A synthetic two-account, two-project, standalone-chat fixture includes three
models, parent/child nodes, sampling-only, reconstruction-only and shared source
occurrences. It uses the real Rust server and built React app, not mocked HTTP.

The month/Luna/All accounts parent view shows Own 23.82 M and Subtree 45.87 M;
the child shows 22.05 M. A reproduced bug changed Own to Subtree on return.
The fix stores parent view, node search/pagination and scroll position in the
navigation trail, applying the view with the returned response. It preserves
the user's current account/model/date filters. Repeating the actual browser
journey now returns to Own 23.82 M, with the corresponding trend unchanged.

Unit tests cover parent filter restoration, model labels and policy text. HTTP
tests verify mutation refusal, foreign Host refusal, unchanged database bytes and
absence of identity initialization. Browser evidence does not replace native
zoom/scroll acceptance, full real-account reconciliation or code-owner review.
