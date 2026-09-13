#!/bin/bash
# shellcheck disable=SC1090,SC1091,SC2034
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib.sh"
[[ $BZ_VERSION == rhbz ]] || { echo "ERROR: requires BZR_BZ_VERSION=rhbz" >&2; exit 2; }
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
runtime=$(container_runtime)
container=$(bugzilla_container_name)
port=$(bugzilla_container_port "$runtime" "$container")
export BZ_URL="http://127.0.0.1:${port}"
export BZR_BIN="${BZR_COMPARE_BIN:-$REPO_ROOT/target/release/bzr}"
[[ -x $BZR_BIN ]] || { echo "ERROR: bzr comparison binary is not executable: $BZR_BIN" >&2; exit 1; }
TEST_ID_PREFIX=compare
umask 077
FUNC_CONFIG_DIR=$(mktemp -d /tmp/bzr-rhbz-compare-config.XXXXXX)
COMPARE_EXCHANGE_DIR="$FUNC_CONFIG_DIR/compare"
mkdir -p "$COMPARE_EXCHANGE_DIR"
COMPARE_ADMIN_EMAIL="admin@test.bzr"
BZR_COMPARE_API_KEY="FuncTest0123456789abcdef0123456789abcdef"
export FUNC_CONFIG_DIR COMPARE_EXCHANGE_DIR COMPARE_ADMIN_EMAIL BZR_COMPARE_API_KEY
export XDG_CONFIG_HOME="$FUNC_CONFIG_DIR"
cp "$SCRIPT_DIR/compare/python-bugzilla-adapter.py" "$COMPARE_EXCHANGE_DIR/"
chmod 600 "$COMPARE_EXCHANGE_DIR/python-bugzilla-adapter.py"
cleanup() {
    local status=$?
    [[ -z ${PYBZ_RUNTIME:-} ]] || pybz_sidecar_stop "$runtime" || status=1
    rm -rf "$FUNC_CONFIG_DIR"
    exit "$status"
}
trap cleanup EXIT
pybz_sidecar_start "$runtime" "$container"
resource_init
CURRENT_TEST_GROUP=07-rhbz-smoke
source "$SCRIPT_DIR/compare/rhbz/07-rhbz-smoke.sh"
_render_test_result
CURRENT_TEST_GROUP=09-rhbz-fields
source "$SCRIPT_DIR/compare/rhbz/09-rhbz-fields.sh"
_render_test_result
CURRENT_TEST_GROUP=08-rhbz-externalbugs
source "$SCRIPT_DIR/compare/rhbz/08-rhbz-externalbugs.sh"
_render_test_result
test_summary
