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

WORKDIR=""
cleanup() { [ -n "$WORKDIR" ] && rm -rf "$WORKDIR"; }
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
  WORKDIR="$(mktemp -d)"
  fixtures="$WORKDIR/releases/v0.0.0-test"
  install_dir="$WORKDIR/bin"
  mkdir -p "$fixtures"
  target="$(detect_native_target)"
  if [ -z "$target" ]; then
    echo "smoke: skipping success_path on unsupported host" >&2
    return 0
  fi
  mkfixtures "$fixtures" "v0.0.0-test" "$target"

  BZR_BASE_URL="file://$WORKDIR/releases" \
  BZR_VERSION="v0.0.0-test" \
  BZR_INSTALL_DIR="$install_dir" \
  BZR_SKIP_SMOKE=1 \
    sh "$INSTALL_SH"

  [ -x "$install_dir/bzr" ] || { echo "smoke: bzr not installed at $install_dir/bzr" >&2; return 1; }
  echo "smoke: success_path OK"
}

test_success_path
echo "smoke: all sub-tests passed"
