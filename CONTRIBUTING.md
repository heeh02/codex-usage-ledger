# Contributing

Thank you for helping improve Codex Usage Ledger. The project welcomes focused
bug fixes, compatibility work, tests, documentation, accessibility improvements,
translations, and carefully reviewed product changes.

## Before opening code

1. Read [`AGENTS.md`](AGENTS.md) and the `AGENTS.md` in the module you will edit.
2. Search existing issues and open one before changing accounting, persistence,
   privacy, security, public API contracts, or release behavior.
3. Use a branch or worktree. Do not develop directly on `main`.
4. Use only synthetic fixtures. Never attach a Codex database, auth file, raw
   rollout, prompt, account hash, private path, or screenshot of private projects.

## Development setup

- Rust stable from `rust-toolchain.toml` with rustfmt and clippy.
- Node.js 22 and npm for `web/`.
- Apple Silicon macOS 13+ with Xcode Command Line Tools for the app bundle.

Core checks:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
scripts/check-module-boundaries.sh
node scripts/check-workflow-security.mjs
npm --prefix web ci
npm --prefix web run build
```

On Apple Silicon macOS:

```bash
bash macos/build-app.sh
codesign --verify --deep --strict "dist/Codex Usage Ledger.app"
```

## Pull requests

- Keep one primary concern per pull request and explain any unavoidable cross-module change.
- Describe the user-visible outcome, affected contract, risk, and exact validation performed.
- Add regression tests before changing parser, accounting, migration, filter, or lifecycle behavior.
- Do not include generated build output. CI must be able to rebuild it.
- Update `CHANGELOG.md` for user-visible behavior and add an ADR for a lasting architecture decision.

Maintainers may ask for a large pull request to be split before detailed review.
Passing CI is necessary but does not override the accounting and privacy invariants.

## Reporting security issues

Do not open a public issue for a vulnerability or accidental private-data
exposure. Follow [`SECURITY.md`](SECURITY.md).
