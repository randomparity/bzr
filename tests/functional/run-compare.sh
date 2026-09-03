#!/bin/bash
# Compare bzr with python-bugzilla against one real Bugzilla instance.
# SC1091: lib.sh is resolved from the computed script directory.
# SC2317/SC2329: cleanup is invoked through the EXIT trap.
# shellcheck disable=SC1090,SC1091,SC2034,SC2043,SC2317,SC2329
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

source "$SCRIPT_DIR/lib.sh"

TEST_ID_PREFIX=compare
BZR_BIN="${BZR_COMPARE_BIN:-$REPO_ROOT/target/release/bzr}"
BZ_PORT="${BZR_FUNC_PORT:-}"
if [[ -z "$BZ_PORT" ]]; then
    _compare_runtime=$(container_runtime) || {
        echo "ERROR: neither podman nor docker found in PATH" >&2
        exit 1
    }
    _compare_container=$(bugzilla_container_name) || {
        echo "ERROR: could not derive the Bugzilla container name" >&2
        exit 1
    }
    BZ_PORT=$(bugzilla_container_port "$_compare_runtime" "$_compare_container") || {
        echo "ERROR: could not determine Bugzilla container port for" \
            "'$_compare_container'; is it running?" \
            "(tests/functional/setup-bugzilla.sh start)" >&2
        exit 1
    }
fi
BZ_URL="http://127.0.0.1:${BZ_PORT}"

if [[ ! -x "$BZR_BIN" ]]; then
    echo "ERROR: bzr comparison binary is not executable: $BZR_BIN" >&2
    exit 1
fi

FUNC_CONFIG_DIR=$(mktemp -d /tmp/bzr-compare-config.XXXXXX)
COMPARE_EXCHANGE_DIR="$FUNC_CONFIG_DIR/compare"
mkdir -p "$COMPARE_EXCHANGE_DIR"
export XDG_CONFIG_HOME="$FUNC_CONFIG_DIR"
export BZ_URL BZR_BIN COMPARE_EXCHANGE_DIR
CURRENT_TEST_GROUP=""
_compare_runtime="${_compare_runtime:-}"

cleanup() {
    if [[ -n "$_compare_runtime" ]]; then
        pybz_sidecar_stop "$_compare_runtime" || true
    fi
    rm -rf "$FUNC_CONFIG_DIR"
    _cleanup_tmpfiles
    return 0
}
trap cleanup EXIT

if [[ -z "$_compare_runtime" ]]; then
    _compare_runtime=$(container_runtime) || {
        echo "ERROR: neither podman nor docker found in PATH" >&2
        exit 1
    }
fi
_compare_container=$(bugzilla_container_name) || {
    echo "ERROR: could not derive the Bugzilla container name" >&2
    exit 1
}
pybz_sidecar_start "$_compare_runtime" "$_compare_container"

for _phase in \
    00-products; do
    CURRENT_TEST_GROUP="$_phase"
    source "$SCRIPT_DIR/compare/${_phase}.sh"
done

echo "── Comparison cleanup (${BZ_VERSION}) ───────────────────────────"
echo "  Cleaning up temp files..."

if test_summary; then
    exit 0
else
    exit 1
fi
