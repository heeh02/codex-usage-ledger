# Dashboard startup and retention

Opening `serve` or starting `daemon` does not call raw-event compaction. They may still
prepare missing derived rollups/request detail, synchronize the local directory
and refresh account metadata through existing paths; it is not a byte-read-only
audit command. The change specifically separates dashboard access from deleting
historical raw details. Explicit maintenance retains its normal compaction
guards and behavior. Starting live collection alone does not authorize deletion.

Active request-union maintenance drains ingestion bursts rather than processing
only one small batch per collection tick. Each transaction retains the existing
1,000-group / 10,000-member limits. A maintenance call runs at most 32 batches,
checking a two-second scheduling budget between batches, and stops immediately
when the projection is ready. One transaction can exceed the scheduling budget;
this is not a hard deadline. Pending work remains durable and keeps the normal
snapshot-readiness guard; maintenance never substitutes legacy totals.

A retained/raw conflict can block deletion even if the top-level rollup has been
verified. The error is now `RetainedEvidenceMismatch`, rather than the misleading
`RollupNotVerified`. It means the selected deletion batch rolled back; earlier
successfully committed maintenance batches are not implicitly rolled back.
Neither equal total token values nor a failed dashboard startup is authority to
overwrite hashes, drop mismatch predicates, or delete retained observations.

The HTTP-process regression constructs an old raw event with a deliberately
different retained hash, starts a real serve subprocess with an empty source
home, and waits for the dashboard's idle state. Raw rows, compacted keys and the
conflicting retained hash remain unchanged. Separate store tests confirm that
explicit compaction still fails and rolls back for hash or token-field conflicts.

This does not resolve historical hash provenance, verify independent inference
usage, or guarantee that other startup preparation failures are nonfatal. Such
integrity failures still require a documented reconciliation/migration receipt.

The request-backfill rehash mechanism and read-only provenance diagnostic are
documented in [retained hash provenance](retained-hash-provenance.md). Its forward
fix does not silently normalize old mismatches.
