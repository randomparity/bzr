#!/bin/bash
# Run functional tests against all supported Bugzilla versions.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VERSIONS=("bz50" "bz52")
FAILED=0
RESULTS=()

for ver in "${VERSIONS[@]}"; do
    echo ""
    echo "╔══════════════════════════════════════════════════════════╗"
    echo "║  Starting tests for ${ver}                                  ║"
    echo "╚══════════════════════════════════════════════════════════╝"
    echo ""

    export BZR_BZ_VERSION="$ver"
    "$SCRIPT_DIR/setup-bugzilla.sh" start

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
