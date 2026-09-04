# Open-source repository rebuild receipt

Date: 2026-09-04

The public repository target was initialized from a sanitized source-tree
archive rather than from the pre-governance Git graph. The original remote is a
separate private, read-only archive and is not a parent, ref, tag, release, or
fork of this repository.

## Provenance and privacy

- Initial public-target commit: `c98f29997637f62a4a8158e413f44ce70820d17f`.
- Initial source tree: `c1224c2dc72821c6d1530ad8d7355bc64dcd8286`.
- Initial remote refs: one `main` branch and no tags.
- Current-tree and every-reachable-commit privacy gates passed before push.
- The initial main CI run passed repository policy, Rust on macOS/Linux/Windows,
  Web unit/build/browser tests, supply-chain receipts, and the macOS app build.

CI evidence: <https://github.com/heeh02/codex-usage-ledger/actions/runs/33871304309>

## Governance state

`main` requires pull requests, strict required status checks, linear history,
resolved review conversations, and applies the rules to administrators. Force
pushes and branch deletion are disabled. The workflow token defaults to
read-only; only the source-free tagged-release publisher can receive
`contents: write`.

This receipt proves repository-history isolation and build gates. It does not
claim Developer ID signing, notarization, Windows Authenticode, or Linux package
signing; those boundaries remain explicit in the release process.
