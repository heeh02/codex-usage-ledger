# Bounded reconstruction review batches

`draft-reconstruction-batch` produces review drafts only. It does not apply them,
import new evidence, change identities, or switch production policy.

```sh
codex-usage-ledger draft-reconstruction-batch --db /work/review-shadow.sqlite3 \
  --codex-home /work/synthetic-codex --output-dir /work/private-drafts \
  --limit 2 --max-bytes-per-source 1073741824
```

The output directory must already exist outside Codex home. Read-only source
inventory is ordered by unique thread identifier; `--after` resumes strictly after
the previous report's `nextAfter`. Each call accepts 1–10 sources with a bounded
byte budget per source. The source reader may make multiple verification passes;
this byte budget is a captured-prefix bound, not total physical I/O or a deadline.
Thread-source ambiguity remains an audit error, not an automatic identity choice.

Each thread has a deterministic hashed draft filename. Existing complete drafts
are structurally verified and compared to the ledger's old facts/expected absences
before reuse. Reuse does not open the rollout. Draft target and parser policy must
match the current batch. A source disappearing after capture does not erase a
valid historical draft, but current source revalidation is not implied by reuse.

Items report `created`, `reused`, `applied`, `incomplete`, or `review_required`, a seal when
available, and the draft path. Per-source failures do not stop subsequent sources.
Pagination progress is not successful coverage: callers must retain failed items
and review them, not treat `hasMore=false` as a completed migration. Incomplete or
damaged files are never overwritten or deleted. A fresh output directory is needed
for an explicitly reviewed retry with a different budget or corrected baseline.
The tool does not yet provide unattended failed-item retry scheduling.

No persisted ledger schema or HTTP DTO changes. The CLI returns a version-1 batch
report; source drafts retain their separate version/policy and correction gates.
An applied draft will no longer match its original expected ledger facts. The
batch verifies its correction receipt and post-image in a read transaction before
reporting `applied`, without redrafting the corrected state. Changed post-images
remain review-required; receipt verification is not a new source scan.

Synthetic tests verify reuse after the original source is moved, unchanged draft
bytes, strict seek continuation, stale-ledger refusal, preservation of damaged
drafts, source-home output refusal, applied-receipt checks and invalid batch bounds. This utility makes
manual review generation repeatable; it is not full migration orchestration or
proof of complete account usage.
