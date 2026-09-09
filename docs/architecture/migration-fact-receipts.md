# Migration fact receipts

Use `node scripts/audit-usage-facts.mjs <snapshot-file>` on a consistent, quiescent
SQLite copy before and after a migration. It streams ordered row serialization
into SHA-256 for an allowlist of usage facts, local rollups, reconstruction facts,
compacted keys and official daily/thread usage. Counts and digests are printed;
row contents, auth data and catalog titles are not printed. No migration or
source ingestion is performed by the auditor.

This utility does not provide a live cross-table snapshot: it opens independent
read-only queries. First create a consistent SQLite backup, ensure no writer uses
the copy, and keep that condition for the audit. The main-file identity/mtime/size
check detects some changes but cannot prove absence of a WAL-only writer. Do not
use immutable mode to ignore live WAL contents. Sidecar access errors must be
handled explicitly, not silently worked around against the production database.

Matching fingerprints demonstrate preservation of the selected rows and their
dimensions, not correctness/completeness of the accounting algorithm. Schema,
catalog, identity epochs, projections and new retained evidence are outside this
receipt scope. Record their separate migration checks and backfill progress.
Actual user receipts remain outside the public repository. Run the synthetic
utility regression with `node --test scripts/audit-usage-facts.test.mjs`.

## Bounded retained-detail continuation

After a separately validated upgrade, `backfill-requests --db <existing-ledger>
--batches <1..100>` resumes the existing persisted request-detail target, with
at most 1,000 rows per transaction. The default allowance is one batch. Missing
files and unsupported schema versions are rejected by the read-only schema
check before the ledger is opened for maintenance. No source discovery,
authentication, history import or token-rollup recalculation is part of this
command. This is a writing maintenance action, not a read-only audit.

The JSON result separates `backfillComplete` from `historyComplete` (always
false); completion means only that the persisted migration target was reached.
A new process against a completed target reports zero attempted batches. Opening
the ledger can still initialize normal WAL settings, so zero batches does not
promise identical database-file bytes. Compare allowlisted facts and backfill
state separately. The preflight/open pair is not a defense against a hostile
concurrent process replacing the database; perform acceptance on an idle copy.
