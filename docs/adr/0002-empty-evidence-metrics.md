# Empty evidence metrics

Status: accepted for the unreleased visualization branch; release review pending.
Date: 2026-09-05.

## Context and decision

Isolated empty-ledger browser acceptance showed 100% request matching and zero
token cards despite unavailable sources. The summary hard-coded empty matching
to one and empty input cache rate to zero. Those are undefined ratios.

Return required nullable `matchRate`, `cacheRate` and `averagePerDay` fields.
Matching is null with no evidence events; a nonempty unknown-only sample is 0%.
Cache rate is null with no input tokens, including a recorded zero-token event.
The local resolved metric is unknown/null with no confirmed events; one or more
confirmed zero-token events produce a genuine zero-valued local sample. Local
average is null without confirmed events. Quality numerator and denominator use
the same selected-window aggregation method.

Keep additive token/count structures unchanged for conservation calculations.
Presentation must consult evidence counts/resolved status rather than interpreting
an empty additive sum as measured usage. Summary and recent-activity token cards
show a dash without confirmed records. Confirmed request count remains a count
(including zero). A zero-event composition shows no recorded composition.

## Alternatives, compatibility and verification

Reject reporting 100% or 0% for an empty denominator, and reject replacing all
zero token values with unknown (which loses real recorded zeros). This widens
three required numeric response fields to nullable and needs a paired backend/
Web release; old strict clients are incompatible. No persisted facts or schema
migrations change. Synthetic tests distinguish empty, unknown-only and confirmed
zero states and filtered scopes. Rendered bilingual checks and isolated browser
acceptance complement accounting tests; full installed-app acceptance remains
separate. Missing history is not repaired by this presentation correction.
