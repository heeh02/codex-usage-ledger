# macOS Shell Instructions

The macOS target owns the app lifecycle, menu-bar/Dock surfaces, WKWebView
containment, local service process, zoom, localization bridge, and export panels.

## Boundaries

- Never parse Codex logs, databases, auth files, or ledger tables in Swift.
- Launch only the bundled Rust binary and bind it to the fixed loopback endpoint.
- Keep navigation, CSP, message-handler, and process-lifecycle checks fail closed.
- Do not weaken the non-persistent web data store or URL allowlist for convenience.
- Keep bridge message names and payloads centralized and typed.

## Validation

- Build through `macos/build-app.sh`; do not hand-edit bundle output.
- Verify Info.plist, architecture, nested executable permissions, and deep signing.
- Test start, retry, mode switch, quit, port conflict, language persistence, zoom,
  and export failure paths for lifecycle changes.

## Code Review Rules

- Reject direct Codex credential or source-data access from the shell.
- Reject unbounded waits, orphaned child processes, external navigation, and
  message handlers that accept unvalidated payloads.
