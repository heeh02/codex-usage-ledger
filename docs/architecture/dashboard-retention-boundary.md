# Dashboard startup and retention

Opening `serve` no longer calls automatic raw-event compaction. It may still
prepare missing derived rollups/request detail, synchronize the local directory
and refresh account metadata through existing paths; it is not a byte-read-only
audit command. The change specifically separates dashboard access from deleting
historical raw details. Explicit maintenance and collection retain their normal
compaction guards and behavior.

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
