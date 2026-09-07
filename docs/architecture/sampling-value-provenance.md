# Sampling anchors and rollout quantities

The current post-sampling importer does **not** parse a second independent set
of input/cache/output amounts from the sampling log. Its `Observation` contains
a log identity, timestamp, thread, turn and optional model. Numeric `TokenUsage`
comes from associated rollout usage records, not the sampling log. The log's
`total_usage_tokens` and compaction/window fields are not imported as consumption.

Consequently, retained-sampling and reconstruction amounts may agree because
they descend from the same numeric record. Their agreement alone is not an
independent measurement cross-check or proof of separate model calls. A source
record key identifies a local row/version; a sampling receipt identifies a local
log occurrence. Neither is an upstream response ID. Keep these distinctions in
all interpretation of the [union candidate](source-union-shadow.md).

## Association policy

New ingestion uses `mutual_unique_nearest_v2` within each supplied mature
thread cohort. After sorting timestamps, associate only when each side is the
other's unique nearest neighbor within an inclusive 250 ms. Equal-distance or
duplicate-timestamp ties are ambiguous. If a candidate prefers another anchor,
the unmatched anchor remains unknown: it cannot consume a farther leftover
record merely because its nearest record was already used.

The five standard integer fields (input, cache read, output, reasoning and total)
must be present and valid. Null/partial/malformed usage is not a measured zero.
Missing or null optional cache-write detail remains unknown. An invalid nearest
snapshot remains an unavailable candidate, so filtering it out cannot force a
match to an older amount. Only explicit `event_msg`/`token_count` records qualify.

For example, anchors at 0/20 ms and candidate rows at 15/150 ms previously
allowed a greedy 0→15, 20→150 assignment. The new rule associates 20→15 only;
the first anchor stays unknown, without a candidate link or recorded amount.
The existing `unique_nearest_timestamp` link label remains true for the stricter
subset; it is not retroactive evidence that older links met the mutual rule.
The current policy is recorded additively in the sampling cursor metadata.

Lookup uses binary search and exact wide-integer nanoseconds, avoiding a growing
scan through all candidates and saturation of remote timestamps into one value.
Observation/candidate sorting and existing maturity/cursor boundaries remain.
This policy does not prove completeness outside the supplied cohort or recover
deleted source rows. Historical retained amounts and links are not automatically
repaired or certified by updating the importer.

## Shared numeric counter normalization

Sampling candidates and reconstruction now call the same private numeric
normalizer (`src/counter.rs`). A valid increasing cumulative counter supplies
the increment, even if `last_token_usage` is stale. Equal consumed components
supply no new amount. Cache-write observation weight is metadata: its changes
cannot manufacture a new request/reset, and incomparable weights become unknown.
Initial identifiable last usage and counter-reset fallback retain reconstruction's
existing policy; they do not prove an upstream request identity.

An unchanged or invalid nearest candidate remains a blocker, with an explicit
unknown reason and no numeric/candidate link; it cannot force association with an
older row. This withholds the sampling anchor, not a confirmed zero-token call.
The reconstruction occurrence can still represent the original increment.

Candidate cursor JSON version 2 retains the cumulative baseline and continuity
mode alongside the byte position. Existing version-1/unreadable checkpoints at
a nonzero offset establish a fresh baseline without re-reading or rewriting old
facts. Their first cumulative snapshot is not attributed as a new amount.
Last-only records remain supported for a whole stream observed from its start;
after cumulative mode begins, missing/invalid totals break continuity rather than
bridging the gap into a later timestamp. A restart restores the same numeric state.
Each supplied sampling-source cohort, its log cursor, and its candidate counter
cursors commit in one transaction. A failed candidate-cursor write rolls back
the facts and log cursor too. This is not yet a bounded-history scan/latency claim.
No SQL schema or public response field changes. Old confirmed facts/associations
are not retroactively corrected or relabeled.

**Stream/replay normalization remains unfinished.** Sampling still lacks the
reconstruction adapter's canonical/foreign-history boundary state. Legacy
last-only evidence, reset/rollback identification, cross-poll association coverage
and unresolved anchors still require eligibility review before union promotion.
A local row/version key does not prove distinct consumption. Shared arithmetic
alone does not close those semantic gaps or authorize the production switch.

## Evidence and next gate

Synthetic ingestion verifies that an exaggerated context counter is not used
as Token quantity, the ambiguous anchor receives unknown quality, only the
mutual candidate's amount is added, and a second unchanged scan reads no bytes.
Duplicate timestamps, reverse ties, tolerance edges and wide timestamp ranges
are covered alongside existing copied-source and append-only tests.
Regression also covers repeated counters within/across imports, durable restart,
stale last amounts, broken cumulative continuity, legacy offset-only checkpoints,
coverage-only changes and cursor-write rollback/retry. A shared-origin synthetic
stream has equal sampling/reconstruction component vectors and conserves account,
project, model, thread and calendar aggregates despite a stale last snapshot.

Legacy association enrichment must use source observations and actual rollout
positions with frozen-value validation; equality of copied numbers is not a
replacement for that provenance. A policy disagreement requires a reviewed
correction, not silently attaching a key or changing historical amounts.
