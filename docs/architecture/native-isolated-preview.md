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
navigation, CSP and nonpersistent Web storage remain unchanged. Health readiness
now checks the current launched process ID and generation before creating the
WebView. Earlier service-name-only readiness was insufficient; see
[the corrected process binding](../adr/0005-native-health-process-binding.md).
An existing listener is not permission to attach to another ledger.

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

## Language bootstrap

The saved language is injected at document start into the nonpersistent Web
store. After a validated language-change bridge message, Swift updates both
its preference and the next document's bootstrap script. Merely updating the
preference left the original script stale, allowing a later native reload to
restore the previous language. An unchanged language now leaves scripts alone;
a changed language replaces the language script and reinstalls the same CSP.
Both scripts remain main-frame-only at document start. This does not trigger a
navigation or reset React filters. Message handlers and the URL allowlist are
unchanged. Pure/native-controller tests cover valid languages, invalid-string
fallback, stable unchanged scripts and preserved CSP injection.

## Validation status — 2026-09-07

Pure Swift tests cover UUID validation, normal/isolated path separation, bundled
resource identity, explicit serve/daemon source arguments, environment overrides,
directory permissions and broken database-link refusal. The owned-process stop
tests pass. The standard build produced an arm64, macOS-13-minimum, ad-hoc-signed
source bundle; deep signature verification passed. Its file-manifest SHA-256 is
`41d6fa850935d0598d9e0e5241d4848d33a00fe065ac2b97da00ba60ff4a82a0`.

The source bundle launched successfully using a fresh isolated UUID profile,
superseding the earlier failed launch attempt. Native AX/window readback showed
the isolation banner and empty-evidence states. Verified Chinese-to-English,
English native reload, quit/reopen retaining English, English-to-Chinese and
Chinese native reload. Command-plus/minus/reset changed 100/110/100 percent
without losing the selected Today view; a 700-pixel narrow window at 160 percent
retained readable controls and could scroll to the final overview sections.
This is empty-ledger native evidence, not populated-project/model acceptance.

The collection-dialog exercise did not establish a reliable cancellation
result: enabled state was observed and explicitly returned to read-only mode.
Final preference readback was collection disabled, Chinese, 100 percent. Both
shutdowns completed; the owned app/service processes exited and port 47127 was
released. Explicit runtime paths remained under the isolated profile, the source
directory was empty, and raw/retained/reconstructed usage tables contained zero
rows. No installed app replacement or real-ledger migration was performed.

Retry/port-conflict scenarios, reliable modal cancellation, export-failure paths,
populated-data native journeys and installed-app migration acceptance remain
open. Signing here does not mean notarization or public release. Identity/
process-boundary changes still require code-owner review before release.
