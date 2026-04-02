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

1. Update the version in `Cargo.toml`.
2. Add a matching dated entry to `CHANGELOG.md`.
3. Verify installation docs still match the published crate name.
4. Run the release checks locally:

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
cargo build --release
cargo publish --dry-run
```

5. Commit the release changes.
6. Create and push a version tag:

```bash
git tag vX.Y.Z
git push origin main
git push origin vX.Y.Z
```

## What automation does

### GitHub release binaries

Pushing a `v*` tag triggers `.github/workflows/release.yml`, which:

- Builds release binaries for supported platforms
- Packages them with `LICENSE` and `README.md`
- Creates a GitHub Release for the tag

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
