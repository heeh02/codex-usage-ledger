# Release process

The tagged workflow produces three checksummed artifacts from one reviewed tag:

- an Apple Silicon macOS GUI application with the bundled Rust service and Web UI;
- a Linux x86-64 Rust CLI/local-service archive;
- a Windows x86-64 Rust CLI/local-service archive.

The macOS job uses locked Cargo and npm dependency graphs and runs the same
bundle validation and deep signature verification as a local build. Linux and
Windows each run the locked cross-platform core test suite and a packaged-binary
smoke check on their native GitHub-hosted runner before uploading an artifact.

## Release gates

1. Merge only with all required CI checks green.
2. Update `CHANGELOG.md` and keep Cargo, Web, and Info.plist versions equal.
3. Create an annotated `vX.Y.Z` tag on the exact reviewed commit.
   The quality gate rejects lightweight tags, version mismatches, and tags that
   do not resolve to the workflow commit.
4. The release workflow first reruns Rust, Web, browser, privacy, contract,
   license, and SBOM gates on the tag itself. Only then does it rebuild the
   macOS app and both cross-platform CLI packages, verify the nested app bundle,
   and upload checksummed, read-only workflow artifacts. A final job with no
   source checkout receives the only `contents: write` token, attaches the
   release receipts, and opens or updates a draft release.
5. A maintainer compares the tag, workflow run, checksum, and release notes
   before publishing the draft.

## Signing boundary

The current public workflow applies an ad-hoc signature. That proves the bundle
is internally consistent and that macOS can validate its nested code, but it is
not Developer ID identity, notarization, Gatekeeper acceptance, or App Store
evidence. Artifact names include `adhoc` so this boundary is visible after a
download.

A future Developer ID workflow must use a protected GitHub environment, import
the certificate only inside the release job, notarize the exact checksummed
archive, staple the ticket, and record Gatekeeper verification. It must never
expose signing secrets to pull requests or claim notarization based on an
ad-hoc build.

Windows and Linux artifacts are CLI/local-service packages, not native GUI
applications. They are currently checksummed but not Authenticode-signed,
code-signed, or distribution-notarized; the release notes must preserve that
boundary. Each archive includes the production Web dashboard; run `serve` or
`daemon` with `--web-root ./web` (PowerShell: `--web-root .\web`) to expose it on
the default loopback address.

Every packaged platform includes `LICENSE`, `SECURITY.md`,
`THIRD_PARTY_NOTICES.md`, CycloneDX Rust/Web SBOMs, and a deterministic
third-party license receipt. The receipt embeds license files for installed
locked dependencies. A versioned allowlist pins the declared license and
upstream source for packages whose published archive has no standalone text;
new undeclared exceptions fail the build. Lockfile-only binaries for other
platforms are not represented as installed files on the current runner.

## Reproducibility scope

The source revision, Rust dependency graph, npm dependency graph, toolchain
channel, target architecture, and build entry point are fixed. GitHub-hosted
runner and Xcode image updates can still change bytes, so the workflow promises
a repeatable, auditable build recipe—not cross-run byte-for-byte identity until
the macOS SDK and signing environment are also pinned and independently tested.
