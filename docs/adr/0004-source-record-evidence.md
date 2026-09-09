# Shared local source-record evidence

Status: accepted for the unreleased branch; accounting/migration review pending.
Date: 2026-09-05.

## Decision

Timestamp proximity and equal token amounts do not establish independent request
identity. However, the sampling adapter obtains its token dimensions from a
specific rollout record that reconstruction can also interpret. Retain a common
local measurement key formed from machine, physical file identity, canonical
thread, byte offset and a SHA-256 digest of parsed JSON content.

Schema 34 adds `source_record_evidence`, keyed separately by evidence side and
event ID. Only digests are retained, never raw source lines. Both adapters emit
keys for newly observed records. Supplemental keys do not change existing event
IDs or dedup hashes. Missing old keys stay absent; migration does not guess or
backfill them. Conflicting keys for one stored event fail transactionally.

Reject automatically treating a candidate timestamp match as proof, changing
legacy IDs and recounting them, or adding both source totals. Matching keys
establish a shared local source occurrence, not server request equality,
non-replayed history or account billing. A future union still requires component
agreement, source coverage/replay checks, shadow comparison and a migration
receipt. This change does not replace `max_thread_day_v1`.

## Validation and compatibility

Tests cover genuine schema-33 upgrade without invented keys; unchanged legacy
hashes and totals during enrichment; conflict rollback; retained evidence after
compaction; and two adapters reading the same synthetic file. The compaction
test also reproduced ordinary replay reinsertion; that path now checks compacted
keys before inserting. An identical replay stays unchanged, conflicting immutable
data fails, and no historical duplicate cleanup is performed. Deploy with the
new binary only after the usual migration review; older binaries cannot open
schema 34 safely. Installed-app and real-ledger acceptance remain separate.
