# Quota interval chart contract

Question: how did observed remaining quota and recorded account activity change
during this selected interval? This is not an estimate of a fixed Token grant.

The existing native application's interval detail owns rendering. Reuse its chart
primitives and theme; do not introduce another dashboard runtime. Two vertically
aligned panels share one time domain: observed remaining percent (0–100) above,
hourly account Token bars (zero-based, M units) below. Percent is `100 - used`.
The step line is a display convention between sufficiently close observations,
not proof of the actual reset instant; gaps and null observations break it.
Single observations render as points. Never extend a last sample into the future.

`chart` is an additive optional quota-interval response field. Observations are
selected from the same frozen account/stream/revision interval. At most 1,000
points are returned; truncation is explicit and must not look like full coverage.
Token buckets contain only exact interval-filtered facts, clipped to the interval
boundaries, and retain event counts and all component fields. Their sum must
equal interval totals. No-evidence/review/pending responses do not invent bars.
Official percentage observations may still be shown when Token detail is absent.

Use existing quota/Token semantic colors, separate line and bar shapes, explicit
axis labels and an accessible data table. At narrow widths retain readable text
and a shared horizontal scale rather than shrinking labels. Verify Chinese and
English, sparse/unknown samples, truncation, interval boundaries and conservation.
Public examples and screenshots use synthetic data only.
