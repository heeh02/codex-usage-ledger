# Reconstruction work scheduling

Previously, every growing already-reconstructed source ranked ahead of every
new pending source. With enough hot sources to fill the file budget on each
tick, historical sources could remain unprocessed indefinitely. A reconstructing
source caught up to an incomplete EOF could also be mistaken for active backfill.

## Separate history and live-tail service

Scheduling now separates:

- History: pending files, files behind their previously observed length, and
  active parser-policy requalification.
- Tails: sources already caught up to their previously observed length that
  have new bytes to consume, including an incomplete EOF that has since grown.

When both lanes have work and the budget is at least two, reserve tail service
while giving the remaining slots to history. Unused slots can be borrowed by
the other lane. With a one-file budget, alternate lanes across calls/restarts.
Within history, finish admitted partial files before opening new ones. Eligible
tails rotate by stable source ID instead of letting the largest hot source
permanently win. The effective budget never exceeds available work.

A compact machine-scoped `reconstruction-scheduler-v1` cursor stores allocation
attempts and lane/tail position. Its offset is a logical sequence, not JSONL bytes
or Token consumption. It is written before processing the selected work; a failed
attempt can advance scheduling without advancing a source's ingestion cursor.
This is not a multi-writer distributed fairness guarantee or an overload SLA.
Existing source/cursor transactions and accounting policies remain unchanged.

## Actionable work versus incomplete evidence

Pending empty files are inspected once, then wait for append without producing
usage. A trailing incomplete record remains incomplete in its source/parser
state, but an idle EOF has no immediately runnable backfill work. The report's
`pendingSources` now counts actionable pending/reconstructing sources in the
requested target set; it is **not** a count of every incomplete evidence state.
This semantic refinement does not add/change JSON fields. It must not be used as
proof of complete historical coverage. Once the partial record is completed,
normal ingestion resumes and counts it once.

Targets outside allowed Codex roots are excluded before budget allocation and
reported, so an invalid target cannot repeatedly consume a valid source's slot.
No source files, existing usage facts or credentials are modified by scheduling.

Tests cover hot-source/history coexistence, rotating tails, single-slot restart,
policy-upgrade classification, extreme requested budgets, a real synthetic
multi-file ingestion sequence and empty/partial EOF followed by append. These
establish local scheduling behavior, not real-account calibration or native-app
delivery. Historical correction and main source-union promotion remain separate.
