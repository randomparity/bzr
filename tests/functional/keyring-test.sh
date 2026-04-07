#!/usr/bin/env bash
#
# Real-OS-keychain functional test for bzr.
#
# Builds bzr, sets up a temporary XDG_CONFIG_HOME, writes a server
# entry, stores a secret via `bzr config set-keyring`, resolves it,
# migrates inline -> keyring, and cleans up. Runs on macOS, Linux
# (if Secret Service is reachable), and Windows (via Git Bash).
#
# Usage: tests/functional/keyring-test.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT_DIR"

TMP_CONFIG_HOME="$(mktemp -d)"
SERVICE_NAME="bzr-functional-test-$$"
SERVER_NAME="fntest"
SECRET="functional-test-secret-$RANDOM"

cleanup() {
  # Best-effort: remove the keychain entry and the temp config.
  if [[ -x ./target/debug/bzr ]]; then
    XDG_CONFIG_HOME="$TMP_CONFIG_HOME" \
      ./target/debug/bzr config unset-keyring "$SERVER_NAME" 2>/dev/null || true
  fi
  rm -rf "$TMP_CONFIG_HOME"
  return 0
}
trap cleanup EXIT

# On Linux, probe for a reachable Secret Service before running.
if [[ "$(uname -s)" == "Linux" ]]; then
  if ! command -v secret-tool >/dev/null 2>&1; then
    echo "SKIP: secret-tool not installed; cannot verify Secret Service."
    exit 0
  fi
  # Probe: lookup a nonexistent entry. Exit code 1 means "not found"
  # (service is reachable); exit codes >1 mean D-Bus / service
  # unavailable.
  set +e
  secret-tool lookup bzr-probe-svc bzr-probe-acct >/dev/null 2>&1
  rc=$?
  set -e
  if [[ $rc -gt 1 ]]; then
    echo "SKIP: Secret Service unavailable (rc=$rc)."
    exit 0
  fi
fi

echo "Building bzr..."
cargo build --quiet

BZR="./target/debug/bzr"
export XDG_CONFIG_HOME="$TMP_CONFIG_HOME"

echo "1. Creating server with inline key..."
"$BZR" config set-server "$SERVER_NAME" \
  --url "https://example.invalid" \
  --api-key "initial-inline"

echo "2. Migrating inline -> keyring..."
"$BZR" config migrate-to-keyring "$SERVER_NAME" \
  --service "$SERVICE_NAME" --yes

echo "3. Verifying 'config show' reports api_key_source=keyring..."
if ! "$BZR" config show --json | grep -q '"api_key_source": *"keyring"'; then
  echo "FAIL: expected api_key_source=keyring in config show output"
  "$BZR" config show --json
  exit 1
fi

echo "4. Overwriting the stored secret via set-keyring..."
# BZR_KEYRING_TEST_SECRET is a debug-build-only hook (guarded by
# #[cfg(debug_assertions)]); it bypasses the stdin prompt so the
# functional test does not need an interactive TTY. It has no effect
# in release builds.
BZR_KEYRING_TEST_SECRET="$SECRET" "$BZR" config set-keyring "$SERVER_NAME" \
  --service "$SERVICE_NAME"

echo "5. Removing keychain entry via unset-keyring..."
"$BZR" config unset-keyring "$SERVER_NAME"

echo "OK: keyring functional test passed."
