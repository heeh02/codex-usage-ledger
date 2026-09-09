# Collector degraded state

Status: accepted for the unreleased visualization branch; release review pending.
Date: 2026-09-05.

## Context

Sampling and quota errors propagated out of the daemon loop, terminating its
HTTP service. Reconstruction errors could leave an apparently healthy status.
Neither outcome accurately represents retained usage during a source outage.

## Decision

Add `degraded` to the collection-phase wire enum. Independently attempt sampling,
quota tails and reconstruction. Their errors retain ledger facts and applicable
source checkpoints, publish source codes rather than error text, and retry on
the existing periodic schedule. A later successful pass (including no new rows)
clears degraded state. Identical status does not cause repeated status writes.
Source transactions remain responsible for atomicity; earlier completed batches
are not rolled back merely because a later source fails.

The UI explains incomplete recent collection in both languages, without showing
failed progress as zero usage, a completed refresh, or a quota reset. Initial
authentication/catalog failures, ledger corruption and HTTP task supervision
are separate lifecycle work and are not claimed fixed by this change.

## Compatibility and release boundary

This extends a closed enum and is not compatible with old strict clients.
Ship the Rust daemon and regenerated Web bundle together in the next release;
do not deploy the changed daemon behind an old dashboard. The current installed
release is untouched. No persisted-schema migration or historical usage rewrite
is needed. Accounting and release code-owner acceptance remains required.

## Alternatives and verification

Reject terminating the service on a temporary source failure or hiding failure
under `live`. Do not roll back independently committed source batches or retry
by clearing their checkpoints. Test unavailable-source retries, preserved
facts/counters, unchanged-status writes, recovery on an empty successful pass,
and bilingual rendering without raw error text or a completed progress meter.
These tests do not replace native lifecycle or full browser acceptance.
