# Isolated native preview

The native app accepts `--isolated-profile <UUID>` for local acceptance without
using the normal ledger. The UUID is normalized to lowercase; missing, malformed,
duplicate or misspelled isolated flags fail closed. This is an explicit launch
option, not a replacement for the user's default configuration.

The profile root is `CodexUsageLedgerPreview-<UUID>` under the system temporary
directory. It contains `data/ledger.sqlite3` and a separate, initially empty
`codex/` source directory. Serve and daemon modes both receive explicit paths.
Inherited Codex-home, ledger and Web-root environment overrides are removed.
Only bundled Rust/Web resources and the existing fixed loopback port are used;
navigation, CSP, nonpersistent Web storage and health ownership checks remain
unchanged. An existing listener is not permission to attach to another ledger.

The profile root/data/source directories are private. Existing symbolic links
at the profile root, data, database or source boundary are refused, including
broken links. This does not claim protection against a hostile same-user process
racing filesystem operations. No source recovery or data import is automatic.

Zoom, collection preference and language use a separate UUID-qualified defaults
domain. Invalid isolated arguments also avoid the production preference domain.
The native toolbar displays an isolated-preview label. Temporary profile data
and its separate preference domain persist until explicitly cleaned; restarting
with the same UUID is intentional. Do not treat this as a production migration
receipt or as representative token evidence without synthetic source fixtures.

## Validation status

Pure Swift tests cover UUID validation, normal/isolated path separation, bundled
resource identity, explicit serve/daemon source arguments, environment overrides,
directory permissions and broken database-link refusal. The app builds through
the standard script with arm64 and ad-hoc signature checks. A direct native
launch attempt returned exit code 1 without diagnostics, before the isolated
data directory was created. No successful native-window acceptance is claimed.
Start/retry/mode switch/quit/port conflict, language persistence, zoom, export
failure and installed-app migration acceptance remain required. Signing here
does not mean notarization or public release. Identity/process-boundary changes
still require code-owner review before release.
