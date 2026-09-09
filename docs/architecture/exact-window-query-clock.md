# Exact-window query indexing and bundle clock

Schema 35 adds a global `(effective_at,event_id)` retained-request index. The
precise event union projects one effective-time column, rather than two copies
later wrapped in COALESCE. Time predicates can then seek retained rows directly.
Existing raw/reconstruction effective-time indexes remain applicable. No usage
rows, source selection or account/project assignments change in this migration.

All period descriptors within one bundle use a single internal reference clock.
Previously summary, curve and breakdowns independently called current time;
queries lasting seconds could disagree on the first rolling-window boundary.
The reference field is skipped in both HTTP deserialization and serialization,
and cloned dimensional queries retain it. Bundle assembly additionally uses one
SQLite read snapshot for summaries, curves, quality, catalog pages and status.
WAL writers may commit concurrently; their changes appear on the next snapshot,
not halfway through an existing response. This does not freeze separate HTTP
requests or reserve a database revision for later pagination.

The derived source selector is refreshed before the read snapshot opens. If a
writer marks it dirty in that gap, snapshot preparation retries (at most three
attempts) rather than refreshing from inside a nested transaction. Continued
churn returns an explicit retryable query error. Standalone conversation pages
retain their own count/rows transaction; within a bundle they reuse the outer
snapshot. Error paths release that snapshot. Long reads can delay WAL checkpoint
reclamation even though they do not hold a reserved write lock, so latency and
WAL growth still require operational monitoring.

Within that read snapshot, identical exact time-series reads can reuse an
in-memory result. The key contains grain, dimension, exact timestamp bounds,
account/project/model/quality, timezone and the connection's own change counter.
This memo is distinct from the HTTP response cache: it is absent outside the
snapshot and dropped on success, error or stack unwinding. A later snapshot
cannot reuse it; concurrent WAL writes remain governed by SQLite's snapshot.
No memo data is written to the database or browser storage.

The memo accepts at most 64 entries and approximately 8 MiB of retained result/
key payload; oversized values are queried normally without caching. This is an
estimated retained-payload budget, not an operating-system RSS limit. Returning
clones also avoids callers mutating cached rows. Tests check key isolation,
repeat hits, external-write visibility on the next snapshot, error cleanup and
the entry/payload limits. This optimizes repeated calculations, not accounting
selection or the completeness of timestamp evidence.

The quality page uses the same selected-period aggregator for confirmed,
quarantined and unknown events, not whole-day totals for rolling boundaries.
Its confirmed count/components reconcile with the summary. Recent-activity and
project/session activity cutoffs use the bundle reference instant as well. The
recent-15-minute window ends at that instant exclusively, not one second in
the future. These remain observed activity, not a quota conversion or proof of
complete collection.

Rolling-seven-day explorer queries build one thread/hour projection for both
conversation ranking and selected conversation details. Membership merges
per-thread usage into roots before SQL sorting and pagination. Detail node
totals and timelines reuse that projection; page limits affect visible rows,
not the own/subtree denominator. Missing catalog membership is not assigned to
an invented root. No source-selection policy or historical event is changed.

Durable hourly rollups carry Shanghai civil-hour keys. A target timezone can
reuse them only when each stored hour starts on a target hour boundary and its
UTC offset stays constant throughout that hour. This preserves old aggregate
facts after request-detail retention without mixing timezone labels. Open-ended
windows also reuse complete-hour rollups rather than scanning lifetime raw rows.
Scalar dimension totals need exact instants but not local hour labels, so they
use the complete-hour path independently of the requested display timezone.

Fractional offsets or within-hour clock transitions require timestamp evidence
to split a stored hour. That fallback must conserve counts and every component
by dimension against the retained-hour result. A mismatch returns explicit
insufficient-time-precision, not a smaller curve paired with a larger total.
This is a loss-detection gate, not proof of replay-free historical identity.
Large timestamp-backed windows can still cost more than hour-aligned queries.

The timestamp path also applies to non-Shanghai calendar windows, not just
rolling seven days. Summary/quality, dimensional breakdowns, conversation
ranking/detail, project totals/sparklines and previous-period comparisons must
honor the same local-midnight/UTC-instant boundaries. Day-key aggregations derive
their date from timezone-local buckets rather than substituting storage dates.
The shared comparison path also fixes exact rolling comparison boundaries.
Synthetic UTC, New York DST-transition and Kathmandu fractional-offset cases
cover today/week/month/year/custom windows with records just before, at and
after the boundary. Official provider-date records are not relabeled or used to
allocate local project usage by this change; official timezone comparability
and historical source coverage still require separate evidence.

Tests cover genuine schema-34 upgrade, exact usage preservation, time-index
search plans, an event exactly at an anchored rolling boundary, and rejection of
client-supplied reference time. A two-connection WAL test commits new usage and
account-registry state between reads, verifies old values remain consistent,
then sees new values in a fresh snapshot; failure releases the transaction.
Existing compaction/boundary and conservation
regressions remain required. Performance checks on private snapshots are not
production latency guarantees. Publish migration and performance receipts
without user facts; the installed ledger needs separate upgrade acceptance.
