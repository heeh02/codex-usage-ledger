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
