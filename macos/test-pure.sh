#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
test_binary="$(mktemp -d)/codex-usage-ledger-pure-tests"
sdk_path="$(xcrun --sdk macosx --show-sdk-path)"
swiftc_bin="$(xcrun --sdk macosx --find swiftc)"

"$swiftc_bin" \
  -sdk "$sdk_path" \
  -target arm64-apple-macos13.0 \
  -o "$test_binary" \
  "$repo_root/macos/Sources/CodexUsageLedgerApp/NativeLocalization.swift" \
  "$repo_root/macos/Sources/CodexUsageLedgerApp/DashboardBridge.swift" \
  "$repo_root/macos/Sources/CodexUsageLedgerApp/LedgerProcessDiagnostics.swift" \
  "$repo_root/macos/Sources/CodexUsageLedgerApp/LedgerServiceState.swift" \
  "$repo_root/macos/Sources/CodexUsageLedgerApp/LedgerServiceLifecycle.swift" \
  "$repo_root/macos/Tests/PureStateTests.swift"

"$test_binary"
