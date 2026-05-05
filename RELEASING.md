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

## Release checklist

1. Update the version in `Cargo.toml` to match the tag exactly, including any prerelease suffix:
   - Stable: `version = "0.2.0"` for tag `v0.2.0`
   - Release candidate: `version = "0.2.0-rc5"` for tag `v0.2.0-rc5`
   - Beta / alpha: same pattern (`0.3.0-beta.1`, etc.)

   Cargo's semver supports prerelease identifiers, and `bzr --version` reads from `Cargo.toml`, so keeping the two aligned means the binary always reports the same string as the tag the user installed. The `installer-smoke` job in `release.yml` validates this on every tag push by comparing the running binary's `--version` output against the Cargo.toml field. Prerelease tags are not published to crates.io — `publish-crates.yml`'s `if: !contains(github.ref_name, '-')` gates them out.

   Run `cargo build` (or `cargo check`) after the bump so `Cargo.lock` regenerates with the new version.

2. If `rust-version` changed, update the MSRV badge in `README.md` and the "Requires Rust X+" line in the "From source" install section.
3. Add a matching dated entry to `CHANGELOG.md`. Entries must use `## [X.Y.Z] - YYYY-MM-DD` exactly — `release.yml` extracts release notes by matching that heading format, and a different format silently produces an empty release body. For prereleases, use the full prerelease version (`## [0.2.0-rc5] - 2026-05-04`).
4. Verify installation docs still match the published crate name.
5. Run the release checks locally:

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
cargo build --release
cargo publish --dry-run
```

6. Commit the release changes.
7. Create and push a version tag:

```bash
git tag vX.Y.Z
git push origin main
git push origin vX.Y.Z
```

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

1. Merge the release commit to `main`
2. Push the `vX.Y.Z` tag
3. Confirm the crates.io publish workflow succeeds
4. Confirm the GitHub release workflow succeeds
5. Verify installation from crates.io:

```bash
cargo install bzr --version X.Y.Z
bzr --version
```

## If publish fails

- If crates.io rejects the name, rename the package and update the install docs
- If crates.io rejects the version, bump the version and retag
- If the tag does not match `Cargo.toml`, update one or the other so they match exactly
