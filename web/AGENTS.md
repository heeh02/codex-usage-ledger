# Web Dashboard Instructions

The Web dashboard presents API facts. It must not reimplement accounting,
identity assignment, project resolution, or missing-account estimation.

## Boundaries

- Keep HTTP access in the API client and typed contract layer.
- Features may import `shared/`; features must not import another feature's private files.
- Keep request filters separate from the last applied response so labels and data
  switch atomically.
- Persist only presentation preferences. Never persist account usage, raw prompts,
  project paths, or credentials in browser storage.
- WKWebView messages use typed bridge functions rather than inline casts.

## UI and localization

- Every user-visible string and accessibility label must exist in both locale catalogs.
- Preserve 560, 700, 900, and 1280 pixel layouts, 80–160% zoom, keyboard access,
  visible focus, reduced motion, and no horizontal page overflow.
- A project, session, or long Subagent tree must remain scrollable to its final row.
- Charts must conserve the selected query result and resize without clipping labels.

## Tests

- Run typecheck, unit tests, and production build for logic changes.
- Run responsive browser tests when changing layout, navigation, filters, charts,
  localization, privacy mode, or export.
- Do not update visual snapshots until the semantic assertions pass.

## Code Review Rules

- Reject inline account/project arithmetic that is not a pure display conversion.
- Reject hard-coded Chinese/English pairs in feature components after locale catalogs land.
- Reject CSS selectors that depend on unrelated page internals.
