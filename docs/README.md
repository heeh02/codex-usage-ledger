# Documentation map

The repository separates normative contracts from explanatory architecture,
versioned evidence, and historical decision material.

- `contracts/`: current, testable product invariants and public data semantics.
- `architecture/`: current component boundaries, dependency direction, and
  security/privacy model.
- `adr/`: durable architecture decisions that change those boundaries.
- `releases/`: evidence tied to a version; a receipt is not a timeless promise.
- `archive/`: completed goals, investigations, and superseded plans retained for
  traceability only.
- `assets/`: synthetic public media approved for documentation.

When documents disagree, source code and tests do not silently decide the
product contract. Open an issue, update the relevant contract or ADR, and make
the implementation and verification evidence agree in the same change.

The current tagged build and signing boundary is documented in
[`releases/process.md`](releases/process.md).
The generated loopback response contract is documented in
[`contracts/http-api.md`](contracts/http-api.md).
The intentionally small crate surface is documented in
[`contracts/rust-api.md`](contracts/rust-api.md).
