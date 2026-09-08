# Exact-match review transfer

`prepare-review-transfer` builds a new candidate from an ordinary ledger and an
explicitly selected, checkpointed review shadow. Both input SHA-256 values are
required. Neither input is overwritten, and an existing output is refused.

The candidate starts as a consistent copy of the ordinary ledger, retaining its
account, quota, catalog and other state. Existing raw request evidence is filled
into its retained projection; shadow-only sampling observations are not imported.

Reconstruction replacement/removal requires equality of every current original
fact column with an archived pre-correction row. Existing provenance must also
match a known old or reviewed key. New or modified original rows outside those
matches remain untouched. Original affected rows and keys are archived inside
the candidate. Only reviewed final rows for eligible original IDs are inserted;
new review candidates are never imported by this operation.

Sampling keys are copied only for existing retained facts matching identity,
hash, quality, time, model and all Token components; conflicting existing keys
are not overwritten. A receipt records row conservation and input fingerprints.
Input fingerprints are checked again before commit. Failures may leave an
uninstalled candidate file for diagnosis, but never replace an input.

Candidate creation is not installation or accounting-policy promotion. Derived
union staging, candidate validation and the recoverable installed-ledger switch
remain separate steps. The original ledger must remain available for rollback.
