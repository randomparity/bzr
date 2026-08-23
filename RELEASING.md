# Releasing `bzr`

This project has two release outputs:

- A crates.io package published from `Cargo.toml`
- GitHub release binaries built from a `v*` git tag

## Before the first publish

Confirm that the crate name is usable on crates.io before you rely on `cargo install bzr`.

Checks to run:

```bash
# Fast local check against the registry index
cargo search '^bzr$'

# Optional: use a purpose-built checker that matches crates.io rules
cargo install cargo-avail
cargo avail bzr
```

Things to verify:

- The exact package name is not already taken on crates.io
- The canonical name does not collide with an existing crate
  Hyphens and underscores are treated as equivalent by crates.io
- The binary name is what users should type after install
  For this project that is `bzr`

If `bzr` is already taken, rename the package before publishing. The install command in the README must match the published crate name.

## Version-number convention

Between releases, `Cargo.toml` carries a `-dev` SemVer pre-release suffix
marking the next planned release. By default this is the next **patch**
version — directly after `v0.4.4` ships the project lives at
`version = "0.4.5-dev"` (the `post-release-bump` job in step 7 of the
checklist applies this automatically). The suffix is only a placeholder:
the actual release version is chosen at tag time per SemVer based on what
landed, so when breaking changes or new features have accumulated the
release-prep PR bumps to the next minor instead — this is how `0.4.5-dev`
shipped as `0.5.0`. The suffix makes development builds clearly
distinguishable from the most recent release in `bzr --version` output, and
SemVer correctly orders `0.4.5-dev < 0.5.0`. `publish-crates.yml` gates
anything containing `-` out of crates.io publishes, so dev builds cannot
accidentally publish.

Each binary additionally embeds the current git short SHA via
`build.rs`, so `bzr --version` prints `bzr 0.4.0-dev (97e0d35)` —
two dev builds at different commits show different versions.
Release builds also carry the SHA (`bzr 0.4.0 (a1b2c3d)`); this is
purely additional traceability and breaks no automation, since
`installer-smoke` does substring matching and `preflight` reads
`Cargo.toml` directly.

## Security assessment

While preparing the dated `CHANGELOG.md` section, review the canonical
[GitHub Security Advisories](https://github.com/randomparity/bzr/security/advisories)
inventory. Every release section must carry exactly one of the following
whole-line markers, which are validated before publication. A dependency update
belongs under a separate dependency heading or explicitly says it is
dependency-only; it never replaces the project-vulnerability assessment.

Use this template when no publicly identified runtime vulnerability in `bzr`
was fixed:

```text
Security assessment: No publicly identified runtime vulnerabilities in bzr were fixed in this release.
```

Use this template when one or more publicly identified runtime vulnerabilities
in `bzr` were fixed. List every qualifying vulnerability that has a public
identifier when the GitHub Release is created; a CVE, GHSA, RUSTSEC identifier,
or comparable public identifier qualifies.

```text
Security assessment: Publicly identified runtime vulnerabilities in bzr were fixed in this release.
#### Vulnerability: <public identifier>
- Affected bzr versions: <version range>
- First fixed version: <version>
- Runtime impact: <impact>
- Advisory: https://public-advisory-url
- Upgrade guidance: <guidance>
```

The advisory link and the advisory itself must use the required fields in
[`SECURITY.md`](SECURITY.md). A public identifier that appears after the final
review but before GitHub Release creation is a residual publication race: the
structural validator cannot make the external advisory inventory atomic with
publication. Update the published release notes promptly if this occurs.

`N/A` is for an external compliance questionnaire only, and is truthful only
when there are no release notes, no publicly known qualifying project
vulnerabilities, or users cannot practically update the software themselves.
`bzr` normally publishes release notes and users can update it, so a normal
release that fixes no qualifying vulnerability uses the exact no-vulnerability
marker above, never `N/A`.

Immediately before pushing the tag, refresh the advisory inventory and validate
the exact candidate release body. Replace `X.Y.Z` with the candidate version,
whose heading must already be `## [X.Y.Z] - YYYY-MM-DD`; the trap removes the
temporary file when the subshell completes, without replacing the caller's EXIT
trap. Extraction requires exactly one literal candidate heading and rejects a
missing or duplicate heading. For the current development section only, set
`VERSION=Unreleased`; its heading must be exactly `## [Unreleased]` and is the
only accepted undated form.

```bash
(
  set -euo pipefail
  VERSION=X.Y.Z
  notes_file=$(mktemp)
  trap 'rm -f "$notes_file"' EXIT
  bash tools/extract-release-notes.sh CHANGELOG.md "$VERSION" >"$notes_file"
  test -s "$notes_file"
  bash tools/check-release-security-notes.sh "$notes_file"
)
```

If the final refresh or validator finds a problem before tag push, correct the
release-preparation changes and repeat the final refresh and validation before
pushing a tag. If the tag-triggered validation fails or a required correction
is discovered after the tag is public, do not move or reuse that tag. First
determine whether crates.io or another channel partially published the original
version and clearly mark any partial GitHub Release. Correct the notes, then
cut a new patch version; the independent crates.io workflow may already have
published the original version.

## Release checklist

1. Update the version in `Cargo.toml` to match the tag exactly,
   stripping the `-dev` suffix that lives on `main` between releases:
   - Stable: `version = "0.2.0"` for tag `v0.2.0` (was `0.2.0-dev`)
   - Release candidate: `version = "0.2.0-rc5"` for tag `v0.2.0-rc5`
   - Beta / alpha: same pattern (`0.3.0-beta.1`, etc.)

   Cargo's semver supports prerelease identifiers, and `bzr --version` reads from `Cargo.toml`, so keeping the two aligned means the binary always reports the same string as the tag the user installed. The `installer-smoke` job in `release.yml` validates this on every tag push by comparing the running binary's `--version` output against the Cargo.toml field (substring match — the git-SHA suffix from `build.rs` does not affect it). Prerelease tags are not published to crates.io — `publish-crates.yml`'s `if: !contains(github.ref_name, '-')` gates them out.

   Run `cargo build` (or `cargo check`) after the bump so `Cargo.lock` regenerates with the new version.

2. If `rust-version` changed, update the MSRV badge in `README.md` and the "Requires Rust X+" line in the "From source" install section.
3. Add a matching dated entry to `CHANGELOG.md`. Entries must use
   `## [X.Y.Z] - YYYY-MM-DD` exactly. The shared extractor stops publication when it cannot select
   that exact dated heading. For prereleases, use the full prerelease version
   (`## [0.2.0-rc5] - 2026-05-04`).
4. Verify installation docs still match the published crate name.
5. Run the release checks locally:

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
cargo build --release
cargo publish --dry-run
```

6. Commit the release changes on a `release/vX.Y.Z-prep` branch:

```bash
git checkout -b release/vX.Y.Z-prep
git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "chore: bump version to X.Y.Z"
# If the CHANGELOG date is a separate commit, that's fine:
git commit -m "chore: date CHANGELOG entry for vX.Y.Z"
git push -u origin release/vX.Y.Z-prep
```

The project blocks direct pushes to `main` (a pre-commit-style guardrail mirrored by the `Never push directly to main` rule). Release prep follows the same PR-based flow as feature work — see the v0.2.0 lineage (`git log --first-parent v0.2.0` shows the tag landed on `Merge pull request #131 from randomparity/release/v0.2.0-prep`).

7. Open a `release: prep vX.Y.Z` PR against `main`. Wait for CI to go green,
   then merge it. The resulting merge commit on `main` is what you tag.

8. Run the `Functional Tests` GitHub Actions workflow on `main` and wait for
   success before tagging. It executes `tests/functional/run-all-versions.sh`
   against the real Bugzilla bz50, bz52, and bz53 containers.

9. Immediately before pushing the tag, perform the final advisory refresh and
   run the security-assessment extraction and validation command above. Resolve
   any result before continuing.

10. Tag the merge commit and push the tag:

```bash
git checkout main
git pull --ff-only origin main
git tag -a vX.Y.Z -m "bzr vX.Y.Z"
git push origin vX.Y.Z
```

Pushing the tag triggers `release.yml` (multi-arch build, GitHub Release, installer smoke, Homebrew tap bump on stable tags) and `publish-crates.yml` (crates.io publish on stable tags). Both workflows verify the tag version matches `Cargo.toml`.

## What automation does

### GitHub release binaries

Pushing a `v*` tag triggers `.github/workflows/release.yml`, which:

- Generates manpages once via `cargo run -p xtask -- man` and shares them across all targets
- Builds release binaries for supported platforms
- Packages each binary with `LICENSE`, `README.md`, and `man/man1/*.1` into a tarball or zip
- Builds `.deb` packages for `x86_64`, `aarch64`, and `ppc64el` Linux targets
- Builds `.rpm` packages for `x86_64`, `aarch64`, `ppc64le`, and `s390x` Linux targets
- Runs `lintian` and `rpmlint` (warn-only) on the resulting packages
- Smoke-tests `dpkg`/`dnf` install of the x86_64 packages in containers
- Generates a `SHA256SUMS` file covering every artifact (tarballs, zips, `.deb`, `.rpm`) for download verification
- Stages `install.sh` and `install.ps1` as release assets, with the release
  tag baked into the `BZR_VERSION` default
- Runs an `installer-smoke` job after the release that re-runs each script
  against the real GitHub Releases CDN (Ubuntu and Windows runners) and
  verifies `bzr --version` matches the released tag
- Creates a GitHub Release for the tag with all artifacts attached

If you change runtime dependencies (e.g., add a new linked native library), update the
`depends`/`requires` fields in `[package.metadata.deb]` and
`[package.metadata.generate-rpm]` in `Cargo.toml`. The defaults declare `libc6`,
`libdbus-1-3` (Debian) and `glibc`, `dbus-libs` (RPM) for the keyring backend.

To regenerate manpages locally:

```bash
make man    # writes to man/man1/
```

### Homebrew tap

The `homebrew` job in `release.yml` runs after the `release` job on stable
(non-prerelease) `v*` tags only (`if: !contains(github.ref_name, '-')`). It:

- Downloads the just-published release tarballs for macOS arm64 and Linux
  x86_64/aarch64
- Computes their SHA256 sums (and the GitHub source-tarball sum for the
  Intel-Mac source-build branch of the formula)
- Renders `homebrew/bzr.rb.template` into `Formula/bzr.rb` in
  [`randomparity/homebrew-tap`](https://github.com/randomparity/homebrew-tap)
- Commits and pushes the bumped formula

This job needs a `HOMEBREW_TAP_TOKEN` repo secret — a fine-grained PAT with
`Contents: Write` on `randomparity/homebrew-tap`. See
[`homebrew/README.md`](homebrew/README.md) for one-time tap setup.

The bump used to live in a separate `update-homebrew.yml` workflow that
triggered on `release: published`, but events emitted by the default
`GITHUB_TOKEN` do not fire downstream workflows (a GitHub Actions
safeguard against workflow recursion). v0.2.0's tap bump never ran as a
result, and the workflow was folded into `release.yml` here so the bump
runs directly in the release pipeline rather than waiting on a release
event that never arrives.

**Future goal:** publish to `homebrew-core` so users don't need a custom tap. Defer
until `bzr` has a stable track record across multiple releases.

### crates.io publish

Pushing a `v*` tag also triggers `.github/workflows/publish-crates.yml`, which:

- Verifies the tag version matches `Cargo.toml`
- Runs `cargo publish --dry-run`
- Publishes with `cargo publish --locked`

This workflow requires the repository secret `CARGO_REGISTRY_TOKEN`.

Create the token from crates.io and store it in GitHub Actions secrets as:

- `CARGO_REGISTRY_TOKEN`

## Recommended release order

1. Open and merge the `release/vX.Y.Z-prep` PR to `main`
2. Run the `Functional Tests` workflow on `main` and wait for success
3. Pull `main` locally and tag the merge commit (`git tag -a vX.Y.Z -m "bzr vX.Y.Z"`)
4. Push the tag (`git push origin vX.Y.Z`)
5. Confirm `release.yml` succeeds (preflight → manpages → build → release → installer-smoke → homebrew)
6. Confirm `publish-crates.yml` succeeds (stable tags only)
7. Verify installation from crates.io:

```bash
cargo install bzr --version X.Y.Z --locked
bzr --version
```

7. The `post-release-bump` job in `release.yml` will automatically
   open a `chore/bump-to-vX.Y.Z'-dev` PR after the `release` and
   `installer-smoke` jobs succeed (stable releases only — pre-release
   tags skip this step). The auto-PR:
   - Bumps `Cargo.toml` from the just-released `X.Y.Z` to the next
     **patch** version with a `-dev` suffix (e.g. `0.4.0` →
     `0.4.1-dev`). Patch is the safest default; if the next planned
     release is a minor or major bump (e.g. `0.4.0` → `0.5.0-dev`),
     edit the version on the PR before merging.
   - Refreshes `Cargo.lock` via `cargo update -p bzr`.
   - Prepends an empty `## [Unreleased]` section to `CHANGELOG.md`,
     ready for subsequent feature/fix PRs to extend.

   **CI on the auto-PR.** Pull requests opened by `GITHUB_TOKEN` do
   not trigger downstream `pull_request` workflows (the same
   safeguard that pushed the homebrew bump back into `release.yml`).
   The auto-PR's body contains a kick-off recipe — push an empty
   commit, or run the suite locally — so CI signal is available
   before merge. The bump itself is a 3-line diff, so the local-run
   path is usually fastest.

   If for any reason the job fails or the auto-PR is closed without
   merging, open a hand-crafted bump PR following the same shape
   (`chore: bump version to X.Y.Z'-dev after vX.Y.Z release`).
   `main` carrying the released version (no `-dev`) past a release
   is not catastrophic — the next release-prep PR will still land
   cleanly — but it does mean dev builds reuse the released version
   in `bzr --version`.

## If publish fails

- If crates.io rejects the name, rename the package and update the install docs
- If crates.io rejects the version, bump the version and retag
- If the tag does not match `Cargo.toml`, update one or the other so they match exactly
