# 00-build
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 0: Build
# ══════════════════════════════════════════════════════════════════════
echo ""
echo "╔══════════════════════════════════════════════════════════"
echo "║  bzr functional tests (${BZ_VERSION})"
echo "╚══════════════════════════════════════════════════════════"
echo ""

echo "── Phase 0: Build ──────────────────────────────────────────"
if [[ -n "${BZR_BIN:-}" ]] && [[ -x "$BZR_BIN" ]]; then
    echo "  Using pre-built binary: $BZR_BIN"
else
    echo "  Building release binary..."
    (cd "$REPO_ROOT" && cargo build --release 2>&1 | tail -3)
    BZR_BIN="$REPO_ROOT/target/release/bzr"
fi
export BZR_BIN

if [[ ! -x "$BZR_BIN" ]]; then
    echo "FATAL: bzr binary not found at $BZR_BIN"
    exit 1
fi
echo "  Binary: $BZR_BIN"
echo ""

# ── Verify Bugzilla is running ───────────────────────────────────────
echo "  Checking Bugzilla at ${BZ_URL}/rest/version ..."
if ! curl -sf "${BZ_URL}/rest/version" >/dev/null 2>&1; then
    echo "FATAL: Bugzilla is not running at ${BZ_URL}"
    echo "  Run: tests/functional/setup-bugzilla.sh start"
    exit 1
fi
echo "  Bugzilla is up."
echo ""
