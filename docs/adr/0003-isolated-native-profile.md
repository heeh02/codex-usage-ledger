# Explicit isolated native profile

Status: accepted for the unreleased branch; native and code-owner acceptance pending.
Date: 2026-09-05.

## Decision

Native acceptance must not implicitly migrate the installed ledger. Add the
explicit UUID-only `--isolated-profile` argument, with separate temporary
ledger/source paths and a separate preference domain. Invalid requests fail
closed. Normal launches retain existing paths and behavior. The bundled binary,
Web assets, loopback port, health ownership and WebView security policy are
unchanged. The window visibly identifies an isolated launch.

Reject relying only on an inherited environment variable (easy to lose when
launching an app), copying the live ledger for ordinary GUI testing, or changing
ports/navigation policies to bypass an existing listener. The profile has no
automatic source import and is not proof of accounting completeness.

## Consequences

The launch argument is additive and changes no database schema. Private
temporary directories and link refusal limit accidental access to real data;
this is not adversarial same-user filesystem sandboxing. Tests cover argument
validation, path/command/environment separation, permissions and link refusal.
Build/signature checks are separate from native launch and migration acceptance.
See [implementation and current evidence](../architecture/native-isolated-preview.md).
