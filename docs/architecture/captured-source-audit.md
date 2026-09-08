# Captured-prefix reconstruction audit

Status: implemented audit/draft mechanism; not a production migration approval.

Active rollout files append while an audit runs. Treating every mtime/size change
as invalidation prevents historical verification even when all historical bytes
are unchanged. The audit now captures a bounded, line-complete prefix through one
open source handle. Later appends are outside that reader and are never parsed as
part of the captured snapshot.

## Proof and limits

1. Bind the file's physical identity and starting size to the indexed source.
2. Find the last complete newline within the configured byte budget. Oversized
   unfinished lines fail explicitly; partial lines are not parsed as complete.
3. Hash the captured raw prefix before parsing, with bounded memory.
4. Read sequentially to the fixed cutoff through the existing JSONL tailer and
   parser. Hash the exact bytes read, not reserialized JSON, and refuse short reads.
5. Reopen the path, recheck identity/length and hash the prefix again. The initial,
   parsed-stream and final hashes must agree. A replaced path, truncated file,
   modified prefix or incomplete read cannot qualify.

An append may change size/mtime without invalidating this proof. The diagnostic
still reports `sourceChangedDuringRead`; it separately reports
`capturedPrefixBytes` and `capturedPrefixRevalidated`. This is evidence about the
captured interval, not a guarantee that a concurrent writer can never modify it
after verification. It does not prove complete inference history.

Byte/Token-record limits and trailing incomplete lines still withhold complete
coverage. `reachedFileEnd` refers to the initial file boundary, not the larger
file at the end of an append. Every stored source position must still be seen;
malformed or noncanonical sources cannot become complete merely through hashing.

## Draft compatibility

New drafts use header version 2 and retain the existing parser-policy identifier.
They add `capturedPrefixBytes`, `capturedPrefixSha256` and completion field
`capturedPrefixRevalidated`. The seal covers these fields. Structural verification
requires consistent version/proof fields and rejects records outside the captured
boundary. `fullSourceScan` for version 2 is completeness of that captured initial
file plus all stored positions, conditional on successful prefix verification.

Version-1 manifests retain the original unchanged-source requirement and remain
readable. A version-1 manifest cannot carry a version-2 proof. Older software will
reject new draft versions rather than silently weaken its interpretation. No
persisted ledger or HTTP schema changes. Structural verification continues to
report source revalidation/authorization separately: a sealed document is not
independent permission to migrate data.

Shadow apply still rejects new historical insertions and identity reassignment.
Later rows absent from a ledger snapshot may appear as new candidates in a
captured draft; resolving how to retain them separately from a correction remains
an explicit migration step, not an automatic import. The subsequent
[existing-fact-only mode](review-shadow-corrections.md#existing-fact-only-application)
can retain new candidates separately while correcting existing facts.

Tests cover append exclusion through the actual audit sink, prefix modification,
truncation, replacement, byte and record limits, unfinished tails, legacy drafts,
proof/version mismatch, out-of-bound records and failed revalidation. The
existing accounting and shadow tests remain required.
