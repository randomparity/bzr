#!/bin/bash
# Run the comparison harness against every supported Bugzilla version.
# SC2317/SC2329: cleanup_all is invoked through the EXIT trap; the diagnostic code differs between
# supported ShellCheck versions.
# shellcheck disable=SC2317,SC2329
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VERSIONS=("bz50" "bz52" "bz53")
FAILED=0
RESULTS=()

cleanup_all() {
    for ver in "${VERSIONS[@]}"; do
        BZR_BZ_VERSION="$ver" "$SCRIPT_DIR/setup-bugzilla.sh" stop 2>/dev/null || true
    done
    return 0
}
trap cleanup_all EXIT

for ver in "${VERSIONS[@]}"; do
    echo ""
    echo "╔══════════════════════════════════════════════════════════"
    echo "║  Starting comparison for ${ver}"
    echo "╚══════════════════════════════════════════════════════════"
    echo ""

    export BZR_BZ_VERSION="$ver"
    if ! "$SCRIPT_DIR/setup-bugzilla.sh" reset; then
        RESULTS+=("${ver}: FAILED (container start)")
        FAILED=1
        continue
    fi

    if "$SCRIPT_DIR/run-compare.sh"; then
        RESULTS+=("${ver}: PASSED")
    else
        RESULTS+=("${ver}: FAILED")
        FAILED=1
    fi
done

echo ""
echo "╔══════════════════════════════════════════════════════════"
echo "║  Comparison Multi-Version Summary"
echo "╚══════════════════════════════════════════════════════════"
for result in "${RESULTS[@]}"; do
    echo "  $result"
done
echo ""

exit "$FAILED"
