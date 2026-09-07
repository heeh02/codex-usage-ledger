# Review-only reconstruction correction drafts

This is a durable per-record artifact from the
[streaming source comparison](reconstruction-file-audit.md), not an apply command.
It prepares historical correction review without altering old facts, assignments,
source bindings, rollups, or the production source-selection policy.

```sh
codex-usage-ledger draft-reconstruction-correction \
  --db ./synthetic-ledger.sqlite3 --codex-home ./synthetic-codex \
  --thread synthetic-root --output ./correction-draft.jsonl

codex-usage-ledger verify-reconstruction-correction \
  --manifest ./correction-draft.jsonl

codex-usage-ledger verify-reconstruction-correction \
  --manifest ./correction-draft.jsonl --against-db ./synthetic-ledger.sqlite3
```

The generator uses the same read-only schema-35–38 source audit and one ledger
snapshot. It streams each Token position directly to the draft while scanning;
it does not accumulate the full report in memory or reread the source for export.
Default source budgets are 1 GiB / one million Token rows. Audit limits still
apply; an interrupted or bounded scan is not complete historical coverage.

## Private, non-overwriting artifact

The output parent must exist and resolve outside the explicit Codex home. The
file is created exclusively (`create_new`), mode 0600 on Unix. Existing files or
symlinks are never overwritten. It contains private account/project/model/thread
identifiers and usage, but no raw conversation messages/instructions or auth data.
Do not commit it or publish it as a screenshot/report artifact.

Each JSONL line is bounded to 128 KiB and the file to 2 GiB. Verification reads
through a bounded buffer and permits at most ten million measurement entries.
An export failure can leave a partial file; it is retained rather than silently
deleted or overwritten. A retry must use a new destination. Flush and file sync
precede a successful export return; integrity verification then rereads the draft,
not the potentially changing original rollout.

## Format version 1

Exactly one header, ordered measurement records, one completion, then one seal:

- `header`: schema, machine/source/thread, stored and observed file identities,
  initial file length, creation time and explicit parser policy
  `reconstruction_uuid7_strict_v1`. Semantic parser changes require a new policy
  identifier and review; this is not a claim about an installed app version.
- `record`: strictly increasing source byte offset, source-record JSON digest,
  comparison action, suppression reason if applicable, and nullable stored and
  proposed facts. Old hashes and all Token fields are retained. Proposed source
  keys are bound to the header namespace, byte position and JSON digest.
- `completion`: visited/expected old-position counts, Token-record count, EOF,
  canonical/malformed/source-change state and framed processed-record digest.
- `seal`: SHA-256 of the exact header/record/completion bytes including newlines,
  plus measurement count. There may be no bytes after the seal.

The verifier checks order, lengths, counts, byte positions, event/key identity,
Token invariants, action consistency, allowed suppression reasons and the seal.
It recomputes category counts and all stored/proposed Token components. A missing
side stays null. Old and proposed amounts are alternatives, never additive usage.
The seal includes completion metadata, not just measurement lines.

A checksum is **not a signature or approval**. Someone able to rewrite a draft
can recompute its checksum. Even an internally consistent artifact is only a
review input and must not be trusted as independent inference/source proof.

## Optional old-ledger revalidation

`--against-db` opens a compatible evidence ledger read-only and uses one snapshot
while checking the sealed stream. It verifies the old source binding, complete
source row count, and every represented old fact or expected absence. Comparison
includes the stored hash **and** actual time/model/account/project/usage/key
fields; a metadata projection changing while an ingestion hash remains the same
must invalidate the draft. A missing or changed row fails rather than being
silently relabeled. This step does not rescan source files or re-run parsing.

`ledgerRowsRevalidated=true` concerns only the facts represented by the draft.
`fullSourceScan` separately records whether the original scan reached stable EOF
with canonical metadata, no malformed records and every stored position seen.
A bounded scan may be sealed and structurally valid while this flag is false.
Neither flag proves identity continuity, all-account/project coverage, replay
freedom, or that the source has remained unchanged since export.

Every result has `draftOnly=true`, `migrationAuthorized=false`, and
`sourceRevalidated=false`. No apply command, automatic startup repair, production
selector switch or installed-app change is included in this feature.

## Acceptance and promotion gates

Tests cover full/partial source exports, read-only source/index/ledger behavior,
Unix permissions, existing-output refusal, checksum/order/count/identity/action
conflicts, duplicate positions, truncation, data after the seal, large lines,
all-component conservation and stale metadata despite unchanged hashes. CLI
export, verification and ledger-revalidation paths use synthetic fixtures.

Before promotion, review record-level replay/ancestral and source-identity evidence,
revalidate the proposed source mapping, resolve cache-field-presence differences,
stage a reversible correction projection, and reconcile all time/account/model/
project/session dimensions with the source union. Code-owner review and controlled
migration receipts remain required. A candidate suppression sum cannot simply be
subtracted from production totals because the other source may overlap it.
