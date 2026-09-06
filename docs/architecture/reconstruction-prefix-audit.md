# Bounded reconstruction preview and identity review

`audit-reconstruction` compares a prefix of a currently indexed rollout with
stored reconstruction facts. It does not run the importer or change the ledger.

```sh
codex-usage-ledger audit-reconstruction --db ./synthetic-ledger.sqlite3 \
  --codex-home ./synthetic-codex --thread synthetic-root \
  --max-bytes 4194304 --limit 100
```

The ledger must already exist at the supported schema. The native index is
opened read-only, and the selected canonical rollout must remain within the
explicit home's sessions/archived-sessions roots after path resolution.
Different canonical metadata is rejected. No auth discovery or credential
access is performed. Maximum prefix size is 16 MiB; token-record limit is 500.
Reaching either limit is partial work, not completion or a zero usage result.

The version-1 report includes processed/read counts, canonical detection, end of
file/partial-line state, a captured-prefix digest and a metadata-change check.
Each token record can show a stored fact/hash and proposed fact, plus whether a
stored source-record key matches. Fact comparisons include time, thread, model,
account, project and all token fields, excluding the transient import time.
Missing old keys stay unknown. Counter-prefix bookkeeping is separate and must
not be added to either usage total. Output contains private scope metadata and
is not an anonymous sharing artifact.

Ledger reads use one read snapshot without selector refresh or migration. The
source file is not frozen: metadata checks do not defeat a hostile same-user
rewrite or prove full historical continuity. Only matching event positions in
the bounded prefix are compared; shifted/missing old positions require another
audit. `migrationReady` and `historyComplete` are always false. This preview is
evidence for a later reviewed migration, never a deletion authorization/receipt.

## Unix device-number drift

By default the stored and current physical identities must match exactly. An
explicit `--allow-device-drift` allows a diagnostic candidate only when both
identities have valid Unix device/inode shapes, inode is equal/nonzero and device
differs. It uses the old event namespace for comparison and labels the relation
`unix_device_changed_same_inode_candidate`. A matching inode alone is not proof
of identity; this option never updates a binding, cursor, source or event.

## Collector protection

The automatic reconstruction collector previously treated any identity-string
change as replacement and called the destructive replacement primitive. That
could delete retained facts and restart full backfill without proof that the
file had actually been replaced or that the new file retained complete history.

Identity changes now preserve events, rollups, old identity and checkpoints.
Affected reconstruction sources enter the existing unavailable state with
`source_identity_verification_required`; its checkpoint is exempt from generic
unrecoverable-cursor cleanup. Repeated observations do not reset progress or
rewrite source status unnecessarily. No inode-only automatic rebind exists.
The CLI report exposes `identityReviewSources`, and collection status uses
`reconstruction_identity_review` so the bilingual UI explains that verification,
not repeated refresh, is required. Other independently valid collectors can
continue. The explicit replacement primitive remains for receipt-reviewed
maintenance, not automatic identity handling.

Synthetic tests cover legacy/proposed differences, byte and row limits, canonical
mismatch, root containment, opt-in device drift, read-only source/index/ledger
bytes, and preservation through repeated drift or actual file replacement.
Continuity verification/rebinding and historical correction remain separate
work; no installed application or live ledger acceptance is implied.
