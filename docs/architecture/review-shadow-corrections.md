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
transaction's snapshot before application. By default, plans containing new
historical facts are rejected. Account/project/thread identity changes always
remain rejected for separate review.
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

## Existing-fact-only application

`apply-shadow-correction --existing-only` explicitly permits a mixed draft while
applying only its existing-fact corrections. New candidates are retained in
`review_deferred_reconstruction` as sealed-plan-associated proposed facts, not
inserted into reconstruction, sampling, source keys or usage rollups. They do not
enter reported usage. This includes any absent candidates, not an assumption that
every absent fact must be chronologically after the old snapshot.

`review_correction_modes` retains the chosen policy. The additive CLI receipt
field `deferredNewRecords` reports the withheld count. Mode and deferred contents
join the post-image hash; reapplication rejects policy changes or modified/deleted
deferred facts. Old receipts without these extension rows preserve their original
digest and strict mode. Extensions are review-artifact tables only, created in
the same transaction; the normal ledger schema does not change. Older code may
reject the extended receipt's hash rather than accepting it without verification.

Both modes still verify every expected old fact and every expected absence from
the complete draft. Deferred candidates remain available for separate insertion
review; this mode is not permission to discard, confirm, relabel or import them.
Injected correction failure rolls back extensions, deferred candidates, archives,
rollups and receipts together. Tests cover exact retained totals, rejection in
default mode, repeated application, mode mismatch and deferred-content tampering.

The expected digest is a binding, not a signature or code-owner approval. Manifest
stability describes the captured source snapshot; application revalidates ledger
facts, not the current contents of an actively growing rollout. A newer unstable
manifest must not be force-applied. An earlier valid snapshot can be examined as
that snapshot, without absorbing subsequently appended records into its claim.

This path does not switch the installed application, repair sampling associations,
make every union group resolvable, or establish official-account parity. Production
adoption still requires code-owner review, complete scope/coverage acceptance and
the populated native application checks in the active goal.

## Version 2: sampling occurrence links

`link-shadow-sampling` adds association evidence to a corrected shadow:

```sh
codex-usage-ledger link-shadow-sampling --db /work/review-shadow.sqlite3 \
  --codex-home /work/synthetic-codex --manifest /work/correction.jsonl \
  --expected-sha256 VERIFIED_BODY_DIGEST \
  --start 2026-01-01T00:00:00Z --end 2026-02-01T00:00:00Z
```

It first checks the correction receipt's post-image, requalifies retained sampling
anchors, then rechecks under an immediate transaction. The original primary-log
namespace must have a unique saved machine binding matching the correction.
Only confirmed, amount-matching anchors qualify. Each proposed offset/digest must
reproduce the exact key of an occurrence covered by the correction receipt;
amounts and the 250 ms time relation are checked again. Existing conflicting keys
are refused, never overwritten. Unknown, changed and out-of-scope proposals remain
unmodified and are counted separately.

The observed file identity must equal the captured manifest binding. A growing
file may supply historical matches only when each occurrence matches the frozen
corrected key and amount; this is not proof that its entire current prefix is
unchanged. Non-append changes require review. The additive `sourceExtended` audit
field distinguishes observed growth from other metadata changes.

The transaction upgrades only this review artifact from version 1 to 2 and creates
`review_sampling_links`, preserving the previous key, retained fact hash and
correction binding. Sampling amounts, quality, assignments and original hashes
are not rewritten. Repeated links must still match their receipts. Failure rolls
back both the version upgrade and every association. Reconstruction correction
readers accept review versions 1 and 2; the normal ledger schema remains unchanged.

Account/project/model disagreements are reported, not reconciled by relabeling.
The union planner retains those conflicts as unresolved. Matching consumption
with differing write-coverage knowledge still follows the separately versioned
union coverage policy. A successful linkage does not certify unreviewed sources
or make global projection completion a scope-specific completeness guarantee.
