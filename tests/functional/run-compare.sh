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

seed_server_saved_search() {
    if [[ $# -ne 4 ]]; then
        printf 'seed_server_saved_search: expected LOGIN NAME BUG_ID BUG_ID\n' >&2
        return 2
    fi

    local login="$1" name="$2" first_id="$3" second_id="$4"
    local runtime container helper query
    if [[ -z $login || -z $name || ${#name} -gt 64 || $name == *$'\n'* ||
        ! $first_id =~ ^[1-9][0-9]*$ || ! $second_id =~ ^[1-9][0-9]*$ ]]; then
        printf 'seed_server_saved_search: invalid login, name, or bug ID\n' >&2
        return 2
    fi
    runtime=$(container_runtime) || return 1
    container=$(bugzilla_container_name) || return 1
    helper="$SCRIPT_DIR/compare/seed-saved-search.pl"
    query="bug_id=${first_id},${second_id}&bug_id_type=anyexact"
    if [[ ! -r $helper ]]; then
        printf 'seed_server_saved_search: helper is unreadable\n' >&2
        return 1
    fi
    "$runtime" exec -i --workdir /var/www/html/bugzilla "$container" perl -I. - \
        "$login" "$name" "$query" <"$helper"
}

if [[ ! -x "$BZR_BIN" ]]; then
    echo "ERROR: bzr comparison binary is not executable: $BZR_BIN" >&2
    exit 1
fi

cleanup() {
    local status=$?

    if ! redhat_shape_stop; then
        [[ $status -ne 0 ]] || status=1
    fi
    _tls_cleanup
    if ! resource_membership_cleanup; then
        [[ $status -ne 0 ]] || status=1
    fi
    if [[ -n "${PYBZ_RUNTIME:-}" ]]; then
        if ! pybz_proxy_stop tls; then
            [[ $status -ne 0 ]] || status=1
        fi
        if ! pybz_proxy_stop redhat; then
            [[ $status -ne 0 ]] || status=1
        fi
        if ! pybz_sidecar_stop "${_compare_runtime:-}"; then
            [[ $status -ne 0 ]] || status=1
        fi
    fi
    rm -rf "$FUNC_CONFIG_DIR" || true
    _cleanup_tmpfiles || true
    trap - EXIT
    exit "$status"
}

umask 077
FUNC_CONFIG_DIR=$(mktemp -d /tmp/bzr-compare-config.XXXXXX)
trap cleanup EXIT
COMPARE_EXCHANGE_DIR="$FUNC_CONFIG_DIR/compare"
mkdir -p "$COMPARE_EXCHANGE_DIR"
COMPARE_ADMIN_EMAIL="admin@test.bzr"
COMPARE_ADMIN_PASSWORD="FuncTest1!"
BZR_COMPARE_API_KEY="FuncTest0123456789abcdef0123456789abcdef"
_compare_adapter="$SCRIPT_DIR/compare/python-bugzilla-adapter.py"
if [[ ! -r $_compare_adapter ]]; then
    echo "ERROR: comparison adapter is missing or unreadable: $_compare_adapter" >&2
    exit 1
fi
cp "$_compare_adapter" "$COMPARE_EXCHANGE_DIR/python-bugzilla-adapter.py"
chmod 600 "$COMPARE_EXCHANGE_DIR/python-bugzilla-adapter.py"
if [[ ! -r $COMPARE_EXCHANGE_DIR/python-bugzilla-adapter.py ]]; then
    echo "ERROR: comparison adapter staging failed" >&2
    exit 1
fi
pybz_stage_proxy "$SCRIPT_DIR/tls-proxy.py" tls-proxy.py >/dev/null
pybz_stage_proxy "$SCRIPT_DIR/redhat-shape-proxy.py" redhat-proxy.py >/dev/null
export XDG_CONFIG_HOME="$FUNC_CONFIG_DIR"
export BZ_URL BZR_BIN COMPARE_EXCHANGE_DIR COMPARE_ADMIN_EMAIL COMPARE_ADMIN_PASSWORD
export BZR_COMPARE_API_KEY
CURRENT_TEST_GROUP=""
_compare_runtime="${_compare_runtime:-}"

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
resource_init

for _phase in \
    00-products \
    01-bug-lifecycle \
    02-comments \
    03-attachments \
    04-users-groups \
    05-products-components \
    06-auth-config-tls; do
    if [[ $_phase == 03-attachments ]]; then
        seed_comparison_attachment_flag_type
    fi
    CURRENT_TEST_GROUP="$_phase"
    source "$SCRIPT_DIR/compare/${_phase}.sh"
    _render_test_result
done

echo "── Comparison cleanup (${BZ_VERSION}) ───────────────────────────"
echo "  Cleaning up temp files..."

if test_summary; then
    exit 0
else
    exit 1
fi
