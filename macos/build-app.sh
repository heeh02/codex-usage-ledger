#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'

readonly APP_NAME="Codex Usage Ledger"
readonly EXECUTABLE_NAME="CodexUsageLedger"
readonly BUNDLE_IDENTIFIER="com.heeh02.CodexUsageLedger"
readonly RUST_BINARY_NAME="codex-usage-ledger"
readonly RUST_TARGET="aarch64-apple-darwin"
readonly MINIMUM_MACOS_VERSION="13.0"

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
REPO_ROOT="$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd -P)"
DIST_DIR="$REPO_ROOT/dist"
APP_PATH="$DIST_DIR/$APP_NAME.app"
INFO_PLIST_SOURCE="$SCRIPT_DIR/Info.plist"
ICON_SOURCE="$SCRIPT_DIR/Assets/AppIcon.icns"
SWIFT_SOURCES_DIR="$SCRIPT_DIR/Sources/CodexUsageLedgerApp"
WEB_DIR="$REPO_ROOT/web"
WEB_DIST_SOURCE="$WEB_DIR/dist"
RUST_BINARY_SOURCE="$REPO_ROOT/target/$RUST_TARGET/release/$RUST_BINARY_NAME"
CHECKSUM_PATH="$APP_PATH.sha256"
FILE_CHECKSUMS_PATH="$APP_PATH.files.sha256"

log() {
    printf '[build-app] %s\n' "$*"
}

fail() {
    printf '[build-app] error: %s\n' "$*" >&2
    exit 1
}

require_command() {
    command -v "$1" >/dev/null 2>&1 || fail "required command not found: $1"
}

resolve_cargo() {
    if [[ -n "${CARGO:-}" && -x "${CARGO}" ]]; then
        printf '%s\n' "$CARGO"
        return
    fi
    if command -v cargo >/dev/null 2>&1; then
        command -v cargo
        return
    fi
    local candidate
    for candidate in \
        "$HOME/.cargo/bin/cargo" \
        "$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo"
    do
        if [[ -x "$candidate" ]]; then
            printf '%s\n' "$candidate"
            return
        fi
    done
    return 1
}

[[ "$(uname -s)" == "Darwin" ]] || fail "this app bundle must be built on macOS"
[[ "$APP_PATH" == "$REPO_ROOT/dist/Codex Usage Ledger.app" ]] \
    || fail "refusing unexpected app output path: $APP_PATH"
[[ -f "$INFO_PLIST_SOURCE" ]] || fail "missing $INFO_PLIST_SOURCE"
[[ -f "$ICON_SOURCE" ]] || fail "missing $ICON_SOURCE"
[[ -f "$REPO_ROOT/Cargo.lock" ]] || fail "Cargo.lock is required for a reproducible build"
[[ -f "$WEB_DIR/package-lock.json" ]] || fail "web/package-lock.json is required"
[[ -d "$SWIFT_SOURCES_DIR" ]] || fail "missing Swift source directory: $SWIFT_SOURCES_DIR"

require_command npm
require_command xcrun
require_command plutil
require_command codesign
require_command shasum
require_command lipo
require_command find
require_command sort

CARGO_BIN="$(resolve_cargo)" || fail "cargo was not found"
CARGO_TOOLCHAIN_BIN="$(dirname -- "$CARGO_BIN")"
if [[ -x "$CARGO_TOOLCHAIN_BIN/rustc" ]]; then
    export PATH="$CARGO_TOOLCHAIN_BIN:$PATH"
fi
SWIFTC_BIN="$(xcrun --sdk macosx --find swiftc)"
SDK_PATH="$(xcrun --sdk macosx --show-sdk-path)"
[[ -x "$CARGO_BIN" ]] || fail "cargo is not executable: $CARGO_BIN"
command -v rustc >/dev/null 2>&1 || fail "rustc was not found for cargo: $CARGO_BIN"
[[ -x "$SWIFTC_BIN" ]] || fail "swiftc is not executable: $SWIFTC_BIN"
[[ -d "$SDK_PATH" ]] || fail "macOS SDK not found: $SDK_PATH"

SWIFT_SOURCES=()
while IFS= read -r source; do
    SWIFT_SOURCES+=("$source")
done < <(find "$SWIFT_SOURCES_DIR" -type f -name '*.swift' -print | LC_ALL=C sort)
(( ${#SWIFT_SOURCES[@]} > 0 )) || fail "no Swift sources found under $SWIFT_SOURCES_DIR"

export LC_ALL=C
export TZ=UTC
export SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-1}"
export MACOSX_DEPLOYMENT_TARGET="$MINIMUM_MACOS_VERSION"
export SWIFT_DETERMINISTIC_HASHING=1

log "building Rust release binary for $RUST_TARGET"
(cd "$REPO_ROOT" && "$CARGO_BIN" build --release --locked --target "$RUST_TARGET")
[[ -x "$RUST_BINARY_SOURCE" ]] || fail "Rust binary was not produced: $RUST_BINARY_SOURCE"

log "installing locked web dependencies"
(cd "$WEB_DIR" && npm ci --no-audit --no-fund)
log "building React dashboard"
(cd "$WEB_DIR" && npm run build)
[[ -f "$WEB_DIST_SOURCE/index.html" ]] || fail "web build did not produce dist/index.html"

/bin/mkdir -p "$DIST_DIR"
STAGING_ROOT="$(/usr/bin/mktemp -d "$DIST_DIR/.codex-usage-ledger-build.XXXXXX")"
STAGING_APP="$STAGING_ROOT/$APP_NAME.app"
STAGING_CONTENTS="$STAGING_APP/Contents"
STAGING_MACOS="$STAGING_CONTENTS/MacOS"
STAGING_RESOURCES="$STAGING_CONTENTS/Resources"
STAGING_RUST_BIN_DIR="$STAGING_RESOURCES/bin"
STAGING_WEB_DIST="$STAGING_RESOURCES/web/dist"
STAGING_INFO_PLIST="$STAGING_CONTENTS/Info.plist"
STAGING_SWIFT_EXECUTABLE="$STAGING_MACOS/$EXECUTABLE_NAME"
STAGING_RUST_BINARY="$STAGING_RUST_BIN_DIR/$RUST_BINARY_NAME"
RELEASE_METADATA_DIR="${CODEX_LEDGER_RELEASE_METADATA_DIR:-}"

cleanup() {
    if [[ -n "${STAGING_ROOT:-}" && -d "$STAGING_ROOT" ]]; then
        /bin/rm -rf -- "$STAGING_ROOT"
    fi
}
trap cleanup EXIT INT TERM

/bin/mkdir -p \
    "$STAGING_MACOS" \
    "$STAGING_RUST_BIN_DIR" \
    "$STAGING_WEB_DIST"
/bin/cp "$INFO_PLIST_SOURCE" "$STAGING_INFO_PLIST"
/bin/cp "$ICON_SOURCE" "$STAGING_RESOURCES/AppIcon.icns"
/bin/cp "$REPO_ROOT/LICENSE" "$STAGING_RESOURCES/LICENSE"
/bin/cp "$REPO_ROOT/SECURITY.md" "$STAGING_RESOURCES/SECURITY.md"
/bin/cp "$REPO_ROOT/THIRD_PARTY_NOTICES.md" "$STAGING_RESOURCES/THIRD_PARTY_NOTICES.md"
/bin/cp "$RUST_BINARY_SOURCE" "$STAGING_RUST_BINARY"
/usr/bin/ditto "$WEB_DIST_SOURCE" "$STAGING_WEB_DIST"
if [[ -n "$RELEASE_METADATA_DIR" ]]; then
    [[ -d "$RELEASE_METADATA_DIR" ]] \
        || fail "release metadata directory not found: $RELEASE_METADATA_DIR"
    /bin/mkdir -p "$STAGING_RESOURCES/SBOM"
    for metadata_file in rust.cdx.json web.cdx.json THIRD_PARTY_LICENSES.txt; do
        [[ -f "$RELEASE_METADATA_DIR/$metadata_file" ]] \
            || fail "missing release metadata: $RELEASE_METADATA_DIR/$metadata_file"
        /bin/cp "$RELEASE_METADATA_DIR/$metadata_file" \
            "$STAGING_RESOURCES/SBOM/$metadata_file"
    done
fi

/bin/chmod 0755 "$STAGING_RUST_BINARY"
/bin/chmod 0644 \
    "$STAGING_INFO_PLIST" \
    "$STAGING_RESOURCES/AppIcon.icns" \
    "$STAGING_RESOURCES/LICENSE" \
    "$STAGING_RESOURCES/SECURITY.md" \
    "$STAGING_RESOURCES/THIRD_PARTY_NOTICES.md"
find "$STAGING_WEB_DIST" -type d -exec /bin/chmod 0755 {} +
find "$STAGING_WEB_DIST" -type f -exec /bin/chmod 0644 {} +
if [[ -d "$STAGING_RESOURCES/SBOM" ]]; then
    find "$STAGING_RESOURCES/SBOM" -type d -exec /bin/chmod 0755 {} +
    find "$STAGING_RESOURCES/SBOM" -type f -exec /bin/chmod 0644 {} +
fi

log "compiling ${#SWIFT_SOURCES[@]} Swift source file(s)"
"$SWIFTC_BIN" \
    -sdk "$SDK_PATH" \
    -target "arm64-apple-macos${MINIMUM_MACOS_VERSION}" \
    -O \
    -whole-module-optimization \
    -parse-as-library \
    -framework AppKit \
    -framework SwiftUI \
    -framework WebKit \
    -o "$STAGING_SWIFT_EXECUTABLE" \
    "${SWIFT_SOURCES[@]}"
/bin/chmod 0755 "$STAGING_SWIFT_EXECUTABLE"

log "validating bundle metadata and architectures"
plutil -lint "$STAGING_INFO_PLIST" >/dev/null
[[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$STAGING_INFO_PLIST")" \
    == "$BUNDLE_IDENTIFIER" ]] || fail "unexpected CFBundleIdentifier"
[[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$STAGING_INFO_PLIST")" \
    == "$EXECUTABLE_NAME" ]] || fail "unexpected CFBundleExecutable"
[[ "$(/usr/libexec/PlistBuddy -c 'Print :LSMinimumSystemVersion' "$STAGING_INFO_PLIST")" \
    == "$MINIMUM_MACOS_VERSION" ]] || fail "unexpected minimum macOS version"
[[ "$(lipo -archs "$STAGING_SWIFT_EXECUTABLE")" == "arm64" ]] \
    || fail "Swift executable is not arm64-only"
[[ "$(lipo -archs "$STAGING_RUST_BINARY")" == "arm64" ]] \
    || fail "Rust executable is not arm64-only"

log "applying ad-hoc signatures"
codesign --force --sign - --timestamp=none "$STAGING_RUST_BINARY"
codesign --force --sign - --timestamp=none "$STAGING_SWIFT_EXECUTABLE"
codesign --force --deep --sign - --timestamp=none "$STAGING_APP"
codesign --verify --deep --strict --verbose=2 "$STAGING_APP"

# Keep a previously valid bundle until the complete staged replacement passes
# metadata and signature verification. Only this exact app path is replaced.
if [[ -e "$APP_PATH" ]]; then
    [[ -d "$APP_PATH" && ! -L "$APP_PATH" ]] \
        || fail "refusing to replace non-directory app path: $APP_PATH"
    /bin/rm -rf -- "$APP_PATH"
fi
/bin/mv "$STAGING_APP" "$APP_PATH"
/bin/rmdir "$STAGING_ROOT"
STAGING_ROOT=""

MANIFEST_TEMP="$DIST_DIR/.CodexUsageLedger.files.sha256.$$"
(
    cd "$DIST_DIR"
    while IFS= read -r file; do
        shasum -a 256 "$file"
    done < <(find "$APP_NAME.app" -type f -print | LC_ALL=C sort)
) > "$MANIFEST_TEMP"
BUNDLE_SHA256="$(shasum -a 256 "$MANIFEST_TEMP" | /usr/bin/awk '{print $1}')"
/bin/mv -f "$MANIFEST_TEMP" "$FILE_CHECKSUMS_PATH"
printf '%s  %s\n' "$BUNDLE_SHA256" "$APP_NAME.app (bundle file manifest)" \
    > "$CHECKSUM_PATH"

log "built $APP_PATH"
log "SHA-256 (bundle file manifest): $BUNDLE_SHA256"
log "per-file checksums: $FILE_CHECKSUMS_PATH"
log "ad-hoc signed only; this build has not been notarized"
