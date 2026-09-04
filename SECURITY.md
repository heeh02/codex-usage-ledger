# Security policy

Please do not open a public issue containing authentication tokens, account
identifiers, prompt content, absolute private paths or a raw Codex database.

The supported MVP is a standalone local macOS app backed by the cross-platform
Rust core. Its dashboard server is loopback-only. Binding that server to a
non-loopback address, placing it behind a public reverse proxy, or modifying
Codex authentication is outside the security model.

The HTTP surface rejects non-loopback `Host` values and cross-site `Origin`
headers. The embedded web view accepts navigation only to the expected
`127.0.0.1` service and verifies the service identity before displaying it.

The app does not require access, refresh, or API tokens to be included in a bug
report. Do not attach the ledger database or a copied Codex home. Prefer a small
synthetic JSONL fixture that reproduces the issue.

When reporting a vulnerability, use a
[private GitHub security advisory](https://github.com/heeh02/codex-usage-ledger/security/advisories/new),
include a minimal synthetic fixture and the affected version, and rotate any
credential that was accidentally exposed before sharing the report.
