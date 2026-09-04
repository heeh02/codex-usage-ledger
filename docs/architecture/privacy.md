# Privacy and public-data policy

The application is local-first and the repository is public. Both boundaries
must hold independently.

## Runtime boundary

- Bind the dashboard to loopback only.
- Use Codex's bundled app-server for supported read-only account usage.
- Never copy, persist, refresh, print, or request OAuth credentials.
- Persist pseudonymous account fingerprints, not raw account identifiers.
- Keep prompts and absolute source paths out of dashboard responses by default.
- Treat imported databases and rollouts as private user data, never as support
  attachments suitable for a public issue.

## Repository boundary

Public commits may contain only synthetic project, session, account, person,
path, quota, and usage examples. Screenshots must be generated from demo mode
and reviewed at full resolution before commit. Cropping or visual blur is not a
substitute for synthetic data because image metadata and missed labels remain a
risk.

The privacy check scans current source, fixtures, documentation, and governance
files for known private markers, user-home paths, email addresses, and raw-style
account identifiers. Passing the scanner is necessary but not sufficient;
reviewers must inspect new binary assets manually.

Security vulnerabilities or accidental disclosure must be reported through the
private channel in [`SECURITY.md`](../../SECURITY.md), not a public issue.
