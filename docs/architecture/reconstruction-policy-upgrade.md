# Resumable reconstruction parser-state requalification

The parser policy attached to a persisted checkpoint is distinct from the
policy that produced historical usage rows. A new binary must not blindly reuse
an older checkpoint's live/replay state: it may already have escaped inherited
history under a superseded rule.

## Trigger and boundaries

New reconstruction checkpoints record `reconstruction_shared_stream_v2` in
`parser_policy`. A structurally valid checkpoint with an absent policy or the
older `reconstruction_uuid7_strict_v1` policy is lazily requalified before its
next ingestion work. Unchanged inactive files do not trigger a startup-wide
rescan. Unsupported policy/state or source identity requires review; device drift
is not automatically rebound. The ordinary missing/corrupt-checkpoint refusal
still applies.

Requalification reads only the already-committed prefix. The boundary is the old
byte cursor, or the start of its uncommitted partial record. Each call reads at
most the existing complete-record-sized reconstruction slice (64 MiB + 1 byte).
It runs the shared parser but discards event proposals: the prefix is not new
consumption and no historical usage is inserted, deleted or relabeled.
Controlled installation must stop the old collector before enabling the new
one. This is not a supported rolling upgrade with an older binary continuing
to write, nor permission to downgrade and ignore the new policy field.

## Durable state and atomic completion

A separate `reconstruction-policy-upgrade-v2:<thread>` cursor stores the current
prefix position, new parser state, target boundary/line count, frozen model/cwd
hints, parent identity, source metadata and a framed hash chain. The old main
cursor stays unchanged during progress. Partial bytes are rewound and excluded
from this progress checkpoint, preventing large serialized partial-line buffers.
Restart continues from the last complete record rather than byte zero.

Active upgrade cursor IDs keep the job scheduled even without additional file
growth. Healthy upgrade progress has no `last_error`; it is not reported as a
source failure. Completion removes the working parser from the side payload and
retains a compact receipt. The main checkpoint changes policy/state at the same
byte/line boundary. An old partial record remains uncommitted and is handled by
normal tailing when it can be completed.

Each progress/final transaction compares the accounting-relevant main and side
cursor fields with those read by the worker. Concurrent changes cause refusal,
not overwrite. Finalization also refuses if existing same-source/file usage
already extends into the resume range, which requires historical review instead.
Side receipt, main replacement and source status commit together; injected write
failure rolls them all back. No SQL schema or public API field migration is needed.

Identity and size/modification checks run across slices and around the opened
reader. Parent identity must remain consistent. These checks assume normal
append-only source behavior; they do not detect every rewrite-plus-append or
prove an immutable historical prefix. The hash chain is a framed-record review
artifact, not a full-file SHA-256, signature or server receipt.

## What this does not certify

The receipt explicitly preserves historical facts unchanged. It qualifies future
resumption only; it does not repair old overcounts, prove deleted-source coverage,
authorize source rebinding, or turn old rows into current-policy evidence.
Historical correction and [source union](source-union-projection.md) promotion
remain separate reviewed work. Completion of this upgrade must not be treated as
completion of account-wide calibration or installed-app acceptance.

Synthetic tests cover a previously wrong live child state, unchanged historical
fact/hash/components, bounded slices and restart, progress scheduling at idle
EOF, no repeated upgrade after completion, partial records, concurrent cursor
changes, source metadata change, transactional rollback, overlapping history,
unsupported future policies and all five aggregate dimensions after resumption.
