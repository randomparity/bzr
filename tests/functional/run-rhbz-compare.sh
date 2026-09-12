#!/bin/bash
# shellcheck disable=SC1090,SC1091,SC2034
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib.sh"
[[ $BZ_VERSION == rhbz ]] || { echo "ERROR: requires BZR_BZ_VERSION=rhbz" >&2; exit 2; }
runtime=$(container_runtime)
container=$(bugzilla_container_name)
port=$(bugzilla_container_port "$runtime" "$container")
export BZ_URL="http://127.0.0.1:${port}"
TEST_ID_PREFIX=compare
CURRENT_TEST_GROUP=07-rhbz-smoke
source "$SCRIPT_DIR/compare/07-rhbz-smoke.sh"
_render_test_result
test_summary
