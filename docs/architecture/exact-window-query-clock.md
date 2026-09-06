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
and cloned dimensional queries retain it. This anchors window evaluation only;
it does not freeze database writes or substitute for a transactional snapshot.

The quality page uses the same selected-period aggregator for confirmed,
quarantined and unknown events, not whole-day totals for rolling boundaries.
Its confirmed count/components reconcile with the summary. Recent-activity and
project/session activity cutoffs use the bundle reference instant as well. The
recent-15-minute window ends at that instant exclusively, not one second in
the future. These remain observed activity, not a quota conversion or proof of
complete collection.

Tests cover genuine schema-34 upgrade, exact usage preservation, time-index
search plans, an event exactly at an anchored rolling boundary, and rejection of
client-supplied reference time. Existing compaction/boundary and conservation
regressions remain required. Performance checks on private snapshots are not
production latency guarantees. Publish migration and performance receipts
without user facts; the installed ledger needs separate upgrade acceptance.
