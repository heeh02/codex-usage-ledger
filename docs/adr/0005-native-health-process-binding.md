# Bind native readiness to the launched process

Status: accepted for the unreleased branch; native and security review pending.
Date: 2026-09-07.

## Context

The shell previously accepted any healthy ledger service at the fixed loopback
port while its own child was still alive. Another instance could respond during
the new child's startup/exit race. The WebView was also created before readiness
and merely hidden, allowing it to fetch the wrong instance before validation.

## Decision

The health response includes the serving process ID. Native readiness requires
that ID to match the launched live child, alongside service/status/HTTP checks.
After every awaited health response, the launch generation, child identity and
mode must still match. Missing/malformed/mismatched process IDs are rejected.
Do not construct the WebView until readiness is verified; remove it and clear
its loaded state when the service ceases to be ready.

Reject hiding an eagerly loaded foreign page or accepting only a service name.
Fixed loopback binding, CSP and navigation allowlist remain unchanged. PID
matching prevents accidental cross-instance attachment; it is not authentication
against a hostile same-user process that can forge health responses. The shell
still requires complete lifecycle/port-conflict execution acceptance.

## Compatibility and validation

The health field is additive for HTTP clients. The new native shell intentionally
rejects older backends without it; distribute shell/backend as one bundle. Pure
Swift tests cover own/foreign/missing/invalid IDs and HTTP/service/status failure.
A Rust handler test verifies its actual process ID and absence of ledger data.
Native process/readiness race tests and installed acceptance are distinct from
build or pure-test evidence. This does not authorize stopping existing user apps.
