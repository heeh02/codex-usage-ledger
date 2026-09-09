# Read-only union HTTP preview

Status: implemented validation path; not production promotion.

`serve --union-preview --db <review-ledger> --web-root <built-web-root>`
serves the existing React application and HTTP query stack using the resolved
occurrence-union projection. Both paths must be explicit. The existing global
projection-readiness gate still applies to complete bundles and aggregate views.
The HTTP service can start for an incomplete projection so that
`/v1/source-union` can serve independently available scopes; this does not make
the complete dashboard bundle available or expose incomplete aggregate totals.

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

## Native isolated validation

Launch the built executable explicitly with `--isolated-profile <UUID>
--isolated-union`. Prepare a resolved synthetic fixture at that profile's existing
isolated database location before launch. The union flag without a valid profile,
duplicate flags and misspelled isolation flags fail closed. No arbitrary database
path is accepted by the native shell. A missing database is a startup failure.
An unresolved global projection withholds the complete dashboard bundle while
allowing scoped diagnostic reads; neither case falls back to the normal ledger.

This additive launch option always runs the bundled Rust `serve --union-preview`
on the existing fixed loopback port. It ignores saved collection preference,
disables all three native collection controls and omits the Codex-home argument.
Existing normal and isolated collection modes are unchanged. Preferences remain
in the UUID-specific domain; no WebKit/bridge/navigation allowlist is broadened.

Native build, architecture/metadata checks, deep ad-hoc signature verification,
Swift pure-state and owned-process stop tests pass. The real WKWebView displayed
the populated synthetic union overview with M totals, trends and project ranking;
the native toolbar displayed the isolation label and disabled collection. The
isolated fixture's full-file hash was unchanged after startup. This proves the
native data path, not complete interactive zoom/scroll or installed delivery.

Operational caveat: selecting a non-running app through desktop automation may
launch it without arguments. During validation an initial unqualified selection
briefly started the normal saved collection mode; the owned app and child were
stopped. That launch is not read-only evidence and no claim of unchanged normal
data is made. Subsequent validation used explicit executable arguments and
independently checked parent/child command lines before selecting the running app.
