# Documentation Instructions

Documentation is public. Use synthetic names, identifiers, paths, screenshots,
fixtures, dates, and token values unless a public upstream reference is required.

## Classification

- `contracts/`: current normative behavior and compatibility requirements.
- `architecture/`: current system shape and dependency rules.
- `adr/`: immutable accepted architecture decisions.
- `releases/<version>/`: release-specific verification receipts.
- `archive/`: historical goals, plans, and superseded audits; never current authority.

## Privacy and evidence

- Never include home-directory paths, emails, account hashes, raw session IDs,
  prompt text, auth payloads, databases, or private project names.
- Label source evidence, inference, local verification, signed build, notarization,
  and external availability separately.
- A historical document must link to the current contract it no longer governs.

## Code Review Rules

- Reject examples copied from a real ledger or Codex home.
- Reject claims whose validation level is stronger than the recorded evidence.
- Reject new root-level planning reports when an ADR, issue, or release receipt is appropriate.
