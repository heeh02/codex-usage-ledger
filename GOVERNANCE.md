# Governance

Codex Usage Ledger uses a maintainer-led, review-first model while the community
is small. The current maintainer is `@heeh02`.

## Roles

- Contributors propose issues, pull requests, reviews, translations, and tests.
- Reviewers demonstrate sustained understanding of one module and may be added
  to its CODEOWNERS team.
- Maintainers manage releases, security advisories, repository rules, ownership,
  compatibility policy, and final merges.

## Decisions

- Routine changes are decided in pull-request review.
- Accounting semantics, persistent schema, privacy/security boundaries, supported
  platforms, or module dependency direction require an ADR and maintainer approval.
- A breaking contract requires a migration plan and an announced release boundary.
- Security fixes may be developed privately and disclosed after a patched release.

## Becoming a reviewer or maintainer

Regular contributors may be nominated after multiple high-quality changes and
reviews in the relevant module. Maintainer status requires demonstrated judgment
across correctness, privacy, release safety, and community conduct.

This file describes authority, not ownership of contributions. Licensing remains
governed by [`LICENSE`](LICENSE).
