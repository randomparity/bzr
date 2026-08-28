#!/bin/bash
# Functional test runner for bzr CLI against a real Bugzilla instance.
# Prerequisites: Bugzilla container running (see setup-bugzilla.sh).
#
# Orchestrator: this file owns setup (constants, shared globals, cleanup trap)
# and the summary, then sources phases/*.sh in order. The constants and shared
# globals below are consumed by the sourced phase files; shellcheck cannot
# follow the dynamic `source` in the phase loop, so disable its unused-variable
# warning for them here.
# SC1091: lib.sh is resolved from the computed script directory.
# SC2329: cleanup is invoked through the EXIT trap.
# shellcheck disable=SC1091,SC2034,SC2329
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# ── Source helpers ───────────────────────────────────────────────────
source "$SCRIPT_DIR/lib.sh"

# ── Constants ────────────────────────────────────────────────────────
BZ_VERSION="${BZR_BZ_VERSION:-bz50}"
case "$BZ_VERSION" in
bz50) DEFAULT_PORT=8089 ;;
bz52) DEFAULT_PORT=8090 ;;
bz53) DEFAULT_PORT=8091 ;;
*) DEFAULT_PORT=8089 ;;
esac
BZ_PORT="${BZR_FUNC_PORT:-$DEFAULT_PORT}"
BZ_URL="http://127.0.0.1:${BZ_PORT}"
ADMIN_EMAIL="admin@test.bzr"
API_KEY="FuncTest0123456789abcdef0123456789abcdef"

# ── Variables set by earlier phases (initialized for -u safety) ──────
PRODUCT_ID=""
COMP_ID=""
BUG1=""
BUG2=""
BUG3=""
BUG4=""
BUG_DUP_SOURCE=""
BUG_DUP_TARGET=""
CLONE_ID=""
TMPL_BUG=""
COMMENT_ID=""
ATTACH_ID=""
RESTRICTED_BUG=""

# ── Config isolation ─────────────────────────────────────────────────
FUNC_CONFIG_DIR=$(mktemp -d /tmp/bzr-func-config.XXXXXX)
export XDG_CONFIG_HOME="$FUNC_CONFIG_DIR"

cleanup() {
    rm -rf "$FUNC_CONFIG_DIR"
    rm -f /tmp/bzr-func-test.txt /tmp/bzr-func-downloaded.txt
    _cleanup_tmpfiles
    return 0
}
trap cleanup EXIT

# ══════════════════════════════════════════════════════════════════════
# Phases — sourced in order. They share state through the globals declared
# above; they are ordered segments, not independently runnable modules.
# ══════════════════════════════════════════════════════════════════════
for _phase in \
    00-build 01-config 02-server-auth 02c-tls-inline 03-products 04-components \
    05-fields-classifications 06-users 07-groups 08-bugs 08b-bugs-paging \
    08c-bugs-create-fields 08d-bug-update-from-json 08e-bugs-restricted-access \
    09-bug-relationships 09b-bug-collision 09c-bug-links \
    10-bug-clone 11-batch-update 11b-bug-verbs 12-my-bugs 13-templates 14-queries \
    15-comments 15b-comments-private 16-attachments 16b-attachments-private \
    17-global-options 17b-arg-validation 18-completion-schema 18a-json-envelope \
    18b-http-error-preview 18c-skills-install 18d-dependency-analysis 99-sequences; do
    # shellcheck source=/dev/null
    source "$SCRIPT_DIR/phases/${_phase}.sh"
done

# ══════════════════════════════════════════════════════════════════════
# Phase 17: Summary
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 17: Cleanup (${BZ_VERSION}) ──────────────────────────────"
echo "  Cleaning up temp files..."
# cleanup runs via trap

if test_summary; then
    exit 0
else
    exit 1
fi
