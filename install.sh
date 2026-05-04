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
  command -v "$1" >/dev/null 2>&1 || {
    err "missing required command: $1"
    exit 3
  }
}

detect_target() {
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$os/$arch" in
  Linux/x86_64) echo x86_64-unknown-linux-gnu ;;
  Linux/aarch64 | Linux/arm64) echo aarch64-unknown-linux-gnu ;;
  Linux/ppc64le) echo powerpc64le-unknown-linux-gnu ;;
  Linux/s390x) echo s390x-unknown-linux-gnu ;;
  Darwin/arm64) echo aarch64-apple-darwin ;;
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
    echo "$BZR_VERSION"
    return
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
  sums="$1"
  fname="$2"
  dir="$3"
  # Match the filename at end-of-line; strip any leading './' so sha256sum -c
  # finds the file by bare name (sha256sum ./*.tar.gz produces './foo' entries).
  line="$(grep "$fname\$" "$sums" | sed 's|  \./|  |' || true)"
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
    err "neither curl nor wget is installed"
    exit 3
  }
  command -v sha256sum >/dev/null 2>&1 || command -v shasum >/dev/null 2>&1 || {
    err "neither sha256sum nor shasum is installed"
    exit 3
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
  http_get "$archive_url" "$workdir/$archive" || {
    err "download failed: $archive_url"
    exit 4
  }
  http_get "$sums_url" "$workdir/SHA256SUMS" || {
    err "download failed: $sums_url"
    exit 4
  }

  verify_sha256 "$workdir/SHA256SUMS" "$archive" "$workdir" || {
    err "SHA-256 verification failed"
    exit 5
  }

  (cd "$workdir" && tar xzf "$archive") || {
    err "tar extraction failed"
    exit 6
  }

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
