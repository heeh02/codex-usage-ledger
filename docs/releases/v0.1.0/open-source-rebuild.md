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

## Release publication

The source-free tagged release workflow completed on macOS, Linux, and Windows,
and the downloaded assets were verified before the repository became Public.

- Release: <https://github.com/heeh02/codex-usage-ledger/releases/tag/v0.1.0>
- Release workflow: <https://github.com/heeh02/codex-usage-ledger/actions/runs/33872751104>
- Annotated tag target: `4e142c1885d94a976ce3f046d5d4795385c0af91`.
- macOS Apple Silicon archive SHA-256:
  `23d593a4bbf7ac1dd80150a56bf6b1765dde3c56f05dc6eb3bcd5bafe4fa9c39`.
- Linux x86-64 archive SHA-256:
  `2d6bd32295e4c6797e58455d9a5447dc0e8e1736ca5e98dfcaae36da2150fc0d`.
- Windows x86-64 archive SHA-256:
  `60e7ec376d0fc3bf1cd997711189dd4b1e1f8894580d245116f31860a730952a`.

The macOS application passed deep strict code-signature verification and is
explicitly ad-hoc signed. Archive inspection confirmed the expected native
binary format for each platform, the bundled dashboard, project notices,
CycloneDX Rust and Web SBOMs, and the generated third-party license receipt.
The repository and release were then verified through the unauthenticated
public GitHub API.

## Independent acceptance

Independent release review found no P0 or P1 defects. One accepted P2 boundary
remains: platform-only packages present only in lockfiles and not installed on
the current runner do not enter that runner's generated license-text receipt.
The boundary is documented accurately; license-policy exceptions remain pinned
to exact package versions and unknown dependencies fail closed.
