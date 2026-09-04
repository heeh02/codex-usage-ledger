# Repository Working Agreement

These instructions apply to every change. When working below `src/`, `web/`,
`macos/`, or `docs/`, also read that directory's `AGENTS.md`. Start Codex in the
module directory when possible so its local instructions are loaded automatically.

## Product invariants

- Keep official account totals separate from local project/session attribution.
- Never sum Codex cumulative per-thread token counters.
- Preserve the four exclusive token buckets: uncached input, cache read, cache
  write, and output. Reasoning is an output subset and is never added twice.
- Never turn unknown, pending, quarantined, missing, or unrecoverable usage into
  confirmed zero or estimated fact.
- Never read, write, copy, log, commit, or request Codex credentials. The app may
  call the bundled Codex app-server's read-only usage methods.
- Keep the HTTP dashboard loopback-only. Do not broaden Host, Origin, navigation,
  filesystem, or network scope without an explicit security design review.

## Change boundaries

- Keep one concern and one primary module per pull request. Separate behavior,
  schema, visual, dependency, and cleanup changes when they can be reviewed alone.
- Preserve unrelated work and do not rewrite contributor changes to simplify a patch.
- Do not edit generated output under `target/`, `dist/`, `web/dist/`, or
  `web/node_modules/`. Rebuild it from source when validation needs it.
- Public examples, tests, screenshots, and documentation must use synthetic data.
- Before changing a public API, database schema, JSON response, or WKWebView
  message, document the compatibility impact and add a contract test.

## Required checks

- Rust: `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features`.
- Web: from `web/`, run `npm ci` when dependencies changed, then `npm run build` and the relevant unit/E2E checks.
- macOS: run `bash macos/build-app.sh` on Apple Silicon and verify the app with `codesign --verify --deep --strict`.
- Documentation/governance: run current-tree and reachable-history privacy,
  link, generated-file, and version consistency checks.
- Report platform-specific evidence honestly; a local build is not notarization or distribution proof.

## Review and safety

- Accounting, migrations, identity, credential boundaries, loopback security, and
  release/signing changes require code-owner review.
- Flag double counting, cross-account relabeling, gap allocation to projects,
  destructive migration, raw private data, non-loopback serving, or hidden unknowns.
- Prefer the smallest reversible implementation. Ask before adding a production
  dependency or rewriting published Git history.

## Code Review Rules

- Reject changes that weaken a product invariant even when tests pass.
- Require a safe migration path for every persisted-schema change.
- Require synthetic regression evidence for parser or attribution fixes.
- Keep formatting-only feedback in automated checks unless it obscures correctness.
