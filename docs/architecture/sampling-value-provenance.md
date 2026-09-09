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

New ingestion uses `durable_mutual_window_v3`, retaining the mutual-nearest
predicate from `mutual_unique_nearest_v2`. After sorting timestamps, associate only when each side is the
other's unique nearest neighbor within an inclusive 250 ms. Equal-distance or
duplicate-timestamp ties are ambiguous. If a candidate prefers another anchor,
the unmatched anchor remains unknown: it cannot consume a farther leftover
record merely because its nearest record was already used.

The five-second maturity cutoff is shared across a polling tick. Closing an
anchor at time A needs candidate neighbors through A + 250 ms and reverse-anchor
context through A + 500 ms. Look-ahead observations participate in matching but
do not advance the log cursor: only a contiguous mature log-ID prefix commits.
This also preserves existing non-monotonic-ID/time watermark behavior.

Candidate checkpoint version 4 retains normalized read-ahead rows, claimed flags,
the latest finalized anchor timestamp (up to two copies for ties), and file
metadata. A later poll/restart can use those rows without rescanning source JSON.
Candidate reading stops at the required observation horizon, not all available
history. For a future ordered anchor later than T, a candidate at/before the
latest finalized anchor T cannot win the reverse-nearest check against T;
therefore those candidates can be discarded from the window. The original
durable facts are not discarded. Window facts/cursors still commit atomically.

Previously claimed candidates cannot be reused when a closer anchor arrives
late. Observations at/before a prior finalized anchor are retained as unknown,
not used to rewrite prior confirmed associations. This is ordered incremental
matching, not a guarantee of arbitrary late-arrival recovery. Legacy checkpoints
without saved context retain an explicit overlap guard where a prior timestamp
is available; their old associations are not retrospectively certified.

The candidate window is capped at 10,000 records per thread/source; overflow
rejects the cohort before any cursor/fact commit rather than silently truncating
evidence. Append-compatible metadata is checked before/after reuse/read.
Same-size modification, shrink while resuming, or observed in-read replacement
requires review instead of reusing the old window. These metadata checks do not
prove immutable prefixes against undetected rewrite-plus-append operations.
The window cap does not claim a bounded total bootstrap scan or cohort size.

The shared source parser requires five unsigned integer fields (input, cache
read, output, reasoning and total); unsigned numeric strings remain supported.
It never fabricates missing fields or derives an absent declared total. Complete
but non-conserving records remain available to the diagnostic guard for quarantine;
sampling and reconstruction accept only invariant-valid quantities.
Null/partial/malformed usage is not a measured zero. Optional cache-write aliases
`cache_write_input_tokens`, `cache_write_tokens` and `input_cache_write_tokens`
have the same semantics: null/absent is unknown, consistent non-null aliases are
one field, conflicting aliases invalidate the snapshot. An invalid nearest
snapshot remains an unavailable candidate, so filtering it out cannot force a
match to an older amount. Only explicit `event_msg`/`token_count` records qualify.
Null/absent `info` is a metadata-only notification, not a numeric candidate or
counter reset; existing quota normalization remains independent. A non-null,
incomplete usage payload is different and is not silently ignored as metadata.

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

Candidate cursor JSON version 4 retains the cumulative baseline, continuity
mode and stream-boundary state alongside the byte position. Existing version-1/unreadable checkpoints at
a nonzero offset establish a fresh baseline without re-reading or rewriting old
facts. Their first cumulative snapshot is not attributed as a new amount.
Last-only records remain supported for a whole stream observed from its start;
after cumulative mode begins, missing/invalid totals break continuity rather than
bridging the gap into a later timestamp. A restart restores the same numeric state.
Reconstruction now persists an additive `counter_continuity_lost` checkpoint bit;
missing/invalid cumulative snapshots and malformed JSON cannot transfer gap usage
to the next dated/model/account observation. That next valid counter establishes
a baseline only. A valid counter without a timestamp likewise advances only the
baseline, not dated consumption. Skipped non-usage JSON is validated without
allocating prompt trees, and first-line UTF-8 BOM handling matches reconstruction.
Each supplied sampling-source cohort, its log cursor, and its candidate counter
cursors commit in one transaction. A failed candidate-cursor write rolls back
the facts and log cursor too. This is not yet a bounded-history scan/latency claim.
No SQL schema or public response field changes. Old confirmed facts/associations
are not retroactively corrected or relabeled.

## Shared stream-boundary decisions

Both adapters now delegate canonical session, foreign replay, initial child
prefix and live transitions to `src/stream_boundary.rs`. Replay token rows can
advance a baseline but cannot emit consumption. A foreign segment does not end
merely because timestamps have a long gap or the child's metadata reappears.
An eligible own-task start exits both foreign replay and the initial child prefix;
this fixes suppression of a real first child sample within 2 seconds of creation.
Already-live task starts do not reset the current model context.

The own-task predicate retains the existing strict UUIDv7/time-field rules; a
UUIDv4's random leading bits are not a task clock. The initial child-prefix gap
fallback remains a heuristic, not proof of upstream inference ownership.
Future or incomplete records do not advance boundary state. Timestamp comparison
uses parsed instants, including JSON with spaced separators, rather than lexical
RFC3339 comparisons. Candidate parsing now observes metadata/task/context records
in addition to token rows, without retaining prompt bodies in the checkpoint.

Version-2 numeric-only candidate checkpoints retain their numeric baseline but
start with unknown/protected boundary state. Pre-resume counters update only the
baseline; the later identifiable request cannot claim that intervening usage.
They need eligible task-start evidence to resume and are not silently treated as
live. A one-time, at-most-64-KiB header read can recover a matching canonical
creation timestamp, allowing the existing `started_at` predicate to work even
when the task ID is not UUIDv7. It does not infer live state or replay the whole
file. Header-attempt state is persisted with the cursor, including an unavailable
result. The existing reconstruction checkpoint field layout stays compatible.
Whole-file metadata-free root streams retain the legacy anchored path; this is
not available to indexed children or offset-only unknown histories and is not
full canonical provenance proof.

**Production promotion remains gated.** Legacy last-only/partial evidence,
reset/rollback identification, source continuity, cross-poll
association coverage and unresolved anchors still require eligibility review.
A local row/version key does not prove distinct consumption. Shared arithmetic
and boundary decisions do not certify missing history or authorize migration.

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
The same source fixture now contains foreign metadata and an inherited baseline
before its own start. Dedicated regressions cover the child's fast first sample,
foreign replay across a long gap and serialized restart, rejected UUIDv4 starts,
numeric-only checkpoint upgrade and partial/future boundary records.
Clock-controlled incremental tests exercise read-ahead across a restart with zero
new JSON bytes on the second poll, a look-ahead anchor preventing premature
attribution, claimed-candidate reuse/out-of-order refusal, source-change refusal
and capacity rollback. All five dimensions and Token components conserve across
the successful two-poll case; idle historical appends are not rejected merely
because their timestamps precede wall-clock time.
Malformed-JSON/restart ingestion keeps the gap out of all five aggregate
dimensions. A quota-only notification sharing the next sample's timestamp
neither creates a tie nor breaks continuity. Cache aliases, unsigned strings,
undated counters and first-line BOM are covered. Older synthetic fixtures now
state intended zero reasoning/cache values explicitly; missing-field tests
separately require unknown rather than weakening parser rules to pass fixtures.

These rules gate complete local evidence; they do not establish that missing
components erase an otherwise real upstream request. Historical partial records
need separate completeness/eligibility review, not deletion or measured-zero
replacement. No old confirmed row is changed by these forward parser edits.

Legacy association enrichment must use source observations and actual rollout
positions with frozen-value validation; equality of copied numbers is not a
replacement for that provenance. A policy disagreement requires a reviewed
correction, not silently attaching a key or changing historical amounts.
