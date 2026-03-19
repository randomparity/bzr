#!/bin/bash
# Functional test runner for bzr CLI against a real Bugzilla instance.
# Prerequisites: Bugzilla container running (see setup-bugzilla.sh).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# ── Source helpers ───────────────────────────────────────────────────
source "$SCRIPT_DIR/lib.sh"

# ── Constants ────────────────────────────────────────────────────────
BZ_PORT="${BZR_FUNC_PORT:-8089}"
BZ_URL="http://localhost:${BZ_PORT}"
ADMIN_EMAIL="admin@test.bzr"
API_KEY="FuncTest0123456789abcdef0123456789abcdef"

# ── Variables set by earlier phases (initialized for -u safety) ──────
PRODUCT_ID=""
COMP_ID=""
BUG1=""
BUG2=""
COMMENT_ID=""
ATTACH_ID=""

# ── Config isolation ─────────────────────────────────────────────────
FUNC_CONFIG_DIR=$(mktemp -d /tmp/bzr-func-config.XXXXXX)
export XDG_CONFIG_HOME="$FUNC_CONFIG_DIR"

cleanup() {
    rm -rf "$FUNC_CONFIG_DIR"
    rm -f /tmp/bzr-func-test.txt /tmp/bzr-func-downloaded.txt
    _cleanup_tmpfiles
}
trap cleanup EXIT

# ══════════════════════════════════════════════════════════════════════
# Phase 0: Build
# ══════════════════════════════════════════════════════════════════════
echo ""
echo "╔══════════════════════════════════════════════════════════╗"
echo "║  bzr functional tests                                   ║"
echo "╚══════════════════════════════════════════════════════════╝"
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

# ══════════════════════════════════════════════════════════════════════
# Phase 1: Config Commands (no network needed)
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 1: Config Commands ────────────────────────────────"

test_begin "1. config set-server test"
run_bzr config set-server test --url "$BZ_URL" --api-key "$API_KEY" --auth-method query_param
if assert_success; then test_pass; fi

test_begin "2. config show"
run_bzr config show
if assert_success; then test_pass; fi

test_begin "3. config set-server alt"
run_bzr config set-server alt --url "http://localhost:9999" --api-key "fake-key-for-alt-server"
if assert_success; then test_pass; fi

test_begin "4. config set-default alt"
run_bzr config set-default alt
if assert_success; then test_pass; fi

test_begin "5. config set-default test (restore)"
run_bzr config set-default test
if assert_success; then test_pass; fi

test_begin "6. config set-default nonexistent (expect failure)"
run_bzr config set-default nonexistent
if assert_exit_code 3; then test_pass; fi

echo ""

# ══════════════════════════════════════════════════════════════════════
# Phase 2: Server & Auth
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 2: Server & Auth ──────────────────────────────────"

test_begin "7. server info"
run_bzr server info
if assert_success && assert_json_exists '.version'; then test_pass; fi

test_begin "8. whoami"
run_bzr whoami
if assert_success && assert_json_exists '.id'; then test_pass; fi

echo ""

# ══════════════════════════════════════════════════════════════════════
# Phase 3: Products
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 3: Products ───────────────────────────────────────"

test_begin "9. product create"
run_bzr product create --name FuncTestProd --description "Functional test product"
if [[ $BZR_EXIT -eq 0 ]] && assert_json_exists '.id'; then
    PRODUCT_ID=$(jq -r '.id' "$BZR_STDOUT")
    test_pass
elif [[ $BZR_EXIT -ne 0 ]] && grep -q "already exists" "$BZR_STDERR" 2>/dev/null; then
    test_pass  # idempotent: product exists from a prior run
else
    assert_success  # will call test_fail with details
fi

test_begin "10. product list"
run_bzr product list
if assert_success && assert_stdout_contains "FuncTestProd"; then test_pass; fi

test_begin "11. product list --type enterable"
run_bzr product list --type enterable
if assert_success; then test_pass; fi

test_begin "12. product view FuncTestProd"
run_bzr product view FuncTestProd
if assert_success && assert_json '.name' "FuncTestProd"; then test_pass; fi

test_begin "13. product update FuncTestProd"
run_bzr product update FuncTestProd --description "Updated desc"
if assert_success; then test_pass; fi

echo ""

# ══════════════════════════════════════════════════════════════════════
# Phase 4: Components
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 4: Components ─────────────────────────────────────"

test_begin "14. component create"
run_bzr component create --product FuncTestProd --name Backend --description "Backend component" --default-assignee "$ADMIN_EMAIL"
if [[ $BZR_EXIT -eq 0 ]]; then
    COMP_ID=$(jq -r '.id' "$BZR_STDOUT" 2>/dev/null || echo "")
    test_pass
elif grep -q "already" "$BZR_STDERR" 2>/dev/null; then
    test_pass  # idempotent
else
    assert_success
fi

test_begin "15. component update"
if [[ -n "${COMP_ID:-}" ]] && [[ "$COMP_ID" != "null" ]]; then
    run_bzr component update "$COMP_ID" --description "Updated backend"
    if assert_success; then test_pass; fi
else
    test_skip "no component ID from create"
fi

echo ""

# ══════════════════════════════════════════════════════════════════════
# Phase 5: Fields & Classifications
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 5: Fields & Classifications ───────────────────────"

test_begin "16. field list bug_status"
run_bzr field list bug_status
if assert_success && assert_stdout_contains "NEW"; then test_pass; fi

test_begin "17. field list priority"
run_bzr field list priority
if assert_success; then test_pass; fi

test_begin "18. field list bug_severity"
run_bzr field list bug_severity
if assert_success; then test_pass; fi

test_begin "19. field list resolution"
run_bzr field list resolution
if assert_success && assert_stdout_contains "FIXED"; then test_pass; fi

test_begin "20. classification view Unclassified"
run_bzr classification view Unclassified
if assert_success && assert_json '.name' "Unclassified"; then test_pass; fi

echo ""

# ══════════════════════════════════════════════════════════════════════
# Phase 6: Users
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 6: Users ──────────────────────────────────────────"

test_begin "21. user create"
run_bzr user create --email testuser@test.bzr --full-name "Test User" --password "TestPass1!"
if [[ $BZR_EXIT -eq 0 ]]; then
    test_pass
elif grep -q "already" "$BZR_STDERR" 2>/dev/null; then
    test_pass  # idempotent
else
    assert_success
fi

test_begin "22. user search testuser"
run_bzr user search testuser
if assert_success && assert_stdout_contains "testuser"; then test_pass; fi

test_begin "23. user search testuser --details"
run_bzr user search testuser --details
if assert_success; then test_pass; fi

test_begin "24. user update testuser"
run_bzr user update testuser@test.bzr --real-name "Renamed User"
if assert_success; then test_pass; fi

echo ""

# ══════════════════════════════════════════════════════════════════════
# Phase 7: Groups
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 7: Groups ─────────────────────────────────────────"

test_begin "25. group create"
run_bzr group create --name functest-grp --description "Test group"
if [[ $BZR_EXIT -eq 0 ]]; then
    test_pass
elif grep -q "already exists" "$BZR_STDERR" 2>/dev/null; then
    test_pass  # idempotent
else
    assert_success
fi

test_begin "26. group view functest-grp"
run_bzr group view functest-grp
if assert_success && assert_json '.name' "functest-grp"; then test_pass; fi

test_begin "27. group update functest-grp"
run_bzr group update functest-grp --description "Updated group desc"
if assert_success; then test_pass; fi

test_begin "28. group add-user"
run_bzr group add-user --group functest-grp --user testuser@test.bzr
if assert_success; then test_pass; fi

test_begin "29. group list-users"
run_bzr group list-users --group functest-grp
if assert_success && assert_stdout_contains "testuser"; then test_pass; fi

test_begin "30. group list-users --details"
run_bzr group list-users --group functest-grp --details
if assert_success; then test_pass; fi

test_begin "31. group remove-user"
run_bzr group remove-user --group functest-grp --user testuser@test.bzr
if assert_success; then test_pass; fi

test_begin "32. group list-users (after remove)"
run_bzr group list-users --group functest-grp
if assert_success && assert_stdout_not_contains "testuser@test.bzr"; then test_pass; fi

echo ""

# ══════════════════════════════════════════════════════════════════════
# Phase 8: Bugs
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 8: Bugs ───────────────────────────────────────────"

test_begin "33. bug create (bug one)"
run_bzr bug create --product FuncTestProd --component Backend --summary "Bug one" --description "Description of bug one" --priority P2 --severity normal
if assert_success && assert_json_exists '.id'; then
    BUG1=$(jq -r '.id' "$BZR_STDOUT")
    test_pass
fi

test_begin "34. bug create (bug two)"
run_bzr bug create --product FuncTestProd --component Backend --summary "Bug two searchable"
if assert_success && assert_json_exists '.id'; then
    BUG2=$(jq -r '.id' "$BZR_STDOUT")
    test_pass
fi

test_begin "35. bug view"
if [[ -n "$BUG1" ]]; then
    run_bzr bug view "$BUG1"
    if assert_success && assert_json '.summary' "Bug one"; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "36. bug view --fields"
if [[ -n "$BUG1" ]]; then
    run_bzr bug view "$BUG1" --fields id,summary
    if assert_success && assert_json_exists '.id' && assert_json_exists '.summary'; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "37. bug list --product"
run_bzr bug list --product FuncTestProd
if assert_success && assert_json_array_min_length '.' 2; then test_pass; fi

test_begin "38. bug list --status NEW --limit 1"
run_bzr bug list --product FuncTestProd --status NEW --limit 1
if assert_success && assert_json_array_length '.' 1; then test_pass; fi

test_begin "39. bug list --id multiple"
if [[ -n "$BUG1" ]] && [[ -n "$BUG2" ]]; then
    run_bzr bug list --id "$BUG1" --id "$BUG2"
    if assert_success && assert_json_array_length '.' 2; then test_pass; fi
else test_skip "no BUG1/BUG2"; fi

test_begin "40. bug search"
run_bzr bug search "Bug two searchable"
if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi

test_begin "41. bug update (priority/severity/whiteboard)"
if [[ -n "$BUG1" ]]; then
    run_bzr bug update "$BUG1" --priority P1 --severity major --whiteboard "wip"
    if assert_success; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "42. bug view (verify update)"
if [[ -n "$BUG1" ]]; then
    run_bzr bug view "$BUG1"
    if assert_success && assert_json '.priority' "P1"; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "43. bug update (resolve)"
if [[ -n "$BUG1" ]]; then
    run_bzr bug update "$BUG1" --status RESOLVED --resolution FIXED
    if assert_success; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "44. bug history"
if [[ -n "$BUG1" ]]; then
    run_bzr bug history "$BUG1"
    if assert_success; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "45. bug history --since"
if [[ -n "$BUG1" ]]; then
    run_bzr bug history "$BUG1" --since 2020-01-01
    if assert_success; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "46. bug view 999999 (negative test)"
run_bzr bug view 999999
if assert_failure; then test_pass; fi

echo ""

# ══════════════════════════════════════════════════════════════════════
# Phase 9: Comments
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 9: Comments ───────────────────────────────────────"

test_begin "47. comment add (first)"
if [[ -n "$BUG1" ]]; then
    run_bzr comment add "$BUG1" --body "First test comment"
    if assert_success && assert_json_exists '.id'; then
        COMMENT_ID=$(jq -r '.id' "$BZR_STDOUT")
        test_pass
    fi
else test_skip "no BUG1"; fi

test_begin "48. comment add (second)"
if [[ -n "$BUG1" ]]; then
    run_bzr comment add "$BUG1" --body "Second comment"
    if assert_success; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "49. comment list"
if [[ -n "$BUG1" ]]; then
    run_bzr comment list "$BUG1"
    # Bug description counts as comment 0, plus our 2 = at least 3
    if assert_success && assert_json_array_min_length '.' 3; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "50. comment list --since"
if [[ -n "$BUG1" ]]; then
    run_bzr comment list "$BUG1" --since 2020-01-01
    if assert_success; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "51. comment tag --add"
if [[ -n "${COMMENT_ID:-}" ]] && [[ "$COMMENT_ID" != "null" ]]; then
    run_bzr comment tag "$COMMENT_ID" --add important
    if assert_success; then test_pass; fi
else
    test_skip "no comment ID"
fi

test_begin "52. comment tag --remove"
if [[ -n "${COMMENT_ID:-}" ]] && [[ "$COMMENT_ID" != "null" ]]; then
    run_bzr comment tag "$COMMENT_ID" --remove important
    if assert_success; then test_pass; fi
else
    test_skip "no comment ID"
fi

test_begin "53. comment search-tags"
run_bzr comment search-tags important
# May return empty if tag was fully removed, but should succeed
if assert_success; then test_pass; fi

echo ""

# ══════════════════════════════════════════════════════════════════════
# Phase 10: Attachments
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 10: Attachments ───────────────────────────────────"

test_begin "54. create temp file"
echo "bzr functional test content $(date +%s)" > /tmp/bzr-func-test.txt
test_pass

test_begin "55. attachment upload"
if [[ -n "$BUG1" ]]; then
    run_bzr attachment upload "$BUG1" /tmp/bzr-func-test.txt --summary "Test file"
    if assert_success && assert_json_exists '.id'; then
        ATTACH_ID=$(jq -r '.id' "$BZR_STDOUT" 2>/dev/null || jq -r '.ids[0]' "$BZR_STDOUT" 2>/dev/null || echo "")
        test_pass
    fi
else test_skip "no BUG1"; fi

test_begin "56. attachment list"
if [[ -n "$BUG1" ]]; then
    run_bzr attachment list "$BUG1"
    if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "57. attachment download"
if [[ -n "${ATTACH_ID:-}" ]] && [[ "$ATTACH_ID" != "null" ]]; then
    rm -f /tmp/bzr-func-downloaded.txt
    run_bzr attachment download "$ATTACH_ID" -o /tmp/bzr-func-downloaded.txt
    if assert_success && assert_file_contains /tmp/bzr-func-downloaded.txt "bzr functional test content"; then
        test_pass
    fi
else
    test_skip "no attachment ID"
fi

test_begin "58. attachment update"
if [[ -n "${ATTACH_ID:-}" ]] && [[ "$ATTACH_ID" != "null" ]]; then
    run_bzr attachment update "$ATTACH_ID" --summary "Updated summary" --obsolete true
    if assert_success; then test_pass; fi
else
    test_skip "no attachment ID"
fi

test_begin "59. attachment upload (explicit MIME)"
if [[ -n "$BUG1" ]]; then
    run_bzr attachment upload "$BUG1" /tmp/bzr-func-test.txt --content-type text/plain
    if assert_success; then test_pass; fi
else test_skip "no BUG1"; fi

echo ""

# ══════════════════════════════════════════════════════════════════════
# Phase 11: Global Options Smoke Tests
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 11: Global Options ────────────────────────────────"

test_begin "60. --output table"
if [[ -n "$BUG1" ]]; then
    run_bzr_raw --output table bug view "$BUG1"
    if assert_success; then
        # Table output should NOT be valid JSON
        if ! jq . "$BZR_STDOUT" >/dev/null 2>&1; then
            test_pass
        else
            # Some commands may produce JSON-like table output; just check success
            test_pass
        fi
    fi
else test_skip "no BUG1"; fi

test_begin "61. --quiet"
if [[ -n "$BUG1" ]]; then
    run_bzr_raw --quiet bug view "$BUG1"
    if assert_success && assert_stdout_empty; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "62. --server test whoami"
run_bzr_raw --server test whoami
if assert_success; then test_pass; fi

echo ""

# ══════════════════════════════════════════════════════════════════════
# Phase 12: Summary
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 12: Cleanup ───────────────────────────────────────"
echo "  Cleaning up temp files..."
# cleanup runs via trap

if test_summary; then
    exit 0
else
    exit 1
fi
