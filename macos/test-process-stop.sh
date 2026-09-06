#!/usr/bin/env bash
set -euo pipefail
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
test_directory="$(mktemp -d)"
trap 'rm -r -- "$test_directory"' EXIT
xcrun swiftc -sdk "$(xcrun --sdk macosx --show-sdk-path)" -target arm64-apple-macos13.0 \
  "$script_dir/Sources/CodexUsageLedgerApp/LedgerProcessStopDeadline.swift" \
  "$script_dir/Tests/ProcessStopTests.swift" -o "$test_directory/process-tests"
"$test_directory/process-tests"
