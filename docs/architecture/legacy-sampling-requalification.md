# Requalifying legacy sampling associations

Missing `source_record_evidence` rows do not themselves prove that history is
lost. A retained sampling record preserves an observation timestamp and event
identity independently of raw-log retention. Conversely, matching amounts alone
do not establish which rollout record supplied a measurement.

`audit-legacy-sampling` replays the current sampling association mechanism against
those retained anchors. It does not write associations, alter quality, rewrite
amounts, change model/account/project labels or promote an accounting policy.

```sh
codex-usage-ledger audit-legacy-sampling --db /work/review/ledger.sqlite3 \
  --codex-home /work/synthetic-codex --thread synthetic-thread \
  --start 2026-01-01T00:00:00Z --end 2026-02-01T00:00:00Z
```

The input must already use the current schema. A bounded retained-anchor page
loop runs in one ledger snapshot, includes unknown observations and adds 500 ms
of boundary context. The indexed rollout must resolve inside the explicit
source roots. Reading starts at byte zero to rebuild inherited-prefix and
cumulative-counter state using the same parser as live sampling. It does not
read Codex credentials or require the old `logs_2.sqlite` file to remain present.

All nearby candidate records participate, including invalid, inherited and
unchanged records. The mutual-unique-nearest rule cannot skip an unavailable
nearest record and substitute a farther one. No account/model filter is applied
before association. The report includes the observed file identity, bytes read,
source metadata-change flag, category counts and stored/proposed components.
`--include-links` additionally exposes source offsets and canonical record
digests for private review. Limits cap anchors, prefix candidates and source
bytes; exceeding a cap fails instead of dropping competition silently.

## Interpretation

- `amounts_match`: the current association finds the same six consumed amounts.
  Cache-write coverage weight is reported separately because it is knowledge
  metadata, not an additional Token quantity.
- `amounts_changed`: a nearby occurrence is associated but its freshly normalized
  quantities differ. This is a correction proposal, not an applied correction.
- `candidate_unavailable`: the nearest occurrence cannot supply a valid delta.
- `ambiguous` / `no_candidate`: no unique supported association exists.
- `stored_unconfirmed`: the original record stays unconfirmed, even when the
  current parser proposes a value. `proposedRecords` distinguishes available
  proposals from unavailable records within this category.
- `unsupported_legacy_namespace`: this initial audit recognizes only canonical
  primary-log event IDs, not copied-source or generation namespaces.

The ledger anchors and rollout are different evidence surfaces, not independent
server-side usage meters. The observed neighborhood can be incomplete if legacy
observations were never retained. A matching association is not permission to
declare all history recovered; `historyComplete` and `migrationAuthorized` remain
false. A changed source invalidates reuse for migration. Model/account/project
identity is not re-established by this amount comparison and must be preserved
or reviewed independently. Exact source-key restoration still needs a reviewed
mapping of machine/file namespaces and transactional fact-version checks.

Synthetic acceptance covers stale last-usage values, unchanged re-emits,
unknown competing anchors, invalid nearest records, missing original logs,
read-only byte preservation and budget/schema refusal. This is the source
requalification prerequisite for the [main-query preview](union-main-query-preview.md),
not another alternative formula for the dashboard total.
