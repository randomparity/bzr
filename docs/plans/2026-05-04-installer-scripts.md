# Installer Scripts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `install.sh` and `install.ps1` that one-line-install pre-built `bzr` binaries from GitHub Releases, with SHA-256 verification against the existing `SHA256SUMS` file, no PATH mutation, and a CI smoke-test harness on three platforms.

**Architecture:** Two pipe-safe scripts at the repo root. Each detects platform, downloads its target archive + `SHA256SUMS`, verifies the hash, extracts, and drops the binary in a user dir. `release.yml` bakes the release tag into a copy of each script and uploads them as release assets; the existing `SHA256SUMS` step is reordered to cover them. CI runs lint + a 3-platform smoke matrix against fixture archives served via `file://`.

**Tech Stack:** POSIX `sh` (no bash-isms), PowerShell 5.1+ (Windows built-ins only), GitHub Actions (workflow YAML), `shellcheck`/`shfmt`/`PSScriptAnalyzer` for lint.

**Implementation env-var contract** (used by tests, undocumented to end users):
- `BZR_BASE_URL` — overrides the default `https://github.com/randomparity/bzr/releases/download` base. Test fixtures point this at a `file://` URL.
- `BZR_SKIP_SMOKE=1` — skips the post-install `bzr --version` call. Tests use a stub binary that may not be a real executable on Windows.

Both are intentionally undocumented in user-facing docs.

---

## Phase 1: Precursor — Symmetric Windows zip layout

### Task 1: Fix Windows zip packaging in `release.yml`

The Windows packaging step uses `Compress-Archive -Path "$STAGING\*"`, which archives the *contents* of the staging dir. The Unix `tar` step archives the dir itself. The installer needs a symmetric layout (`bzr-<tag>-<target>/{bzr[.exe], LICENSE, README.md, man/man1/}` for both formats).

**Files:**
- Modify: `.github/workflows/release.yml:174`

- [ ] **Step 1: Read the current Windows packaging step**

Run: `grep -n -A 9 "Package (windows)" .github/workflows/release.yml`
Expected: shows the step with `Compress-Archive -Path "$STAGING\*" -DestinationPath "$STAGING.zip"` on the last line.

- [ ] **Step 2: Apply the one-line fix**

Edit `.github/workflows/release.yml`. Replace:

```yaml
          Compress-Archive -Path "$STAGING\*" -DestinationPath "$STAGING.zip"
```

with:

```yaml
          Compress-Archive -Path $STAGING -DestinationPath "$STAGING.zip"
```

- [ ] **Step 3: Lint the workflow**

Run: `actionlint .github/workflows/release.yml`
Expected: no output (success).

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "fix(release): wrap Windows zip contents in bzr-<tag>-<target>/

Compress-Archive with a trailing \\* archives the contents of the
staging directory; without it, the directory itself is archived.
Aligns the zip layout with the Unix tarball so both archive types
extract to bzr-<tag>-<target>/{bzr[.exe], LICENSE, README.md,
man/man1/}."
```

---

## Phase 2: `install.sh` (POSIX bash installer)

### Task 2: Add success-path smoke test

Build the test infrastructure first. The test creates a fake release archive + `SHA256SUMS` in a tempdir, points `BZR_BASE_URL` at `file://$tempdir`, runs `install.sh`, and asserts the binary lands at the expected path.

**Files:**
- Create: `tests/installer/smoke.sh`

- [ ] **Step 1: Create the smoke-test driver**

Create `tests/installer/smoke.sh`:

```sh
#!/bin/sh
# Smoke tests for install.sh. Runs against local file:// fixtures.
# Exits 0 if all sub-tests pass; non-zero on first failure.
set -eu

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
INSTALL_SH="$REPO_ROOT/install.sh"

if [ ! -f "$INSTALL_SH" ]; then
  echo "smoke: $INSTALL_SH not found" >&2
  exit 1
fi

WORKDIR="$(mktemp -d)"
cleanup() { rm -rf "$WORKDIR"; }
trap cleanup EXIT

mkfixtures() {
  # $1 = fixtures dir, $2 = tag, $3 = target
  fix="$1"; tag="$2"; target="$3"
  staging="bzr-$tag-$target"
  mkdir -p "$fix/$staging"
  cat > "$fix/$staging/bzr" <<'STUB'
#!/bin/sh
echo "bzr v0.0.0-test"
STUB
  chmod 0755 "$fix/$staging/bzr"
  echo "fake LICENSE" > "$fix/$staging/LICENSE"
  echo "fake README" > "$fix/$staging/README.md"
  (cd "$fix" && tar czf "$staging.tar.gz" "$staging" && rm -rf "$staging")
  # Generate SHA256SUMS over the tarball.
  if command -v sha256sum >/dev/null 2>&1; then
    (cd "$fix" && sha256sum ./*.tar.gz > SHA256SUMS)
  else
    (cd "$fix" && shasum -a 256 ./*.tar.gz > SHA256SUMS)
  fi
}

detect_native_target() {
  os="$(uname -s)"; arch="$(uname -m)"
  case "$os/$arch" in
    Linux/x86_64)  echo x86_64-unknown-linux-gnu ;;
    Linux/aarch64|Linux/arm64) echo aarch64-unknown-linux-gnu ;;
    Darwin/arm64)  echo aarch64-apple-darwin ;;
    *) echo "" ;;
  esac
}

test_success_path() {
  td="$WORKDIR/success"
  mkdir -p "$td"
  fixtures="$td/releases/v0.0.0-test"
  install_dir="$td/bin"
  mkdir -p "$fixtures"
  target="$(detect_native_target)"
  if [ -z "$target" ]; then
    echo "smoke: skipping success_path on unsupported host" >&2
    return 0
  fi
  mkfixtures "$fixtures" "v0.0.0-test" "$target"

  BZR_BASE_URL="file://$td/releases" \
  BZR_VERSION="v0.0.0-test" \
  BZR_INSTALL_DIR="$install_dir" \
  BZR_SKIP_SMOKE=1 \
    sh "$INSTALL_SH"

  [ -x "$install_dir/bzr" ] || { echo "smoke: bzr not installed at $install_dir/bzr" >&2; return 1; }
  echo "smoke: success_path OK"
}

test_success_path
echo "smoke: all sub-tests passed"
```

- [ ] **Step 2: Make it executable**

```bash
chmod +x tests/installer/smoke.sh
```

- [ ] **Step 3: Run it to verify it fails (install.sh doesn't exist yet)**

Run: `sh tests/installer/smoke.sh`
Expected: `smoke: /Users/.../install.sh not found` and exit non-zero.

- [ ] **Step 4: Commit**

```bash
git add tests/installer/smoke.sh
git commit -m "test(installer): add bash smoke harness with success-path case"
```

---

### Task 3: Implement `install.sh` (success path)

Make the smoke test pass. Minimal implementation: target detection, version resolution from env, archive download, SHA-256 verification, extract, install.

**Files:**
- Create: `install.sh`

- [ ] **Step 1: Create `install.sh`**

```sh
#!/bin/sh
# bzr installer for Linux and macOS.
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/randomparity/bzr/main/install.sh | sh
# Env vars:
#   BZR_VERSION       - release tag (default: latest stable)
#   BZR_INSTALL_DIR   - install directory (default: $HOME/.local/bin)
set -eu

# RELEASE_VERSION_PIN — release.yml rewrites the next line at release time
BZR_VERSION="${BZR_VERSION:-}"
BZR_INSTALL_DIR="${BZR_INSTALL_DIR:-$HOME/.local/bin}"

# Internal: undocumented test override for the GitHub releases base URL.
BZR_BASE_URL="${BZR_BASE_URL:-https://github.com/randomparity/bzr/releases/download}"
# Internal: undocumented test flag to skip the post-install `bzr --version` call.
BZR_SKIP_SMOKE="${BZR_SKIP_SMOKE:-}"

GITHUB_API="https://api.github.com/repos/randomparity/bzr/releases/latest"

err() { printf 'install.sh: %s\n' "$*" >&2; }

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || { err "missing required command: $1"; exit 3; }
}

detect_target() {
  os="$(uname -s)"; arch="$(uname -m)"
  case "$os/$arch" in
    Linux/x86_64)              echo x86_64-unknown-linux-gnu ;;
    Linux/aarch64|Linux/arm64) echo aarch64-unknown-linux-gnu ;;
    Linux/ppc64le)             echo powerpc64le-unknown-linux-gnu ;;
    Linux/s390x)               echo s390x-unknown-linux-gnu ;;
    Darwin/arm64)              echo aarch64-apple-darwin ;;
    *) return 1 ;;
  esac
}

http_get() {
  # $1 = url, $2 = destination path
  if command -v curl >/dev/null 2>&1; then
    curl --fail --silent --show-error --location "$1" -o "$2"
  elif command -v wget >/dev/null 2>&1; then
    wget --quiet "$1" -O "$2"
  else
    err "neither curl nor wget is installed"
    exit 3
  fi
}

resolve_version() {
  if [ -n "$BZR_VERSION" ]; then
    echo "$BZR_VERSION"; return
  fi
  tmpfile="$(mktemp)"
  if ! http_get "$GITHUB_API" "$tmpfile" 2>/dev/null; then
    err "failed to query GitHub API for the latest release"
    err "set BZR_VERSION=vX.Y.Z to pin to a specific tag"
    rm -f "$tmpfile"
    exit 4
  fi
  tag="$(grep '"tag_name"' "$tmpfile" | sed -E 's/.*"tag_name":[[:space:]]*"([^"]+)".*/\1/' | head -n 1)"
  rm -f "$tmpfile"
  if [ -z "$tag" ]; then
    err "could not parse tag_name from GitHub API response"
    exit 4
  fi
  echo "$tag"
}

verify_sha256() {
  # $1 = sums file (relative paths), $2 = filename to verify, $3 = working dir
  sums="$1"; fname="$2"; dir="$3"
  line="$(grep "  $fname$" "$sums" || true)"
  if [ -z "$line" ]; then
    err "checksum line not found for $fname in $sums"
    exit 5
  fi
  if command -v sha256sum >/dev/null 2>&1; then
    echo "$line" | (cd "$dir" && sha256sum -c -)
  else
    echo "$line" | (cd "$dir" && shasum -a 256 -c -)
  fi
}

main() {
  require_cmd uname
  require_cmd mktemp
  require_cmd tar
  command -v curl >/dev/null 2>&1 || command -v wget >/dev/null 2>&1 || {
    err "neither curl nor wget is installed"; exit 3;
  }
  command -v sha256sum >/dev/null 2>&1 || command -v shasum >/dev/null 2>&1 || {
    err "neither sha256sum nor shasum is installed"; exit 3;
  }

  target="$(detect_target)" || {
    err "unsupported platform: $(uname -s)/$(uname -m)"
    err "Try one of:"
    err "  - cargo install bzr --locked"
    err "  - your distro's .deb or .rpm from the GitHub release page"
    err "  - Homebrew: brew tap randomparity/tap && brew install bzr"
    exit 2
  }

  tag="$(resolve_version)"
  archive="bzr-$tag-$target.tar.gz"
  archive_url="$BZR_BASE_URL/$tag/$archive"
  sums_url="$BZR_BASE_URL/$tag/SHA256SUMS"

  workdir="$(mktemp -d)"
  trap 'rm -rf "$workdir"' EXIT INT TERM

  printf 'install.sh: downloading %s\n' "$archive_url" >&2
  http_get "$archive_url" "$workdir/$archive" || { err "download failed: $archive_url"; exit 4; }
  http_get "$sums_url"    "$workdir/SHA256SUMS" || { err "download failed: $sums_url"; exit 4; }

  verify_sha256 "$workdir/SHA256SUMS" "$archive" "$workdir" || { err "SHA-256 verification failed"; exit 5; }

  (cd "$workdir" && tar xzf "$archive") || { err "tar extraction failed"; exit 6; }

  mkdir -p "$BZR_INSTALL_DIR"
  cp "$workdir/bzr-$tag-$target/bzr" "$BZR_INSTALL_DIR/bzr"
  chmod 0755 "$BZR_INSTALL_DIR/bzr"

  if [ -z "$BZR_SKIP_SMOKE" ]; then
    if ! "$BZR_INSTALL_DIR/bzr" --version >/dev/null 2>&1; then
      err "bzr installed at $BZR_INSTALL_DIR/bzr but failed to run."
      err "If this is Linux and the error mentions libdbus, install the runtime lib:"
      err "  Debian/Ubuntu:  sudo apt-get install libdbus-1-3"
      err "  Fedora/RHEL:    sudo dnf install dbus-libs"
      err "  Alpine:         apk add dbus-libs"
      err "Or use the .deb/.rpm package, which declares libdbus as a runtime dep."
      err "Or rebuild without the keyring feature:"
      err "  cargo install bzr --locked --no-default-features"
    fi
  fi

  printf 'install.sh: installed bzr to %s/bzr\n' "$BZR_INSTALL_DIR"
  resolved="$(command -v bzr 2>/dev/null || true)"
  if [ "$resolved" != "$BZR_INSTALL_DIR/bzr" ]; then
    cat >&2 <<EOF
install.sh: $BZR_INSTALL_DIR is not on your PATH. Add it to your shell rc:
  export PATH="$BZR_INSTALL_DIR:\$PATH"
EOF
  fi
}

main "$@"
```

- [ ] **Step 2: Make it executable**

```bash
chmod +x install.sh
```

- [ ] **Step 3: Run smoke test, expect pass**

Run: `sh tests/installer/smoke.sh`
Expected:
```
install.sh: downloading file:///.../v0.0.0-test/bzr-v0.0.0-test-...tar.gz
bzr-v0.0.0-test-...tar.gz: OK
install.sh: installed bzr to /tmp/.../bin/bzr
smoke: success_path OK
smoke: all sub-tests passed
```

- [ ] **Step 4: Run shellcheck**

Run: `shellcheck -s sh install.sh tests/installer/smoke.sh`
Expected: no output (success).

- [ ] **Step 5: Commit**

```bash
git add install.sh tests/installer/smoke.sh
git commit -m "feat(installer): add install.sh with target detection and SHA256 verify"
```

---

### Task 4: Add checksum-mismatch test → already covered

Verify the existing implementation already handles this case end-to-end (it does — the `sha256sum -c` invocation will fail on mismatch). Add the test case to lock the behavior in.

**Files:**
- Modify: `tests/installer/smoke.sh`

- [ ] **Step 1: Append the failure-path test**

Add the following before the final `test_success_path` invocation block at the bottom of `tests/installer/smoke.sh` (so the helper is defined when called):

```sh
test_checksum_mismatch() {
  td="$WORKDIR/checksum"
  mkdir -p "$td"
  fixtures="$td/releases/v0.0.0-test"
  install_dir="$td/bin"
  mkdir -p "$fixtures"
  target="$(detect_native_target)"
  if [ -z "$target" ]; then
    echo "smoke: skipping checksum_mismatch on unsupported host" >&2
    return 0
  fi
  mkfixtures "$fixtures" "v0.0.0-test" "$target"
  # Corrupt the archive after sums were generated.
  echo "tampered" >> "$fixtures/bzr-v0.0.0-test-$target.tar.gz"

  set +e
  BZR_BASE_URL="file://$td/releases" \
  BZR_VERSION="v0.0.0-test" \
  BZR_INSTALL_DIR="$install_dir" \
  BZR_SKIP_SMOKE=1 \
    sh "$INSTALL_SH" >/dev/null 2>&1
  rc=$?
  set -e

  if [ "$rc" != "5" ]; then
    echo "smoke: expected exit 5 (checksum mismatch), got $rc" >&2
    return 1
  fi
  if [ -e "$install_dir/bzr" ]; then
    echo "smoke: bzr should NOT be installed when checksum fails" >&2
    return 1
  fi
  echo "smoke: checksum_mismatch OK"
}
```

And update the runner block at the bottom:

```sh
test_success_path
test_checksum_mismatch
echo "smoke: all sub-tests passed"
```

- [ ] **Step 2: Run smoke test**

Run: `sh tests/installer/smoke.sh`
Expected: both sub-tests pass; final line `smoke: all sub-tests passed`.

- [ ] **Step 3: Commit**

```bash
git add tests/installer/smoke.sh
git commit -m "test(installer): add checksum-mismatch case to bash smoke"
```

---

### Task 5: Add unsupported-platform test

Verify exit 2 with the platform-detection error and no install.

**Files:**
- Modify: `tests/installer/smoke.sh`

- [ ] **Step 1: Append the unsupported-platform test**

Add to `tests/installer/smoke.sh` (above the runner block):

```sh
test_unsupported_target() {
  td="$WORKDIR/unsupported"
  mkdir -p "$td"
  install_dir="$td/bin"
  mkdir -p "$td/stub"
  # Stub `uname` that reports an unsupported arch.
  cat > "$td/stub/uname" <<'STUB'
#!/bin/sh
case "$1" in
  -s) echo Linux ;;
  -m) echo riscv64 ;;
  *)  echo Linux ;;
esac
STUB
  chmod +x "$td/stub/uname"

  set +e
  PATH="$td/stub:$PATH" \
  BZR_VERSION="v0.0.0-test" \
  BZR_INSTALL_DIR="$install_dir" \
  BZR_SKIP_SMOKE=1 \
    sh "$INSTALL_SH" >/dev/null 2>&1
  rc=$?
  set -e

  if [ "$rc" != "2" ]; then
    echo "smoke: expected exit 2 (unsupported), got $rc" >&2
    return 1
  fi
  echo "smoke: unsupported_target OK"
}
```

Update the runner block:

```sh
test_success_path
test_checksum_mismatch
test_unsupported_target
echo "smoke: all sub-tests passed"
```

- [ ] **Step 2: Run smoke test**

Run: `sh tests/installer/smoke.sh`
Expected: three sub-tests pass.

- [ ] **Step 3: Commit**

```bash
git add tests/installer/smoke.sh
git commit -m "test(installer): add unsupported-platform case to bash smoke"
```

---

### Task 6: Add missing-dependency test

Verify exit 3 when a required dependency (`tar`) isn't available.

**Files:**
- Modify: `tests/installer/smoke.sh`

- [ ] **Step 1: Append the missing-dep test**

Add to `tests/installer/smoke.sh` (above the runner block):

```sh
test_missing_dep() {
  td="$WORKDIR/missingdep"
  mkdir -p "$td"
  install_dir="$td/bin"
  # Build a minimal PATH that excludes tar but keeps everything else.
  # We do this by pointing PATH at a curated list of dirs known to contain
  # required tools but never tar (we'll skip if tar is the only path that
  # contains it on this host).
  mkdir -p "$td/sandbox"
  for tool in sh uname mktemp curl wget sha256sum shasum sed grep cp mkdir chmod head rm; do
    src="$(command -v "$tool" 2>/dev/null || true)"
    [ -n "$src" ] && ln -s "$src" "$td/sandbox/$tool"
  done

  set +e
  PATH="$td/sandbox" \
  BZR_VERSION="v0.0.0-test" \
  BZR_INSTALL_DIR="$install_dir" \
  BZR_SKIP_SMOKE=1 \
    sh "$INSTALL_SH" >/dev/null 2>&1
  rc=$?
  set -e

  if [ "$rc" != "3" ]; then
    echo "smoke: expected exit 3 (missing dep), got $rc" >&2
    return 1
  fi
  echo "smoke: missing_dep OK"
}
```

Update the runner block:

```sh
test_success_path
test_checksum_mismatch
test_unsupported_target
test_missing_dep
echo "smoke: all sub-tests passed"
```

- [ ] **Step 2: Run smoke test**

Run: `sh tests/installer/smoke.sh`
Expected: four sub-tests pass.

- [ ] **Step 3: Commit**

```bash
git add tests/installer/smoke.sh
git commit -m "test(installer): add missing-dep case to bash smoke"
```

---

## Phase 3: `install.ps1` (PowerShell installer)

### Task 7: Add success-path PowerShell smoke test

PowerShell version of the smoke harness. Builds zip fixtures with `Compress-Archive`, generates `SHA256SUMS` with `Get-FileHash`, points `BZR_BASE_URL` at `file:///` URLs, and asserts the installed file path.

**Files:**
- Create: `tests/installer/smoke.ps1`

- [ ] **Step 1: Create `tests/installer/smoke.ps1`**

```powershell
# Smoke tests for install.ps1. Runs against local file:// fixtures.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$InstallPs1 = Join-Path $RepoRoot 'install.ps1'

if (-not (Test-Path $InstallPs1)) {
    Write-Error "smoke: $InstallPs1 not found"
    exit 1
}

function New-Fixtures {
    param([string]$Dir, [string]$Tag, [string]$Target)
    $staging = "bzr-$Tag-$Target"
    $stagingDir = Join-Path $Dir $staging
    New-Item -ItemType Directory -Path $stagingDir -Force | Out-Null
    # Stub bzr.exe — content doesn't have to be runnable when BZR_SKIP_SMOKE=1
    Set-Content -Path (Join-Path $stagingDir 'bzr.exe') -Value 'fake bzr binary' -NoNewline
    Set-Content -Path (Join-Path $stagingDir 'LICENSE') -Value 'fake LICENSE' -NoNewline
    Set-Content -Path (Join-Path $stagingDir 'README.md') -Value 'fake README' -NoNewline
    Compress-Archive -Path $stagingDir -DestinationPath (Join-Path $Dir "$staging.zip")
    Remove-Item -Recurse -Force $stagingDir
    # Generate SHA256SUMS
    Push-Location $Dir
    try {
        $hash = (Get-FileHash -Algorithm SHA256 "$staging.zip").Hash.ToLower()
        Set-Content -Path 'SHA256SUMS' -Value "$hash  $staging.zip" -Encoding ascii -NoNewline
    } finally { Pop-Location }
}

function Get-NativeTarget {
    if ($env:PROCESSOR_ARCHITECTURE -eq 'AMD64') { return 'x86_64-pc-windows-msvc' }
    if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') { return 'aarch64-pc-windows-msvc' }
    return ''
}

function Convert-PathToFileUri {
    param([string]$Path)
    # Convert a Windows local path to a file:// URI install.ps1 can fetch.
    $abs = (Resolve-Path $Path).Path
    "file:///" + ($abs -replace '\\', '/')
}

function Test-SuccessPath {
    $work = New-Item -ItemType Directory -Path (Join-Path $env:TEMP ("bzr-smoke-" + [Guid]::NewGuid())) -Force
    try {
        $fixtures = New-Item -ItemType Directory -Path (Join-Path $work 'releases\v0.0.0-test') -Force
        $installDir = Join-Path $work 'bin'
        $target = Get-NativeTarget
        if (-not $target) { Write-Host "smoke: skipping success_path (unsupported host)"; return }
        New-Fixtures -Dir $fixtures.FullName -Tag 'v0.0.0-test' -Target $target

        $env:BZR_BASE_URL = Convert-PathToFileUri (Join-Path $work 'releases')
        $env:BZR_VERSION = 'v0.0.0-test'
        $env:BZR_INSTALL_DIR = $installDir
        $env:BZR_SKIP_SMOKE = '1'
        try {
            & powershell -NoProfile -ExecutionPolicy Bypass -File $InstallPs1
            if ($LASTEXITCODE -ne 0) { throw "install.ps1 failed with exit $LASTEXITCODE" }
        } finally {
            Remove-Item Env:BZR_BASE_URL, Env:BZR_VERSION, Env:BZR_INSTALL_DIR, Env:BZR_SKIP_SMOKE -ErrorAction SilentlyContinue
        }

        if (-not (Test-Path (Join-Path $installDir 'bzr.exe'))) {
            throw "smoke: bzr.exe not installed at $installDir\bzr.exe"
        }
        Write-Host "smoke: success_path OK"
    } finally {
        Remove-Item -Recurse -Force $work
    }
}

Test-SuccessPath
Write-Host "smoke: all sub-tests passed"
```

- [ ] **Step 2: Run it (expect failure — install.ps1 doesn't exist yet)**

Run: `pwsh tests/installer/smoke.ps1` (or `powershell` on Windows)
Expected: error message about missing `install.ps1` and non-zero exit.

- [ ] **Step 3: Commit**

```bash
git add tests/installer/smoke.ps1
git commit -m "test(installer): add powershell smoke harness with success-path case"
```

---

### Task 8: Implement `install.ps1` (success path)

**Files:**
- Create: `install.ps1`

- [ ] **Step 1: Create `install.ps1`**

```powershell
<#
.SYNOPSIS
  bzr installer for Windows.

.DESCRIPTION
  Usage:
    irm https://raw.githubusercontent.com/randomparity/bzr/main/install.ps1 | iex

  Env vars:
    $env:BZR_VERSION       - release tag (default: latest stable)
    $env:BZR_INSTALL_DIR   - install directory (default: %LOCALAPPDATA%\Programs\bzr)
#>
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# Force TLS 1.2 — PS 5.1 defaults to SSL3/TLS1.0 which fails against GitHub.
[Net.ServicePointManager]::SecurityProtocol =
    [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

# RELEASE_VERSION_PIN -- release.yml rewrites the next line at release time
$BzrVersion = $env:BZR_VERSION
$BzrInstallDir = if ($env:BZR_INSTALL_DIR) { $env:BZR_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'Programs\bzr' }

# Internal: undocumented test override for the GitHub releases base URL.
$BaseUrl = if ($env:BZR_BASE_URL) { $env:BZR_BASE_URL } else { 'https://github.com/randomparity/bzr/releases/download' }
# Internal: undocumented test flag to skip the post-install --version call.
$SkipSmoke = [bool]$env:BZR_SKIP_SMOKE

$GithubApi = 'https://api.github.com/repos/randomparity/bzr/releases/latest'

function Write-Err($msg) { [Console]::Error.WriteLine("install.ps1: $msg") }

function Get-Target {
    $arch = $env:PROCESSOR_ARCHITEW6432
    if (-not $arch) { $arch = $env:PROCESSOR_ARCHITECTURE }
    switch ($arch) {
        'AMD64' { return 'x86_64-pc-windows-msvc' }
        'ARM64' { return 'aarch64-pc-windows-msvc' }
        default { return $null }
    }
}

function Resolve-Version {
    if ($BzrVersion) { return $BzrVersion }
    try {
        $resp = Invoke-WebRequest -UseBasicParsing -Uri $GithubApi
        $obj = $resp.Content | ConvertFrom-Json
        if (-not $obj.tag_name) { throw "tag_name missing in API response" }
        return $obj.tag_name
    } catch {
        Write-Err "failed to query GitHub API for the latest release"
        Write-Err "set `$env:BZR_VERSION = 'vX.Y.Z' to pin to a specific tag"
        exit 4
    }
}

function Get-ExpectedHash {
    param([string]$SumsPath, [string]$Filename)
    $line = Get-Content $SumsPath | Where-Object { $_ -match "  $([regex]::Escape($Filename))$" } | Select-Object -First 1
    if (-not $line) {
        Write-Err "checksum line not found for $Filename in SHA256SUMS"
        exit 5
    }
    return ($line -split '\s+')[0].ToLower()
}

# --- main ---
$target = Get-Target
if (-not $target) {
    Write-Err "unsupported platform: $($env:PROCESSOR_ARCHITECTURE)"
    Write-Err "Try one of:"
    Write-Err "  - cargo install bzr --locked"
    Write-Err "  - Homebrew (on macOS/Linux): brew tap randomparity/tap && brew install bzr"
    exit 2
}

$tag = Resolve-Version
$archive = "bzr-$tag-$target.zip"
$archiveUrl = "$BaseUrl/$tag/$archive"
$sumsUrl = "$BaseUrl/$tag/SHA256SUMS"

$work = New-Item -ItemType Directory -Path (Join-Path $env:TEMP ("bzr-install-" + [Guid]::NewGuid())) -Force
try {
    Write-Host "install.ps1: downloading $archiveUrl"
    try {
        Invoke-WebRequest -UseBasicParsing -Uri $archiveUrl -OutFile (Join-Path $work $archive)
        Invoke-WebRequest -UseBasicParsing -Uri $sumsUrl    -OutFile (Join-Path $work 'SHA256SUMS')
    } catch {
        Write-Err "download failed: $_"
        exit 4
    }

    $expected = Get-ExpectedHash -SumsPath (Join-Path $work 'SHA256SUMS') -Filename $archive
    $actual = (Get-FileHash -Algorithm SHA256 -Path (Join-Path $work $archive)).Hash.ToLower()
    if ($expected -ne $actual) {
        Write-Err "SHA-256 verification failed: expected $expected, got $actual"
        exit 5
    }

    try {
        Expand-Archive -Path (Join-Path $work $archive) -DestinationPath $work -Force
    } catch {
        Write-Err "extraction failed: $_"
        exit 6
    }

    if (-not (Test-Path $BzrInstallDir)) {
        New-Item -ItemType Directory -Path $BzrInstallDir -Force | Out-Null
    }
    Copy-Item -Path (Join-Path $work "bzr-$tag-$target\bzr.exe") -Destination (Join-Path $BzrInstallDir 'bzr.exe') -Force

    if (-not $SkipSmoke) {
        try {
            & (Join-Path $BzrInstallDir 'bzr.exe') --version | Out-Null
        } catch {
            Write-Err "bzr installed at $BzrInstallDir\bzr.exe but failed to run: $_"
        }
    }

    Write-Host "install.ps1: installed bzr to $BzrInstallDir\bzr.exe"

    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $procPath = $env:Path
    if (($userPath -notlike "*$BzrInstallDir*") -and ($procPath -notlike "*$BzrInstallDir*")) {
        Write-Host @"
install.ps1: $BzrInstallDir is not on your PATH. Add it (current user, persistent):
  [Environment]::SetEnvironmentVariable('Path',
    [Environment]::GetEnvironmentVariable('Path','User') + ';$BzrInstallDir',
    'User')
"@
    }
} finally {
    Remove-Item -Recurse -Force $work
}
```

- [ ] **Step 2: Run smoke test**

Run on a Windows host: `powershell -NoProfile -File tests/installer/smoke.ps1`

(On non-Windows, this can be skipped — it's tested in CI on `windows-latest`.)

Expected: `smoke: success_path OK` and `smoke: all sub-tests passed`.

- [ ] **Step 3: Run PSScriptAnalyzer**

Run: `pwsh -c "Invoke-ScriptAnalyzer -Path install.ps1, tests/installer/smoke.ps1 -Severity Warning"`
Expected: no diagnostics output.

(If `pwsh` and PSScriptAnalyzer aren't available locally, this is fine — CI runs it.)

- [ ] **Step 4: Commit**

```bash
git add install.ps1 tests/installer/smoke.ps1
git commit -m "feat(installer): add install.ps1 with target detection and SHA256 verify"
```

---

### Task 9: Add PowerShell checksum-mismatch test

**Files:**
- Modify: `tests/installer/smoke.ps1`

- [ ] **Step 1: Append the failure-path test**

Add the function below `Test-SuccessPath` in `tests/installer/smoke.ps1`:

```powershell
function Test-ChecksumMismatch {
    $work = New-Item -ItemType Directory -Path (Join-Path $env:TEMP ("bzr-smoke-" + [Guid]::NewGuid())) -Force
    try {
        $fixtures = New-Item -ItemType Directory -Path (Join-Path $work 'releases\v0.0.0-test') -Force
        $installDir = Join-Path $work 'bin'
        $target = Get-NativeTarget
        if (-not $target) { Write-Host "smoke: skipping checksum_mismatch (unsupported host)"; return }
        New-Fixtures -Dir $fixtures.FullName -Tag 'v0.0.0-test' -Target $target
        # Corrupt the archive after sums were generated.
        Add-Content -Path (Join-Path $fixtures.FullName "bzr-v0.0.0-test-$target.zip") -Value 'tampered'

        $env:BZR_BASE_URL = Convert-PathToFileUri (Join-Path $work 'releases')
        $env:BZR_VERSION = 'v0.0.0-test'
        $env:BZR_INSTALL_DIR = $installDir
        $env:BZR_SKIP_SMOKE = '1'
        try {
            & powershell -NoProfile -ExecutionPolicy Bypass -File $InstallPs1 *>$null
            $rc = $LASTEXITCODE
        } finally {
            Remove-Item Env:BZR_BASE_URL, Env:BZR_VERSION, Env:BZR_INSTALL_DIR, Env:BZR_SKIP_SMOKE -ErrorAction SilentlyContinue
        }

        if ($rc -ne 5) { throw "smoke: expected exit 5 (checksum mismatch), got $rc" }
        if (Test-Path (Join-Path $installDir 'bzr.exe')) {
            throw "smoke: bzr.exe should NOT be installed when checksum fails"
        }
        Write-Host "smoke: checksum_mismatch OK"
    } finally {
        Remove-Item -Recurse -Force $work
    }
}
```

Update the runner block at the bottom:

```powershell
Test-SuccessPath
Test-ChecksumMismatch
Write-Host "smoke: all sub-tests passed"
```

- [ ] **Step 2: Run smoke test (on Windows)**

Run: `powershell -NoProfile -File tests/installer/smoke.ps1`
Expected: both sub-tests pass.

- [ ] **Step 3: Commit**

```bash
git add tests/installer/smoke.ps1
git commit -m "test(installer): add checksum-mismatch case to powershell smoke"
```

---

### Task 10: Add PowerShell unsupported-platform test

The bash version stubbed `uname` on PATH. PowerShell's target detection reads `$env:PROCESSOR_ARCHITECTURE` directly, so we override via the env var.

**Files:**
- Modify: `tests/installer/smoke.ps1`

- [ ] **Step 1: Append the unsupported-platform test**

Add to `tests/installer/smoke.ps1`:

```powershell
function Test-UnsupportedTarget {
    $work = New-Item -ItemType Directory -Path (Join-Path $env:TEMP ("bzr-smoke-" + [Guid]::NewGuid())) -Force
    try {
        $installDir = Join-Path $work 'bin'

        $env:BZR_VERSION = 'v0.0.0-test'
        $env:BZR_INSTALL_DIR = $installDir
        $env:BZR_SKIP_SMOKE = '1'
        # Force install.ps1 to read this fake arch via PROCESSOR_ARCHITEW6432.
        $env:PROCESSOR_ARCHITEW6432 = 'IA64'
        try {
            & powershell -NoProfile -ExecutionPolicy Bypass -File $InstallPs1 *>$null
            $rc = $LASTEXITCODE
        } finally {
            Remove-Item Env:BZR_VERSION, Env:BZR_INSTALL_DIR, Env:BZR_SKIP_SMOKE, Env:PROCESSOR_ARCHITEW6432 -ErrorAction SilentlyContinue
        }

        if ($rc -ne 2) { throw "smoke: expected exit 2 (unsupported), got $rc" }
        Write-Host "smoke: unsupported_target OK"
    } finally {
        Remove-Item -Recurse -Force $work
    }
}
```

Update the runner block:

```powershell
Test-SuccessPath
Test-ChecksumMismatch
Test-UnsupportedTarget
Write-Host "smoke: all sub-tests passed"
```

- [ ] **Step 2: Run smoke test (on Windows)**

Expected: three sub-tests pass.

- [ ] **Step 3: Commit**

```bash
git add tests/installer/smoke.ps1
git commit -m "test(installer): add unsupported-platform case to powershell smoke"
```

---

## Phase 4: CI integration

### Task 11: Add installer-lint job to `ci.yml`

Run `shellcheck`, `shfmt`, and `Invoke-ScriptAnalyzer` on every PR. The job is cheap (~30 s) so it runs unconditionally; no path filtering needed (the project's `ci.yml` doesn't use path filters today, and adding a third-party `paths-filter` action just for this feature would be unjustified scope).

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Read the current CI workflow**

Run: `head -40 .github/workflows/ci.yml`
Note the existing top-level `on:` and `jobs:` block to confirm the new job slots in cleanly.

- [ ] **Step 2: Add the installer-lint job at the end of `jobs:`**

Append this job to `.github/workflows/ci.yml` (use the same `actions/checkout` SHA pin used by neighbouring jobs in the file):

```yaml
  installer-lint:
    name: Lint installer scripts
    runs-on: ubuntu-latest
    permissions:
      contents: read
    steps:
      - uses: actions/checkout@de0fac2e4500dabe0009e67214ff5f5447ce83dd  # v6

      - name: Install shellcheck and shfmt
        run: |
          sudo apt-get update
          sudo apt-get install -y shellcheck
          curl -fsSL https://github.com/mvdan/sh/releases/download/v3.7.0/shfmt_v3.7.0_linux_amd64 -o /usr/local/bin/shfmt
          sudo chmod +x /usr/local/bin/shfmt

      - name: Run shellcheck
        run: shellcheck -s sh install.sh tests/installer/smoke.sh

      - name: Run shfmt (check only)
        run: shfmt -d -ln posix -i 2 install.sh tests/installer/smoke.sh

      - name: Run PSScriptAnalyzer
        shell: pwsh
        run: |
          Install-Module -Name PSScriptAnalyzer -Force -Scope CurrentUser
          $diagnostics = Invoke-ScriptAnalyzer -Path install.ps1, tests/installer/smoke.ps1 -Severity Warning
          if ($diagnostics) {
            $diagnostics | Format-Table
            exit 1
          }
```

- [ ] **Step 3: Lint the workflow**

Run: `actionlint .github/workflows/ci.yml`
Expected: no output.

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci(installer): lint install.{sh,ps1} with shellcheck, shfmt, PSScriptAnalyzer"
```

---

### Task 12: Add installer-test job to `ci.yml`

3-platform smoke matrix. Runs the local-fixtures smoke tests on `ubuntu-latest`, `macos-14`, and `windows-latest`. Like `installer-lint`, runs unconditionally — the matrix takes ~2 min total wall-clock in parallel.

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Append the installer-test job**

Add this job alongside `installer-lint`:

```yaml
  installer-test:
    name: Smoke ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-14, windows-latest]
    runs-on: ${{ matrix.os }}
    permissions:
      contents: read
    steps:
      - uses: actions/checkout@de0fac2e4500dabe0009e67214ff5f5447ce83dd  # v6

      - name: Smoke (bash)
        if: matrix.os != 'windows-latest'
        run: sh tests/installer/smoke.sh

      - name: Smoke (powershell)
        if: matrix.os == 'windows-latest'
        shell: pwsh
        run: ./tests/installer/smoke.ps1
```

- [ ] **Step 2: Lint the workflow**

Run: `actionlint .github/workflows/ci.yml`
Expected: no output.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci(installer): smoke-test install.{sh,ps1} on linux, macos, windows"
```

---

## Phase 5: Release workflow integration

### Task 13: Add "Stage installer scripts" step + reorder SHA256SUMS

The new step writes version-pinned copies of `install.sh` and `install.ps1` into `artifacts/`. SHA256SUMS must come *after* this step so the sums file covers the installer scripts.

**Files:**
- Modify: `.github/workflows/release.yml`

- [ ] **Step 1: Read the current release job**

Run: `awk '/^  release:/,0' .github/workflows/release.yml | head -80`
Note: `Generate SHA256SUMS` is currently the second step in the `release` job (after `Download artifacts`).

- [ ] **Step 2: Add the staging step before Generate SHA256SUMS**

Insert this step in `.github/workflows/release.yml` immediately after the `actions/download-artifact@…` step in the `release` job, and before `Generate SHA256SUMS`. Uses Python rather than `sed` to sidestep the bash-inside-YAML escaping hell that comes with the literal `$BzrVersion` and `${BZR_VERSION:-}` markers; the `assert` doubles as the rewrite-verification check.

```yaml
      - name: Stage installer scripts (with version baked in)
        working-directory: artifacts
        env:
          TAG: ${{ github.ref_name }}
        run: |
          set -euo pipefail
          python3 - <<'PYEOF'
          import os, pathlib
          tag = os.environ["TAG"]

          sh_marker = 'BZR_VERSION="${BZR_VERSION:-}"'
          sh_replace = 'BZR_VERSION="${BZR_VERSION:-' + tag + '}"'
          sh_in = pathlib.Path("../install.sh").read_text()
          sh_out = sh_in.replace(sh_marker, sh_replace, 1)
          assert sh_out != sh_in, f"install.sh marker line not found: {sh_marker!r}"
          pathlib.Path("install.sh").write_text(sh_out)

          ps_marker = "$BzrVersion = $env:BZR_VERSION"
          ps_replace = (
              "$BzrVersion = if ($env:BZR_VERSION) "
              "{ $env:BZR_VERSION } else { '" + tag + "' }"
          )
          ps_in = pathlib.Path("../install.ps1").read_text()
          ps_out = ps_in.replace(ps_marker, ps_replace, 1)
          assert ps_out != ps_in, f"install.ps1 marker line not found: {ps_marker!r}"
          pathlib.Path("install.ps1").write_text(ps_out)
          PYEOF
          chmod +x install.sh
```

- [ ] **Step 3: Verify SHA256SUMS step still comes after**

Run: `grep -n -E "Stage installer scripts|Generate SHA256SUMS" .github/workflows/release.yml`
Expected: `Stage installer scripts` line number is smaller than `Generate SHA256SUMS` line number.

- [ ] **Step 4: Lint the workflow**

Run: `actionlint .github/workflows/release.yml`
Expected: no output.

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "release: stage installer scripts and include them in SHA256SUMS"
```

---

### Task 14: Add `installer-smoke` post-release job

Validates `install.sh` and `install.ps1` against the just-published release CDN. Runs on every tag, including prereleases — that's exactly when version-pin templating regressions matter.

**Files:**
- Modify: `.github/workflows/release.yml`

- [ ] **Step 1: Append the new job to release.yml**

Add this job after the `release` job:

```yaml
  installer-smoke:
    name: Installer smoke ${{ matrix.os }}
    needs: release
    runs-on: ${{ matrix.os }}
    permissions:
      contents: read
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, windows-latest]
    steps:
      - name: Smoke install.sh
        if: matrix.os == 'ubuntu-latest'
        env:
          TAG: ${{ github.ref_name }}
        run: |
          set -euo pipefail
          export BZR_INSTALL_DIR="$HOME/.local/bin"
          curl -fsSL "https://github.com/randomparity/bzr/releases/download/${TAG}/install.sh" | sh
          installed_version="$("$BZR_INSTALL_DIR/bzr" --version 2>&1)"
          echo "installed: $installed_version"
          # bzr --version prints "bzr X.Y.Z[-rcN]"; tag is "vX.Y.Z[-rcN]".
          version_no_v="${TAG#v}"
          case "$installed_version" in
            *"$version_no_v"*) echo "version match: OK" ;;
            *) echo "version mismatch: tag=$TAG output=$installed_version" >&2; exit 1 ;;
          esac
        shell: bash

      - name: Smoke install.ps1
        if: matrix.os == 'windows-latest'
        env:
          TAG: ${{ github.ref_name }}
        shell: pwsh
        run: |
          $env:BZR_INSTALL_DIR = Join-Path $env:LOCALAPPDATA 'Programs\bzr'
          irm "https://github.com/randomparity/bzr/releases/download/$env:TAG/install.ps1" | iex
          $installed = & (Join-Path $env:BZR_INSTALL_DIR 'bzr.exe') --version
          Write-Host "installed: $installed"
          $versionNoV = $env:TAG.TrimStart('v')
          if ($installed -notlike "*$versionNoV*") {
              Write-Error "version mismatch: tag=$env:TAG output=$installed"
              exit 1
          }
          Write-Host "version match: OK"
```

- [ ] **Step 2: Lint the workflow**

Run: `actionlint .github/workflows/release.yml`
Expected: no output.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "release: add post-tag installer smoke against the real CDN"
```

---

## Phase 6: Documentation

### Task 15: Update README install section

Add the one-liners at the top of "Pre-built binaries"; tweak the decision-tree bullet to reference the installer.

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Update the decision-tree bullet (line 42)**

Replace:

```markdown
- **Windows, or any Linux distro without `apt`/`dnf`** — download a [pre-built tarball or zip](#pre-built-binaries).
```

with:

```markdown
- **Windows, or any Linux distro without `apt`/`dnf`** — use the [one-line installer](#pre-built-binaries) or download a pre-built tarball or zip.
```

- [ ] **Step 2: Update the "Pre-built binaries" section**

Replace the entire current "Pre-built binaries" subsection (from `### Pre-built binaries` through the `sha256sum --check` block) with:

````markdown
### Pre-built binaries

For a one-line install on Linux, macOS arm64, or Windows:

**Linux / macOS (Apple Silicon):**

```bash
curl -fsSL https://raw.githubusercontent.com/randomparity/bzr/main/install.sh | sh
```

**Windows (x86_64 / ARM64):**

```powershell
irm https://raw.githubusercontent.com/randomparity/bzr/main/install.ps1 | iex
```

The installer detects your platform, verifies the SHA-256 checksum
against the published `SHA256SUMS`, and drops the binary in
`~/.local/bin` (Unix) or `%LOCALAPPDATA%\Programs\bzr` (Windows).
It never modifies PATH or system files.

Env var overrides:

- `BZR_VERSION=vX.Y.Z` — pin to a specific release tag (default:
  latest stable).
- `BZR_INSTALL_DIR=/some/path` — change the install directory.

To pin both the script and the binary to a specific release (e.g.
for reproducible builds):

```bash
curl -fsSL https://github.com/randomparity/bzr/releases/download/vX.Y.Z/install.sh | sh
```

Manpages are not installed by the script; see [Manual pages](#manual-pages).

#### Manual download

Or download the tarball or zip directly from
[GitHub Releases](https://github.com/randomparity/bzr/releases/latest).

Available builds: Linux (x86_64, aarch64, ppc64le, s390x), macOS arm64
(Apple Silicon), Windows (x86_64, aarch64).

```bash
tar xzf bzr-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz
cd bzr-vX.Y.Z-x86_64-unknown-linux-gnu
sudo install -Dm755 bzr /usr/local/bin/bzr
```

Each archive bundles the binary, `LICENSE`, `README.md`, and a
`man/man1/` directory of manpages — see [Manual pages](#manual-pages)
to install those.

Each release also publishes a `SHA256SUMS` file. Verify a download
before installing:

```bash
sha256sum --check --ignore-missing SHA256SUMS
```
````

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs(readme): document install.{sh,ps1} one-liners under Pre-built binaries"
```

---

### Task 16: Update RELEASING.md

Add bullets describing the new release-time behavior.

**Files:**
- Modify: `RELEASING.md`

- [ ] **Step 1: Add bullets to the "GitHub release binaries" section**

In `RELEASING.md`, find the bullet list under `### GitHub release binaries` (currently ends with "Creates a GitHub Release for the tag with all artifacts attached"). Add these two bullets immediately before that final bullet:

```markdown
- Stages `install.sh` and `install.ps1` as release assets, with the release
  tag baked into the `BZR_VERSION` default
- Runs an `installer-smoke` job after the release that re-runs each script
  against the real GitHub Releases CDN (Ubuntu and Windows runners) and
  verifies `bzr --version` matches the released tag
```

- [ ] **Step 2: Commit**

```bash
git add RELEASING.md
git commit -m "docs(releasing): note installer-script staging and post-tag smoke"
```

---

### Task 17: Add CHANGELOG Unreleased entry

**Files:**
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Read the current top of CHANGELOG.md**

Run: `head -25 CHANGELOG.md`
Confirm the format. Look for an existing `## [Unreleased]` section (project convention is to add entries as work lands).

- [ ] **Step 2: Add the entry**

Under `## [Unreleased]` → `### Added` (create the subsection if it doesn't exist), add:

```markdown
- Installer scripts (`install.sh`, `install.ps1`) for one-line installation
  from GitHub Releases, with SHA-256 verification against the published
  `SHA256SUMS` file. Hosted at the `main` branch URL for always-current
  installs and as release assets pinned to each tag for reproducibility.
```

- [ ] **Step 3: Commit**

```bash
git add CHANGELOG.md
git commit -m "docs(changelog): note installer scripts under Unreleased"
```

---

## Final validation

### Task 18: End-to-end pre-merge validation

Before opening the PR for merge, validate the integration manually.

- [ ] **Step 1: Run the full local smoke**

Run on a Linux or macOS host:

```bash
sh tests/installer/smoke.sh
```

Expected: all four sub-tests pass.

- [ ] **Step 2: Run lint locally**

```bash
shellcheck -s sh install.sh tests/installer/smoke.sh
shfmt -d -ln posix -i 2 install.sh tests/installer/smoke.sh
actionlint .github/workflows/ci.yml .github/workflows/release.yml
```

Expected: no diagnostic output from any tool.

- [ ] **Step 3: Verify the version-pin rewrites would succeed**

Run the same Python rewrite that `release.yml` will run, against a temporary tag:

```bash
TAG=v0.2.1 python3 - <<'PYEOF'
import os, pathlib
tag = os.environ["TAG"]

sh_marker = 'BZR_VERSION="${BZR_VERSION:-}"'
sh_replace = 'BZR_VERSION="${BZR_VERSION:-' + tag + '}"'
sh_in = pathlib.Path("install.sh").read_text()
sh_out = sh_in.replace(sh_marker, sh_replace, 1)
assert sh_out != sh_in, f"install.sh marker not found: {sh_marker!r}"

ps_marker = "$BzrVersion = $env:BZR_VERSION"
ps_replace = (
    "$BzrVersion = if ($env:BZR_VERSION) "
    "{ $env:BZR_VERSION } else { '" + tag + "' }"
)
ps_in = pathlib.Path("install.ps1").read_text()
ps_out = ps_in.replace(ps_marker, ps_replace, 1)
assert ps_out != ps_in, f"install.ps1 marker not found: {ps_marker!r}"

print("Both markers found and rewritten correctly.")
PYEOF
```

Expected: `Both markers found and rewritten correctly.`

If either assertion fails, the marker line in the script doesn't match what `release.yml` expects — fix the script and re-run.

- [ ] **Step 4: Push and open PR**

```bash
git push -u origin feat/installer-scripts
gh pr create --title "feat: add install.sh and install.ps1 with SHA256 verification" --body "$(cat <<'EOF'
## Summary

- Adds `install.sh` (POSIX bash) and `install.ps1` (PowerShell 5.1+) at the repo root for one-line installation from GitHub Releases.
- Verifies downloads against the existing `SHA256SUMS`; release workflow now stages the scripts before the sums file is generated, so the scripts themselves are also covered.
- Fixes a pre-existing zip-layout asymmetry in the Windows packaging step (drops `\*` from `Compress-Archive -Path`).
- Adds CI lint + 3-platform smoke matrix and a post-tag installer smoke job that runs against the real CDN.

Spec: `docs/specs/2026-05-04-installer-scripts-design.md`
Plan: `docs/plans/2026-05-04-installer-scripts.md`

## Test plan

- [ ] CI: `installer-lint` job green
- [ ] CI: `installer-test` job green on ubuntu-latest, macos-14, windows-latest
- [ ] Manual: `curl -fsSL .../main/install.sh | sh` on a clean Ubuntu container against next prerelease tag
- [ ] Manual: `irm .../main/install.ps1 | iex` on a clean Windows VM against next prerelease tag
- [ ] Release: `installer-smoke` job green when next tag is cut
EOF
)"
```

- [ ] **Step 5: Cut a prerelease tag for end-to-end validation**

After PR merges, cut a `v0.2.1-rc1` tag (or similar prerelease) to exercise the full release flow including `installer-smoke`. Confirm:

- The `Stage installer scripts` step writes both files into `artifacts/`.
- `SHA256SUMS` includes lines for `install.sh` and `install.ps1`.
- `installer-smoke` job passes on both Ubuntu and Windows runners.

If anything fails, file follow-up issues — do not roll back; the prerelease is exactly what this validation is for.

---

## Notes for the implementer

- The `BZR_BASE_URL` and `BZR_SKIP_SMOKE` env vars are intentionally undocumented in user-facing docs. They exist solely for the smoke harness. Don't mention them in the README.
- The `RELEASE_VERSION_PIN` comment marker above each script's `BZR_VERSION`/`$BzrVersion` line is the contract with `release.yml`. Don't move or rename either marker without also updating the `sed` patterns and verification `grep`s.
- The pre-existing prerelease zips (`v0.2.0-rc[1-4]`) have the asymmetric layout. The installer will fail against those tags. That's acceptable — they're prereleases and the installer ships post-v0.2.0. Default version resolution skips prereleases via `releases/latest`.
- Linux runtime `libdbus-1` is a known footgun. The bash installer surfaces it in stderr if `bzr --version` fails post-install; users on minimal images should prefer the `.deb`/`.rpm` packages (which declare the dep) or `cargo install … --no-default-features`.
