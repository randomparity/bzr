# Homebrew tap

This directory holds the formula template that
[`.github/workflows/update-homebrew.yml`](../.github/workflows/update-homebrew.yml)
renders into `Formula/bzr.rb` in the
[`randomparity/homebrew-tap`](https://github.com/randomparity/homebrew-tap)
repository on each stable (non-prerelease) `v*` release.

## End-user install

```bash
brew tap randomparity/tap
brew install bzr
```

Supported pre-built platforms:

- macOS arm64 (Apple Silicon)
- Linux x86_64 (glibc)
- Linux aarch64 (glibc)

Intel Mac (`x86_64-apple-darwin`) builds from source via `cargo` and a
build-time `rust` dep. No prebuilt binary is published for Intel macOS.

## One-time tap setup

The first time the tap is published, do this manually (the workflow assumes the
tap repo and the formula already exist):

1. Create the tap repo:

   ```bash
   gh repo create randomparity/homebrew-tap --public \
     --description "Homebrew tap for randomparity projects"
   ```

2. Clone it, copy the rendered formula in, commit, and push:

   ```bash
   git clone https://github.com/randomparity/homebrew-tap
   cd homebrew-tap
   mkdir -p Formula
   # Render the template manually for the first release, e.g. with the same
   # sed pipeline the workflow uses, then commit it as Formula/bzr.rb.
   git add Formula/bzr.rb
   git commit -m "bzr X.Y.Z"
   git push
   ```

3. Create a fine-grained PAT with `Contents: Write` on `randomparity/homebrew-tap`
   and add it to this repo's secrets as `HOMEBREW_TAP_TOKEN`. The
   `update-homebrew.yml` workflow uses it to push subsequent updates.

After the one-time setup, every stable release auto-bumps the formula.

## Future: homebrew-core

A long-term goal is to publish `bzr` to homebrew-core so users don't need to
tap. homebrew-core has higher requirements:

- Stable, in-active-use, with verifiable maintainer
- No conflicts with existing formulas
- Submitted via PR to homebrew/homebrew-core

Defer until the project has a track record (multiple stable releases, real
user base, no outstanding regressions).

## Editing the formula

Update `bzr.rb.template` in this directory; the workflow re-renders it into
the tap on the next release. Keep placeholders (`{{VERSION}}`,
`{{MAC_ARM_SHA}}`, etc.) intact — the workflow's `sed` pass replaces them.
