# Durable request evidence

Status: implementation design, not a deployed schema.

## Verified gap

The current maintenance compactor copies only event ID/hash and compaction time
into compacted keys before deleting raw usage events. Daily/hourly rollups retain
aggregate dimensions, not individual request timestamps or turn membership.
The age-based ingest path can write compact keys and rollup deltas directly,
without ever retaining a raw event. Both paths must be upgraded together.

Sampling currently carries an optional source turn identifier through the
provenance file-identity field. A turn can contain multiple sampling requests;
neither that overloaded field nor one event per timestamp proves a turn boundary.

## Required model

- Persist a compact request fact keyed by the existing canonical event ID and
  source kind. Keep event hash, observed/effective time, thread key, optional
  explicit turn ID, model, observed account attribution, project attribution,
  attribution confidence, token components and quality.
- Preserve absent turn IDs as absent. Never use a thread ID as a synthetic turn.
- Keep account/project attribution updates versioned or transactionally aligned
  with rollups; a historical observed identity and current assignment are
  different concepts.
- Exclude prompt/completion bodies, credentials, full log lines and source paths.
- A request fact is evidence, not an extra amount to add to effective rollups.
  Alternative sampling/reconstruction sources remain independently identifiable.
- Turn views aggregate only explicit same-thread turn memberships. Unassigned
  request facts remain browsable without pretending they form one turn.

## Transaction and upgrade order

1. Add a new schema migration and typed request-evidence write path.
2. Backfill only retained raw events, with a durable cursor and progress.
   Existing compact keys cannot reconstruct lost fields; mark their detail
   coverage unavailable rather than generating facts from aggregate totals.
3. Write request evidence atomically with both ordinary ingestion and the direct
   old-event compact path, before advancing the source cursor.
4. Require the compactor to verify request identity and all token dimensions for
   each candidate before raw deletion. Failure rolls back the entire chunk.
5. Record upgrade counts, exact preserved totals and unavailable historical
   detail counts in a migration receipt. No accounting-source replacement.
6. Expose paged thread/time request queries, then explicit-turn aggregation.
   The UI distinguishes request detail, turn detail and aggregate-only history.

## Acceptance gates

- Fresh schema and upgrade from its immediate predecessor.
- Crash/restart between each write stage leaves no cursor/data discrepancy.
- Ordinary ingest, late historic ingest, replay, compaction, source deletion,
  reassignment and counter conflicts preserve identity and token dimensions.
- Same turn with multiple requests remains multiple requests and one turn.
- Missing turn IDs are not inferred from timestamps or file identity.
- Exact partial-hour/day queries reconcile with persisted request evidence.
- Measure bytes/request, write throughput and paged query latency with a large
  synthetic fixture before setting retention budgets.
- Shadow source selection separately; more detailed storage does not by itself
  validate the existing maximum-per-thread/day source-selection policy.

Until these gates pass, the product must not claim complete lifetime per-turn
history. This design does not authorize recovery of deleted source material or
replacement of the installed ledger.
