# macOS runner migration

## Problem and authority

Issue #884 migrates four retiring macOS 14 runner selections. The maintainer
selected option 2 on 2026-10-15: macOS binaries and Homebrew bottles now support
Sequoia 15 or newer. That decision explicitly replaces the issue's Sonoma
compatibility criteria; all remaining criteria and empty exclusions stand.
See [ADR 0083](../../adr/0083-macos-sequoia-release-floor.md).

## Design

Use explicit `macos-15` (native arm64) for the Keychain job, macOS installer
smoke leg, Apple Silicon release build, and macOS bottle producer. Retain the
unlocked dedicated Keychain, its 15-minute timeout, Linux/Windows coverage,
and existing release-to-formula-to-bottles-to-JSON-merge publication flow.
Set `MACOSX_DEPLOYMENT_TARGET=15.0` only for the Apple Silicon release build.
Homebrew creates its natural `arm64_sequoia` bottle; no retagging occurs.
The release workflow remains the owner; no ownership transition is needed.

Update current release guidance and stale comments, including the removed
`update-homebrew.yml` references in `homebrew/README.md`. Clearly state the new
floor; make no Sonoma compatibility promise. Preserve historical ADRs/specs.

## Success and validation

The final workflow tree contains no retired macOS runner selection. Existing CI
must pass Keychain and installer smoke on macOS 15, with Linux/Windows intact.
Before the final PR, a temporary branch-push workflow builds the same release
command with the floor above on macOS 15, checks arm64 Mach-O minimum OS and
system-only dynamic dependencies, packages binary/manpages, and uses the real
formula template in an isolated local tap. It runs `brew install --build-bottle`,
`brew bottle --json --no-rebuild`, and `brew bottle --merge --write --no-commit`.
The generated tag must actually be `arm64_sequoia`.

Upload the bottle, JSON, formula and source/binary checksums as CI artifacts.
Fresh macOS 15 and macOS 26 jobs merge the JSON with a job-local file root URL,
install normally from that formula without `--force-bottle`, require the receipt
`poured_from_bottle` flag, compare installed binary SHA256 to the producer, and
run `bzr --version` and `brew test`. Record OS, actual tag and results. No tag,
GitHub Release, production tap, credentials or signing/attestation publication
is involved. Delete the temporary workflow after successful evidence capture;
the final PR links the immutable run and explains source/artifact identity.
A missing bottle, unexpected tag, dependency or OS floor, source fallback,
wrong checksum/version, or failed platform job blocks completion.

Repository guardrails and the full default functional run remain mandatory.
No new CLI command/output behavior is introduced, so the existing real-server
functional suite covers the unchanged executable behavior.

## Failure model

- Actors and deployments: GitHub-hosted arm64 macOS 15/26 validation jobs;
  existing release jobs; macOS 15-or-newer Apple Silicon installers.
- Invariants and assets: explicit supported floor, natural bottle metadata,
  exact artifact identity, existing cross-platform coverage, no production
  publication during proof, maintained Keychain isolation and timeout.
- Accepted failure classes: retired-runner brownouts in historical runs are
  irrelevant after final selections change; Sonoma support is explicitly
  removed by the maintainer's policy choice, not inferred compatibility.
- Covered elsewhere: ordinary CLI/API correctness by repository tests and
  functional suite; production credentials/publication by existing release
  jobs. Intel source builds retain the existing formula path; this migration
  changes the prebuilt Apple Silicon release and bottle floor.
