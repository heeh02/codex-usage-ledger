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

Tests cover genuine schema-34 upgrade, exact usage preservation, time-index
search plans, an event exactly at an anchored rolling boundary, and rejection of
client-supplied reference time. Existing compaction/boundary and conservation
regressions remain required. Performance checks on private snapshots are not
production latency guarantees. Publish migration and performance receipts
without user facts; the installed ledger needs separate upgrade acceptance.
