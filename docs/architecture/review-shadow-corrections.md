# Applying reconstruction corrections in an isolated review shadow

The review execution path is deliberately separate from installation and
production-policy promotion. It makes the correction executable against a copy
while preserving both the source ledger and the old rows inside that copy.

```sh
codex-usage-ledger create-review-shadow --db /work/baseline.sqlite3 \
  --output /work/review-shadow.sqlite3
codex-usage-ledger apply-shadow-correction --db /work/review-shadow.sqlite3 \
  --manifest /work/correction.jsonl --expected-sha256 VERIFIED_BODY_DIGEST
```

`create-review-shadow` requires an existing current-schema ordinary ledger and
reserves a new private output; it never overwrites an existing file. The source
handle remains SQLite READ_ONLY. SQLite `VACUUM INTO` creates the separate
consistent copy; tests explicitly cover sparse rowid checkpoint preservation.
The copy is marked with application ID `CULS` and review-artifact version 1.
It retains the normal ledger schema version and adds review-only receipt/archive
tables. No published ledger migration or HTTP response schema is changed.

## Transaction and review boundary

Application requires the generated shadow marker, current ledger schema, the
explicit expected manifest body digest and a complete stable captured-source
manifest. The writable handle is opened without creation or migration, then its
marker/schema are checked again under an immediate transaction.

The manifest verifier checks every old fact or expected absence against that
transaction's snapshot before application. Plans containing new historical facts
or changes to account/project/thread identity are rejected for separate review.
The existing path supports reviewed quantity/model/time/coverage corrections and
suppression of inherited replay records; it does not authorize insertion or
cross-account reassignment by convenience.

One transaction performs:

1. Stage and validate the entire sealed plan.
2. Archive complete original reconstruction rows and existing source keys under
   the manifest digest.
3. Suppress rejected occurrences and persist corrected occurrences through the
   normal hashing/source-key path, preserving original ancillary metadata.
4. Rebuild reconstruction day/hour rollups. Insert triggers alone are insufficient
   because corrections can delete old contributions.
5. Save counts and a typed, framed digest of the complete affected post-image,
   including source keys, then commit.

Failures roll back archives, source facts, derived rollups and receipts together.
Repeating the same plan verifies the post-image before returning `already_applied`;
later edits are not silently accepted just because a receipt exists. Original
rows remain queryable in `review_old_reconstruction` and keys in
`review_old_record_keys`; the untouched baseline provides another copy.

## Evidence and limits

Synthetic tests cover normal-ledger refusal, existing-output refusal, wrong seals,
rollback after an injected delete failure, post-image changes, rowid preservation,
component/dimension conservation, source-key restoration, union-preview reads and
repeat-application idempotence.

The expected digest is a binding, not a signature or code-owner approval. Manifest
stability describes the captured source snapshot; application revalidates ledger
facts, not the current contents of an actively growing rollout. A newer unstable
manifest must not be force-applied. An earlier valid snapshot can be examined as
that snapshot, without absorbing subsequently appended records into its claim.

This path does not switch the installed application, repair sampling associations,
make every union group resolvable, or establish official-account parity. Production
adoption still requires code-owner review, complete scope/coverage acceptance and
the populated native application checks in the active goal.
