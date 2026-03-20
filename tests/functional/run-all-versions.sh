#!/bin/bash
# Run functional tests against all supported Bugzilla versions.
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VERSIONS=("bz50" "bz52")
FAILED=0
RESULTS=()

cleanup_all() {
    for ver in "${VERSIONS[@]}"; do
        BZR_BZ_VERSION="$ver" "$SCRIPT_DIR/setup-bugzilla.sh" stop 2>/dev/null || true
    done
}
trap cleanup_all EXIT

for ver in "${VERSIONS[@]}"; do
    echo ""
    echo "╔══════════════════════════════════════════════════════════╗"
    echo "║  Starting tests for ${ver}                                  ║"
    echo "╚══════════════════════════════════════════════════════════╝"
    echo ""

    export BZR_BZ_VERSION="$ver"
    if ! "$SCRIPT_DIR/setup-bugzilla.sh" start; then
        RESULTS+=("${ver}: FAILED (container start)")
        FAILED=1
        continue
    fi

    if "$SCRIPT_DIR/run-tests.sh"; then
        RESULTS+=("${ver}: PASSED")
    else
        RESULTS+=("${ver}: FAILED")
        FAILED=1
    fi
done

echo ""
echo "╔══════════════════════════════════════════════════════════╗"
echo "║  Multi-Version Summary                                  ║"
echo "╚══════════════════════════════════════════════════════════╝"
for r in "${RESULTS[@]}"; do
    echo "  $r"
done
echo ""

exit $FAILED
