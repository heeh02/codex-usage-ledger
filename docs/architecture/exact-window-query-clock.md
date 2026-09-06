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

The quality page uses the same selected-period aggregator for confirmed,
quarantined and unknown events, not whole-day totals for rolling boundaries.
Its confirmed count/components reconcile with the summary. Recent-activity and
project/session activity cutoffs use the bundle reference instant as well. The
recent-15-minute window ends at that instant exclusively, not one second in
the future. These remain observed activity, not a quota conversion or proof of
complete collection.

Tests cover genuine schema-34 upgrade, exact usage preservation, time-index
search plans, an event exactly at an anchored rolling boundary, and rejection of
client-supplied reference time. A two-connection WAL test commits new usage and
account-registry state between reads, verifies old values remain consistent,
then sees new values in a fresh snapshot; failure releases the transaction.
Existing compaction/boundary and conservation
regressions remain required. Performance checks on private snapshots are not
production latency guarantees. Publish migration and performance receipts
without user facts; the installed ledger needs separate upgrade acceptance.
