# Installer scripts (`install.sh` / `install.ps1`)

**Status:** Approved (brainstorming, 2026-05-04)
**Author:** randomparity
**Target release:** post v0.2.0

## Problem

`bzr` ships pre-built binaries for 7 targets via GitHub Releases, but the
README's install guidance is "download an archive and put the binary on
your PATH yourself". This is a real friction point — especially on
Windows, where users without `cargo` have no other install path. We want
the canonical `curl <url> | sh` (Unix) and `irm <url> | iex` (Windows)
flow that every comparable CLI tool offers, with enough verification
that the pipe-to-shell pattern is defensible.

## Goals

1. One-line install on Linux, macOS (Apple Silicon), and Windows.
2. SHA-256 verification of the downloaded archive against a published
   sums file, so a CDN/MITM attacker can't swap binaries silently.
3. No surprise mutations: the installer never edits dotfiles, registry,
   or system PATH. It places the binary and tells the user how to
   make it discoverable.
4. Predictable pinning: a user can lock both the script *and* the
   binary version to a specific release tag.
5. Clear, actionable failure modes for unsupported platforms (Intel
   Mac, FreeBSD, 32-bit Windows, etc.).

## Non-goals (explicitly out of scope)

- Adding `x86_64-apple-darwin` to the release matrix. The installer
  surfaces the gap; the matrix change is a separate ticket.
- PATH mutation by default. No `--add-to-path` flag in v1.
- Signing (cosign / minisign / GPG). SHA-256 sums are the bar.
- Homebrew formula, Scoop manifest, AUR, `.deb`/`.rpm` packaging.
- A `bzr self update` subcommand.
- Statically-linked Linux binaries (musl) to remove the `libdbus-1`
  runtime dependency.
- Locale-sensitive output / i18n.

## Architecture

Two POSIX/PowerShell scripts at the repo root, plus narrowly-scoped
changes to the release workflow and docs:

```
bzr/
├── install.sh                 # NEW — POSIX sh, Linux + macOS
├── install.ps1                # NEW — PowerShell 5.1+, Windows
├── .github/workflows/
│   ├── ci.yml                 # MOD — add lint + smoke jobs for installers
│   └── release.yml            # MOD — SHA256SUMS, version pin, asset upload, smoke
├── tests/installer/
│   ├── smoke.sh               # NEW — local-server smoke test for install.sh
│   └── smoke.ps1              # NEW — local-server smoke test for install.ps1
├── docs/installation.md       # NEW — long-form install reference
├── README.md                  # MOD — replace "Pre-built binaries" with one-liners
├── CHANGELOG.md               # MOD — add Unreleased entry
└── RELEASING.md               # MOD — note one-time manual smoke before merge
```

Both installers follow the same data flow:

1. Detect OS + arch → map to one of the 7 release targets.
2. Resolve version (env override → baked-in default → GitHub API
   "latest").
3. Resolve install dir (env override → platform default).
4. Download archive + `bzr-<tag>-SHA256SUMS` from
   `releases/download/<tag>/`.
5. Verify the archive's SHA-256 against the sums file; abort on
   mismatch.
6. Extract, move binary into place, set executable bit (Unix only).
7. Run `bzr --version` as a smoke check.
8. Print a PATH hint if the install dir isn't on PATH.

Unsupported platforms abort at step 1 with an error pointing at
`cargo install bzr --locked`.

### Component boundaries

- **`install.sh`** depends on: `curl` *or* `wget`, `tar`, `sha256sum`
  *or* `shasum -a 256`, `mktemp`, `uname`, POSIX `sh`. No bash-isms.
- **`install.ps1`** depends on: PowerShell 5.1+ built-ins only
  (`Invoke-WebRequest`, `Get-FileHash`, `Expand-Archive`,
  `[Net.ServicePointManager]`).
- **`release.yml`** is the only place that knows about the version
  pin and the SHA256SUMS layout. The scripts read what the workflow
  publishes; the workflow doesn't reach into the scripts beyond a
  single, comment-marked line each.
- **`docs/installation.md`** is the authoritative reference;
  `README.md` shows only the one-liners and links out.

## `install.sh` — bash/POSIX installer

**Header:** `#!/bin/sh` with `set -eu`. Pipe-safe (`curl ... | sh`).

### Target detection

| `uname -s` | `uname -m`              | Release target                    |
| ---------- | ----------------------- | --------------------------------- |
| Linux      | `x86_64`                | `x86_64-unknown-linux-gnu`        |
| Linux      | `aarch64` or `arm64`    | `aarch64-unknown-linux-gnu`       |
| Linux      | `ppc64le`               | `powerpc64le-unknown-linux-gnu`   |
| Linux      | `s390x`                 | `s390x-unknown-linux-gnu`         |
| Darwin     | `arm64`                 | `aarch64-apple-darwin`            |
| Darwin     | `x86_64`                | unsupported → `cargo` hint, exit 2 |
| anything else            | unsupported → `cargo` hint, exit 2 |

### Dependencies

Probed at startup. Abort with a precise error naming the missing tool.

- HTTP: `curl` (preferred) or `wget` (fallback).
- Archive: `tar`.
- Hashing: `sha256sum` (Linux) or `shasum -a 256` (macOS, BSDs).
- Misc: `mktemp`, `uname`.

### Linux `libdbus-1` runtime hint

The default-feature binary dynamically links `libdbus-1.so.3`. After
copying the binary into place, run `bzr --version`; if it exits
non-zero with a `libdbus` error in stderr, print:

```
bzr is installed but cannot start because libdbus-1 is missing.
Install it with one of:
  Debian/Ubuntu:  sudo apt-get install libdbus-1-3
  Fedora/RHEL:    sudo dnf install dbus-libs
  Alpine:         apk add dbus-libs
Or rebuild without the keyring feature:
  cargo install bzr --locked --no-default-features
```

The install is still considered successful (exit 0) since the binary
is on disk.

### Environment variable overrides

- `BZR_VERSION` — release tag like `v0.2.0`.
  - Default for `main`-branch copy: query GitHub API for the latest
    release tag.
  - Default for release-asset copy: baked-in tag for that release
    (see "Release workflow → version pin" below).
- `BZR_INSTALL_DIR` — install directory.
  - Default: `$HOME/.local/bin` (created if missing).

### Flow

1. Probe deps → detect target → resolve version → resolve dir.
2. `mktemp -d` workdir; `trap 'rm -rf "$workdir"' EXIT`.
3. Download `bzr-<tag>-<target>.tar.gz` and `bzr-<tag>-SHA256SUMS`
   into workdir.
4. Verify: `grep " bzr-<tag>-<target>.tar.gz$" SHA256SUMS |
   sha256sum -c -` (or `shasum -a 256 -c -` on macOS).
5. `tar xzf <archive>`; copy `bzr-<tag>-<target>/bzr` to
   `$BZR_INSTALL_DIR/bzr`; `chmod 0755`.
6. Run `"$BZR_INSTALL_DIR/bzr" --version` (with libdbus hint on
   Linux failure).
7. PATH check: if `command -v bzr` doesn't resolve to the installed
   path, print:
   ```
   $BZR_INSTALL_DIR is not on your PATH. Add it to your shell rc:
     export PATH="$BZR_INSTALL_DIR:$PATH"
   ```

### Exit codes

| Code | Meaning                       |
| ---: | ----------------------------- |
|    0 | Success                       |
|    2 | Unsupported platform          |
|    3 | Missing required dependency   |
|    4 | Download failure (HTTP error) |
|    5 | Checksum mismatch             |
|    6 | Extraction failure            |

## `install.ps1` — PowerShell installer

**Header:** `Set-StrictMode -Version Latest`,
`$ErrorActionPreference = 'Stop'`. Pipe-safe (`irm ... | iex`).

### TLS prelude

PS 5.1 defaults to SSL3/TLS1.0 and silently fails against GitHub.
First executable line:

```powershell
[Net.ServicePointManager]::SecurityProtocol = `
  [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
```

### Target detection

| `PROCESSOR_ARCHITECTURE` (with `ARCHITEW6432` fallback) | Release target              |
| ------------------------------------------------------- | --------------------------- |
| `AMD64`                                                 | `x86_64-pc-windows-msvc`    |
| `ARM64`                                                 | `aarch64-pc-windows-msvc`   |
| anything else                                           | unsupported → `cargo` hint  |

The `ARCHITEW6432` check covers 32-bit PowerShell running on a 64-bit
Windows host; without it, ARM64 + x86 PowerShell mis-detects.

### Dependencies

Pure built-ins: `Invoke-WebRequest -UseBasicParsing`,
`Get-FileHash -Algorithm SHA256`, `Expand-Archive`. No external tools.

### Environment variable overrides

- `$env:BZR_VERSION` — release tag.
- `$env:BZR_INSTALL_DIR` — defaults to
  `Join-Path $env:LOCALAPPDATA 'Programs\bzr'`.

### Flow

1. TLS prelude → detect target → resolve version → resolve dir
   (create if missing).
2. Create temp dir; wrap remaining steps in `try`/`finally` to
   remove it.
3. `Invoke-WebRequest` to download
   `bzr-<tag>-<target>.zip` and `bzr-<tag>-SHA256SUMS` into temp dir.
4. Parse the sums file (lines are `<hex>  <filename>`), find the
   line for our archive, compare against
   `(Get-FileHash -Algorithm SHA256 $archive).Hash`. Case-insensitive.
5. `Expand-Archive` the zip into temp dir; copy
   `bzr-<tag>-<target>\bzr.exe` to `$BzrInstallDir\bzr.exe`
   (overwrite if present).
6. Run `& "$BzrInstallDir\bzr.exe" --version` as smoke test.
7. PATH check via `[Environment]::GetEnvironmentVariable('Path',
   'User')` and the process PATH; if neither contains the install
   dir, print:
   ```
   $BzrInstallDir is not on your PATH. Add it (current user, persistent):
     [Environment]::SetEnvironmentVariable('Path',
       [Environment]::GetEnvironmentVariable('Path','User') + ';$BzrInstallDir',
       'User')
   ```

### Exit codes

Mirror the bash installer except code 3 (missing dep) is unused —
all dependencies are PowerShell built-ins.

### Execution policy

`irm ... | iex` works regardless of execution policy because `iex`
evaluates a string; the script never touches disk in the canonical
invocation. The downloaded `.ps1` file *would* hit policy if saved
and run directly — covered in the troubleshooting doc.

## Release workflow changes (`release.yml`)

One precursor fix to the per-target build matrix, plus three
additions to the `release` job.

### 0. Make archive layouts symmetric (precursor fix)

Today the two packaging steps produce asymmetric layouts:

- Unix: `tar czf "$STAGING.tar.gz" "$STAGING"` — tarball wraps
  contents in a `bzr-<tag>-<target>/` directory.
- Windows: `Compress-Archive -Path "$STAGING\*"` — zip contains
  `bzr.exe`, `LICENSE`, `README.md` at the root, with no wrapper
  directory.

Change the Windows step to drop the `\*`:

```yaml
Compress-Archive -Path $STAGING -DestinationPath "$STAGING.zip"
```

Both archive types now extract to `bzr-<tag>-<target>/{bzr[.exe],
LICENSE, README.md}`. This lets `install.sh` and `install.ps1`
share the same "look in the wrapper directory" extraction logic
described in their respective flow sections. Pre-existing
prerelease zips (`v0.2.0-rc[12]`) keep their old layout but are
not used by the installer.

### 1. Generate `bzr-<tag>-SHA256SUMS`

After `actions/download-artifact@…` populates `artifacts/`:

```yaml
- name: Generate SHA256SUMS
  working-directory: artifacts
  run: |
    set -euo pipefail
    find . -maxdepth 1 -type f \( -name '*.tar.gz' -o -name '*.zip' \) -printf '%f\n' \
      | sort \
      | xargs -I{} sha256sum {} > "bzr-${GITHUB_REF_NAME}-SHA256SUMS"
```

One sums file covers all 7 archives. Filename matches the archive
prefix for easy globbing.

### 2. Stage installer scripts with version baked in

Both scripts have a single, comment-marked line that the workflow
rewrites:

```sh
# RELEASE_VERSION_PIN — release.yml rewrites the next line at release time
BZR_VERSION="${BZR_VERSION:-}"
```

```powershell
# RELEASE_VERSION_PIN -- release.yml rewrites the next line at release time
$BzrVersion = $env:BZR_VERSION
```

The release step copies the scripts into `artifacts/` and runs `sed`
to bake in `${GITHUB_REF_NAME}` as the default. Then a verification
step asserts the rewrite hit:

```yaml
- name: Verify version pin succeeded
  working-directory: artifacts
  run: |
    set -euo pipefail
    grep -q "BZR_VERSION:-${GITHUB_REF_NAME}" install.sh
    grep -q "BzrVersion.*${GITHUB_REF_NAME}" install.ps1
```

If `sed` produces a no-op (line moved or renamed), this fails the
release rather than shipping a broken asset.

### 3. Upload everything

Existing final step is `gh release create … artifacts/*`. With
`SHA256SUMS`, `install.sh`, and `install.ps1` now in `artifacts/`,
no glob change is needed. `--prerelease` still applies based on tag
suffix.

Final asset count per release: 7 archives + 1 sums file + 2 scripts
= 10 assets.

## Documentation changes

### `docs/installation.md` (new)

Authoritative install reference. Sections:

1. **Quick install** — both one-liners, copy-pasteable.
2. **What the installer does** — 4-bullet summary.
3. **Supported platforms** — 7-row target table; "Intel Mac → use
   `cargo install`" callout.
4. **Customizing the install** — env-var table with examples
   (`BZR_VERSION`, `BZR_INSTALL_DIR`).
5. **Pinning to a specific release** — show
   `releases/download/<tag>/install.sh` form.
6. **Verifying manually** — for users who'd rather not pipe to a
   shell. One block per OS showing the manual `sha256sum -c` /
   `Get-FileHash` flow.
7. **Other install methods** — link out to README sections for
   crates.io and source builds. No duplication.
8. **Uninstall** — one-liner per OS. Note that no extra state is
   created (config at `~/.config/bzr/` is the user's to manage).
9. **Troubleshooting** — checksum mismatch, missing `libdbus-1`,
   GitHub API rate-limiting (point at `BZR_VERSION` override),
   Windows execution policy.

### `README.md` (modified)

Replace the current "Pre-built binaries" subsection (lines 33–40)
with the two one-liners, a 4-line summary of installer behavior, and
a link to `docs/installation.md`. Keep the existing platform list
and Intel Mac note. Keep the GitHub Releases archive link as a
fallback. Add a one-line mention of the installer in the
"Quickstart → 1. Install bzr" section (around line 88).

### `docs/bzr-cli.md` (no change)

Installer adds no CLI surface.

### `CHANGELOG.md` (modified)

Add under `[Unreleased] / Added`:

```markdown
- Installer scripts (`install.sh`, `install.ps1`) for one-line
  installation from GitHub Releases, with SHA-256 verification.
```

### `RELEASING.md` (modified)

Add a one-time bootstrap note: before merging this feature, manually
run each script from a clean macOS / Linux / Windows host against
the next prerelease tag (`v0.2.0-rcN`) to validate end-to-end.
Thereafter, CI carries the load.

## Testing

### CI lint (every PR, path-filtered)

- `shellcheck -s sh install.sh tests/installer/smoke.sh`
- `shfmt -d -ln posix -i 2 install.sh tests/installer/smoke.sh`
- `Invoke-ScriptAnalyzer install.ps1 tests/installer/smoke.ps1
  -Severity Warning` via the `microsoft/psscriptanalyzer-action`
  (pinned to SHA, per repo convention; warnings treated as failures).

### CI smoke tests (every PR, 3-platform matrix)

New `installer-test` job in `ci.yml` running on `ubuntu-latest`,
`macos-14`, and `windows-latest`. Each platform runs its installer
against a local HTTP server (Python `http.server`) serving fixture
archives + a fixture `SHA256SUMS`. Cases:

- Success path
- Checksum mismatch (corrupt one byte) → exit 5
- Unsupported target (stub `uname` on PATH) → exit 2
- Missing dep (bash only — temporarily move `tar` aside) → exit 3

Tests never touch the real GitHub Releases API and never modify the
host beyond a tempdir.

### Release smoke test (post-tag)

New `installer-smoke` job in `release.yml`, depending on `release`.
For each of `ubuntu-latest` and `windows-latest`:

1. Download `install.sh` / `install.ps1` from the just-created
   release.
2. Run it against the real GitHub Releases CDN.
3. Assert `bzr --version` returns the freshly-released tag.

Runs on **all** tags including prereleases — that's exactly when
templating regressions matter most.

### One-time manual validation (pre-merge)

Documented in `RELEASING.md`. Before merging this feature, run each
script from a clean host on a real prerelease tag.

## Security considerations

- HTTPS to GitHub releases provides transport security. SHA-256 sums
  protect against post-build tampering of the archive (a CDN swap or
  GH Actions cache poisoning would still need the sums file to also
  be wrong, which is detectable in `installer-smoke`).
- The installer never runs downloaded code other than the binary
  itself, and only after `--version` succeeds.
- The installer never writes to `$HOME` outside `$BZR_INSTALL_DIR`
  (no rc-file edits) and never writes to system locations.
- The `main`-branch script is mutable; users who want frozen install
  logic can pin via `releases/download/<tag>/install.sh`.

## Risks and open questions

- **GitHub API rate limit on `latest`:** unauthenticated requests
  are 60/hr per IP. Shared-NAT environments could hit this. Mitigation:
  clear error pointing at `BZR_VERSION` override.
- **`libdbus-1` runtime gap on minimal Linux:** documented and
  surfaced at install time. Not blocking, but a real source of
  user friction until/unless we ship musl builds.
- **Windows ARM64 untested in CI:** GitHub does not yet offer ARM64
  Windows runners. We trust the build artifact and rely on user
  reports.

## Implementation order

1. Fix `release.yml` Windows packaging to produce a symmetric
   zip layout (precursor; can ship on its own).
2. Add `install.sh` and `install.ps1` with full target-detection,
   download, verify, install, and PATH-hint logic. Local
   smoke-test scaffolding (`tests/installer/`).
3. Wire CI lint + smoke jobs in `ci.yml`.
4. Update `release.yml` (release job): SHA256SUMS, version pin,
   asset upload, pin-verification, post-release smoke job.
5. Add `docs/installation.md`. Update `README.md`, `CHANGELOG.md`,
   `RELEASING.md`.
6. Cut a prerelease tag (`v0.2.1-rc1` or similar), run the one-time
   manual validation, then merge.
