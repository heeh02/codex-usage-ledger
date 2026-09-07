# Declared-fork prefix evidence and task identity

## Correct task-clock interpretation

Both live replay interpretation and reconstruction use the same task-boundary
predicate. It previously accepted the first twelve hexadecimal characters of
any identifier as a UUIDv7 clock. A UUIDv4's random prefix could therefore look
newer than a fork and prematurely release foreign-history protection, even
when its explicit task start time was old.

Time extraction now requires the whole UUID: canonical hyphenated or compact
shape, hexadecimal digits, version 7 and the RFC variant. UUIDv4, other versions,
invalid variants and partial/trailing identifiers supply no UUID clock. A task
with an eligible explicit embedded start time can still resume the stream.
Rewritten outer timestamps alone remain insufficient.

Synthetic RED/GREEN tests reproduce the random-v4 error and validate shape,
version, variant and fallback start time. A reconstruction test round-trips its
checkpoint while an old v4 task remains foreign; a later explicitly timed task
resumes with only its new delta. This is a forward interpretation repair, not
automatic historical correction. Already-consumed old checkpoints do not prove
that prior history was replay-free; do not reset or rewrite them without review.

## Read-only parent comparison

```sh
codex-usage-ledger audit-inherited-prefix --codex-home ./synthetic-codex \
  --thread synthetic-child --max-bytes 1073741824 --max-tokens 1000000
```

The command requires explicit `forked_from_id` in the child's canonical metadata
and an indexed parent. Both paths use the existing native-index/canonical-path
containment checks. No ledger, auth discovery, login change or migration occurs.
The child's initial Token-info sequence is captured up to the first eligible
canonical task boundary and compared, in order, with the parent's initial
sequence. It does not search for a convenient matching subsequence or certify
all later foreign segments. An unavailable parent or missing declaration fails.

Each source has a byte budget of at most 2 GiB (default 1 GiB); up to one million
Token fingerprints are retained (also the default). Reads are at most 4 MiB per
chunk with the shared 64 MiB line limit. Invalid/missing Token-info payloads fail
instead of silently shortening the stream. Source resets fail; ordinary metadata
changes prevent a verified-prefix flag. Child metadata is checked again after
the parent scan. These checks do not defeat hostile same-user rewrites.

`identicalDeclaredPrefix` requires a nonempty, complete same-length sequence,
an observed canonical task boundary, matching metadata identities, no observed
source changes and equality of every canonicalized `payload.info` fingerprint.
Outer timestamps and byte offsets are excluded from the comparison because
copies may rewrite them. Sequence digests are Token-info digests, not raw-file
or historical-origin hashes. Offsets remain in the report for inspection.

## Cache-write field presence is not usage proof

Re-serialized history may gain zero-valued cache-write fields that were absent
from the ancestor. Strict comparison preserves this difference. A separate
`zeroWriteDefaultCompatibleRecords` comparison omits only integer-zero values
under the recognized cache-write keys within total/last usage. Nonzero values,
other components, field types and context metadata remain significant.

`zeroWriteDefaultCompatiblePrefix` is **compatibility only**, not exact source
identity and not evidence of observed zero writes. It never changes the strict
result, cache coverage weights, or production usage. The first raw mismatch and
first remaining compatibility mismatch (with source offsets) are reported.
A final rollback snapshot or differing record cannot be silently discarded to
make the prefix pass. Inspect such cases independently against source events.

Reports always retain `migrationReady=false` and
`independentInferenceProven=false`. Prefix correspondence is one input to a
reviewed per-record correction manifest; it cannot authorize summing cumulative
counters, deleting historical facts or relabeling copied history as current
account usage. Combined with the [full-source audit](reconstruction-file-audit.md),
it helps determine which old records need correction before source-union
promotion. Real-source receipts remain private and outside the repository.
