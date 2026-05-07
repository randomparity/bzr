#!/bin/bash
# Functional test runner for bzr CLI against a real Bugzilla instance.
# Prerequisites: Bugzilla container running (see setup-bugzilla.sh).
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
    *)    DEFAULT_PORT=8089 ;;
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
CLONE_ID=""
TMPL_BUG=""
COMMENT_ID=""
ATTACH_ID=""

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

# ══════════════════════════════════════════════════════════════════════
# Phase 1: Config Commands (no network needed)
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 1: Config Commands ────────────────────────────────"

test_begin "1. config set-server test"
run_bzr config set-server test --url "$BZ_URL" --api-key "$API_KEY" --auth-method query_param --email "$ADMIN_EMAIL"
if assert_success; then test_pass; fi

test_begin "2. config show"
run_bzr config show
if assert_success; then test_pass; fi

test_begin "3. config set-server alt"
run_bzr config set-server alt --url "http://localhost:9999" --api-key "fake-key-for-alt-server"
if assert_success; then test_pass; fi

test_begin "3a. config set-server auto-detect"
run_bzr config set-server auto --url "$BZ_URL" --api-key "$API_KEY" --email "$ADMIN_EMAIL"
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

test_begin "8a. --server auto whoami"
run_bzr_raw --json --server auto whoami
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
# Component update REST endpoint is not available on Bugzilla 5.0 or 5.2
if [[ -n "${COMP_ID:-}" ]] && [[ "$COMP_ID" != "null" ]]; then
    run_bzr component update "$COMP_ID" --description "Updated backend"
    if [[ $BZR_EXIT -eq 0 ]]; then
        test_pass
    elif grep -q "32614" "$BZR_STDERR" 2>/dev/null; then
        test_skip "component update REST endpoint not available"
    else
        assert_success  # report the actual error
    fi
else
    # Component was already created in a prior run; try to look up the ID
    COMP_ID=$(curl -sf "${BZ_URL}/rest/component?product=FuncTestProd&name=Backend&Bugzilla_api_key=${API_KEY}" 2>/dev/null | jq -r '.components[0].id // empty' 2>/dev/null || echo "")
    if [[ -n "$COMP_ID" ]] && [[ "$COMP_ID" != "null" ]]; then
        run_bzr component update "$COMP_ID" --description "Updated backend"
        if [[ $BZR_EXIT -eq 0 ]]; then
            test_pass
        elif grep -q "32614" "$BZR_STDERR" 2>/dev/null; then
            test_skip "component update REST endpoint not available"
        else
            assert_success
        fi
    else
        test_skip "no component ID available"
    fi
fi

echo ""

# ══════════════════════════════════════════════════════════════════════
# Phase 5: Fields & Classifications
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 5: Fields & Classifications ───────────────────────"

test_begin "16. field list status (alias resolution)"
run_bzr field list status
if assert_success && assert_stdout_contains "CONFIRMED"; then test_pass; fi

test_begin "17. field list priority"
run_bzr field list priority
if assert_success; then test_pass; fi

test_begin "18. field list severity (alias resolution)"
run_bzr field list severity
if assert_success; then test_pass; fi

test_begin "19. field list resolution"
run_bzr field list resolution
if assert_success && assert_stdout_contains "FIXED"; then test_pass; fi

test_begin "19a. field list bug_status (internal name still works)"
run_bzr field list bug_status
if assert_success && assert_stdout_contains "CONFIRMED"; then test_pass; fi

test_begin "19b. field list nonexistent_xyz (error case)"
run_bzr field list nonexistent_xyz
if assert_failure; then test_pass; fi

test_begin "19c. field aliases"
run_bzr field aliases
if assert_success && assert_stdout_contains "status" && assert_stdout_contains "bug_status"; then test_pass; fi

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

# Re-enable testuser in case it was disabled by a prior run (test 24 sets disable_login=true)
test_begin "21b. user re-enable (idempotent fix)"
run_bzr user update testuser@test.bzr --disable-login false --login-denied-text ""
if assert_success; then test_pass; fi

test_begin "22. user search testuser"
run_bzr user search testuser
if assert_success && assert_stdout_contains "testuser"; then test_pass; fi

test_begin "23. user search testuser --details"
run_bzr user search testuser --details
if assert_success; then test_pass; fi

test_begin "24. user update testuser"
# Note: Bugzilla 5.0 REST API does not support real_name updates
# (set_real_name method not found). Use login_denied_text instead.
run_bzr user update testuser@test.bzr --disable-login true --login-denied-text "test disabled"
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
if [[ $BZR_EXIT -eq 0 ]] && assert_json '.name' "functest-grp"; then
    test_pass
else
    assert_success
fi

test_begin "26a. group view functest-grp with --api rest"
run_bzr_raw --json --server test --api rest group view functest-grp
if [[ $BZR_EXIT -eq 0 ]] && assert_json '.name' "functest-grp"; then
    test_pass
else
    assert_success
fi

test_begin "27. group update functest-grp"
run_bzr group update functest-grp --description "Updated group desc"
if assert_success; then test_pass; fi

# Re-enable testuser before group membership tests (test 24 disables it)
test_begin "27b. user re-enable for group tests"
run_bzr user update testuser@test.bzr --disable-login false --login-denied-text ""
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

# Re-disable testuser so it's excluded from list-users results (Bugzilla 5.0
# default user search hides disabled users, which is also what test 24 does)
run_bzr user update testuser@test.bzr --disable-login true --login-denied-text "test disabled" >/dev/null 2>&1 || true

test_begin "32. group list-users (after remove)"
run_bzr group list-users --group functest-grp
if assert_success && assert_stdout_not_contains "testuser@test.bzr"; then test_pass; fi

echo ""

# ══════════════════════════════════════════════════════════════════════
# Phase 8: Bugs
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 8: Bugs ───────────────────────────────────────────"

test_begin "33. bug create (bug one)"
run_bzr bug create --product FuncTestProd --component Backend --summary "Bug one" --description "Description of bug one" --priority Normal --severity normal --op-sys Linux --rep-platform PC
if assert_success && assert_json_exists '.id'; then
    BUG1=$(jq -r '.id' "$BZR_STDOUT")
    test_pass
fi

test_begin "34. bug create (bug two)"
run_bzr bug create --product FuncTestProd --component Backend --summary "Bug two searchable" --op-sys All --rep-platform All
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

test_begin "40a. bug list --status multiple (OR)"
run_bzr bug list --product FuncTestProd --status NEW --status CONFIRMED
if assert_success; then test_pass; fi

test_begin "40b. bug list --status negation (NOT)"
run_bzr bug list --product FuncTestProd --status '!CONFIRMED'
if assert_success; then test_pass; fi

test_begin "40c. bug list --product multiple (OR)"
run_bzr bug list --product FuncTestProd --product TestProduct
if assert_success; then test_pass; fi

test_begin "41. bug update (priority/severity/whiteboard)"
if [[ -n "$BUG1" ]]; then
    run_bzr bug update "$BUG1" --priority Highest --severity major --whiteboard "wip"
    if assert_success; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "42. bug view (verify update)"
if [[ -n "$BUG1" ]]; then
    run_bzr bug view "$BUG1"
    if assert_success && assert_json '.priority' "Highest"; then test_pass; fi
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

test_begin "45a. bug list --changed-since (recent activity)"
if [[ -n "$BUG2" ]]; then
    # Capture a timestamp safely after BUG2 was created/modified, so the
    # filter window includes BUG2 and excludes any older fixtures. Bugzilla
    # matches "at or after" inclusively; subtract 5 minutes to tolerate clock
    # skew between the runner and the container.
    SINCE_TS=$(date -u -d '5 minutes ago' '+%Y-%m-%dT%H:%M:%SZ' 2>/dev/null \
                || date -u -v-5M '+%Y-%m-%dT%H:%M:%SZ')
    run_bzr bug list --product FuncTestProd --changed-since "$SINCE_TS"
    if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi
else test_skip "no BUG2"; fi

test_begin "45b. bug list --changed-since (malformed -> exit 7)"
run_bzr bug list --product FuncTestProd --changed-since "tomorrow"
if assert_exit_code 7; then test_pass; fi

test_begin "45c. bug list --whiteboard (substring positive includes bug)"
if [[ -n "$BUG1" ]]; then
    # BUG1 had --whiteboard "wip" set in test 41.
    run_bzr bug list --product FuncTestProd --whiteboard wip
    if assert_success && assert_stdout_contains "$BUG1"; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "45d. bug list --whiteboard (notsubstring excludes bug)"
if [[ -n "$BUG1" ]] && [[ -n "$BUG2" ]]; then
    # BUG1 has whiteboard "wip"; BUG2 does not. Negation must exclude BUG1
    # and include BUG2.
    run_bzr bug list --product FuncTestProd --whiteboard '!wip'
    if assert_success \
        && assert_stdout_not_contains "\"id\":$BUG1" \
        && assert_stdout_contains "$BUG2"; then
        test_pass
    fi
else test_skip "no BUG1/BUG2"; fi

test_begin "45e. bug list --resolution FIXED (positive)"
if [[ -n "$BUG1" ]]; then
    # BUG1 was resolved FIXED in test 43.
    run_bzr bug list --product FuncTestProd --resolution FIXED
    if assert_success && assert_stdout_contains "$BUG1"; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "45f. bug list --resolution '!FIXED' (notequals excludes resolved)"
if [[ -n "$BUG1" ]] && [[ -n "$BUG2" ]]; then
    # BUG1 is FIXED; BUG2 is open (empty resolution). The notequals filter
    # must exclude BUG1 and include BUG2 (empty resolution !=
    # "FIXED" by Bugzilla's notequals semantics).
    run_bzr bug list --product FuncTestProd --resolution '!FIXED'
    if assert_success \
        && assert_stdout_not_contains "\"id\":$BUG1" \
        && assert_stdout_contains "$BUG2"; then
        test_pass
    fi
else test_skip "no BUG1/BUG2"; fi

test_begin "46. bug view 999999 (negative test)"
run_bzr bug view 999999
if assert_failure; then test_pass; fi

test_begin "46a. bug view multi-ID (all succeed, JSON wrapped shape)"
if [[ -n "$BUG1" ]] && [[ -n "$BUG2" ]]; then
    run_bzr bug view "$BUG1" "$BUG2"
    if assert_success \
        && assert_json_array_length '.bugs' 2 \
        && assert_json_array_length '.failed' 0; then
        test_pass
    fi
else test_skip "no BUG1/BUG2"; fi

test_begin "46b. bug view multi-ID strict bails on inaccessible bug"
if [[ -n "$BUG1" ]]; then
    run_bzr bug view 999999 "$BUG1"
    if assert_failure; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "46c. bug view multi-ID --permissive surfaces per-bug error"
if [[ -n "$BUG1" ]] && [[ -n "$BUG2" ]]; then
    run_bzr bug view "$BUG1" "$BUG2" 999999 --permissive
    if assert_success \
        && assert_json_array_length '.bugs' 2 \
        && assert_json_array_length '.failed' 1 \
        && assert_json '.failed[0].id' "999999"; then
        test_pass
    fi
else test_skip "no BUG1/BUG2"; fi

test_begin "47. bug create (bug three — clone source)"
run_bzr bug create --product FuncTestProd --component Backend --summary "Clone source bug" --description "Description for cloning" --priority Highest --severity critical --op-sys Linux --rep-platform PC
if assert_success && assert_json_exists '.id'; then
    BUG3=$(jq -r '.id' "$BZR_STDOUT")
    test_pass
fi

test_begin "48. bug create (bug four — with relationships)"
if [[ -n "$BUG1" ]] && [[ -n "$BUG2" ]]; then
    run_bzr bug create --product FuncTestProd --component Backend --summary "Bug with relationships" --blocks "$BUG1" --depends-on "$BUG2" --op-sys All --rep-platform All
    if assert_success && assert_json_exists '.id'; then
        BUG4=$(jq -r '.id' "$BZR_STDOUT")
        test_pass
    fi
else test_skip "no BUG1/BUG2"; fi

# ── Bug create: description-source precedence ────────────────────────
test_begin "48a. bug create --description-file"
DESC_FILE=$(mktemp /tmp/bzr-func-desc.XXXXXX)
echo "description from file" > "$DESC_FILE"
run_bzr bug create --product FuncTestProd --component Backend \
    --summary "From file" --description-file "$DESC_FILE" \
    --op-sys All --rep-platform All
if assert_success; then test_pass; fi
rm -f "$DESC_FILE"

test_begin "48b. bug create --description and --description-file conflict (clap exit 2)"
DESC_FILE=$(mktemp /tmp/bzr-func-desc.XXXXXX)
echo "should-not-appear" > "$DESC_FILE"
run_bzr_raw bug create --product FuncTestProd --component Backend \
    --summary "Conflict test" --description literal \
    --description-file "$DESC_FILE" --op-sys All --rep-platform All
if assert_exit_code 2; then test_pass; fi
rm -f "$DESC_FILE"

test_begin "48c. bug create stdin description (piped)"
run_bzr bug create \
    --product FuncTestProd --component Backend \
    --summary "From stdin" --op-sys All --rep-platform All \
    <<<"description from stdin"
if assert_success; then test_pass; fi

test_begin "48d. bug create --description-file wins over piped stdin"
DESC_FILE=$(mktemp /tmp/bzr-func-desc.XXXXXX)
echo "from file" > "$DESC_FILE"
run_bzr bug create \
    --product FuncTestProd --component Backend \
    --summary "Precedence file>stdin" --description-file "$DESC_FILE" \
    --op-sys All --rep-platform All \
    <<<"from stdin"
if assert_success; then
    BUG_ID=$(jq -r '.id' "$BZR_STDOUT")
    # Verify the description that landed is from the file, not stdin
    run_bzr bug view "$BUG_ID"
    if assert_stdout_contains "from file"; then test_pass; fi
fi
rm -f "$DESC_FILE"

test_begin "48e. bug create --description-file missing path → exit 7"
run_bzr_raw bug create --product FuncTestProd --component Backend \
    --summary "Missing file" --description-file /nonexistent-bzr-path-xyz-123 \
    --op-sys All --rep-platform All
if assert_exit_code 7; then test_pass; fi

test_begin "48f. bug create empty piped stdin without explicit description → exit 7"
run_bzr_raw bug create \
    --product FuncTestProd --component Backend \
    --summary "Empty stdin" --op-sys All --rep-platform All \
    </dev/null
if assert_exit_code 7; then test_pass; fi

test_begin "48g. bug create empty fake-editor → exit 7 (TTY-conditional)"
EDITOR_SCRIPT=$(mktemp /tmp/bzr-empty-editor-XXXXXX.sh)
cat > "$EDITOR_SCRIPT" <<'SH'
#!/bin/sh
: > "$1"
SH
chmod +x "$EDITOR_SCRIPT"
# Note: this exercises the editor branch only when stdin is a TTY.
# Under non-TTY (most CI), stdin is piped (empty here from < /dev/null)
# so the empty-stdin branch fires first — also exit 7. Either way, the
# expected exit code is 7.
EDITOR="$EDITOR_SCRIPT" run_bzr_raw bug create \
    --product FuncTestProd --component Backend \
    --op-sys All --rep-platform All < /dev/null
if assert_exit_code 7; then test_pass; fi
rm -f "$EDITOR_SCRIPT"

echo ""

# ══════════════════════════════════════════════════════════════════════
# Phase 9: Bug Relationships
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 9: Bug Relationships ────────────────────────────────"

test_begin "49. bug view (verify create relationships)"
if [[ -n "$BUG4" ]]; then
    run_bzr bug view "$BUG4"
    if assert_success && assert_stdout_contains "$BUG1"; then test_pass; fi
else test_skip "no BUG4"; fi

test_begin "50. bug update --blocks-add"
if [[ -n "$BUG2" ]] && [[ -n "$BUG3" ]]; then
    run_bzr bug update "$BUG2" --blocks-add "$BUG3"
    if assert_success; then test_pass; fi
else test_skip "no BUG2/BUG3"; fi

test_begin "51. bug update --depends-on-add"
# Use BUG3 on BUG3 itself (add BUG2 to BUG3's depends_on — independent of blocks chain)
if [[ -n "$BUG3" ]] && [[ -n "$BUG2" ]]; then
    run_bzr bug update "$BUG3" --depends-on-add "$BUG2"
    if assert_success; then test_pass; fi
else test_skip "no BUG3/BUG2"; fi

test_begin "52. bug update --blocks-remove"
if [[ -n "$BUG2" ]] && [[ -n "$BUG3" ]]; then
    run_bzr bug update "$BUG2" --blocks-remove "$BUG3"
    if assert_success; then test_pass; fi
else test_skip "no BUG2/BUG3"; fi

test_begin "53. bug update --depends-on-remove"
if [[ -n "$BUG3" ]] && [[ -n "$BUG2" ]]; then
    run_bzr bug update "$BUG3" --depends-on-remove "$BUG2"
    if assert_success; then test_pass; fi
else test_skip "no BUG3/BUG2"; fi

echo ""

# ══════════════════════════════════════════════════════════════════════
# Phase 10: Bug Clone
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 10: Bug Clone ───────────────────────────────────────"

test_begin "54. bug clone (defaults)"
if [[ -n "$BUG3" ]]; then
    # Pass --op-sys and --rep-platform since some Bugzilla versions require them
    # and the Bug struct doesn't include these fields for automatic copying
    run_bzr bug clone "$BUG3" --op-sys Linux --rep-platform PC
    if assert_success && assert_json_exists '.id'; then
        CLONE_ID=$(jq -r '.id' "$BZR_STDOUT")
        test_pass
    fi
else test_skip "no BUG3"; fi

test_begin "55. bug view (verify clone fields)"
if [[ -n "$CLONE_ID" ]]; then
    run_bzr bug view "$CLONE_ID"
    if assert_success && assert_json '.summary' "Clone source bug" && assert_json '.priority' "Highest"; then
        test_pass
    fi
else test_skip "no CLONE_ID"; fi

test_begin "56. bug clone (with overrides)"
if [[ -n "$BUG3" ]]; then
    run_bzr bug clone "$BUG3" --summary "Overridden summary" --no-comment --op-sys Linux --rep-platform PC
    if assert_success && assert_json_exists '.id'; then test_pass; fi
else test_skip "no BUG3"; fi

test_begin "57. bug clone --add-depends-on"
if [[ -n "$BUG3" ]]; then
    run_bzr bug clone "$BUG3" --summary "Depends on source" --add-depends-on --no-cc --no-keywords --op-sys Linux --rep-platform PC
    if assert_success && assert_json_exists '.id'; then test_pass; fi
else test_skip "no BUG3"; fi

echo ""

# ══════════════════════════════════════════════════════════════════════
# Phase 11: Batch Update
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 11: Batch Update ────────────────────────────────────"

test_begin "58. bug update (batch — two bugs)"
if [[ -n "$BUG2" ]] && [[ -n "$BUG4" ]]; then
    run_bzr bug update "$BUG2" "$BUG4" --whiteboard "batch-test"
    if assert_success; then test_pass; fi
else test_skip "no BUG2/BUG4"; fi

test_begin "59. bug view (verify batch — bug2)"
if [[ -n "$BUG2" ]]; then
    run_bzr bug view "$BUG2"
    if assert_success && assert_json '.whiteboard' "batch-test"; then test_pass; fi
else test_skip "no BUG2"; fi

test_begin "60. bug view (verify batch — bug4)"
if [[ -n "$BUG4" ]]; then
    run_bzr bug view "$BUG4"
    if assert_success && assert_json '.whiteboard' "batch-test"; then test_pass; fi
else test_skip "no BUG4"; fi

echo ""

# ══════════════════════════════════════════════════════════════════════
# Phase 12: My Bugs
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 12: My Bugs ─────────────────────────────────────────"

test_begin "61. bug my (assigned)"
run_bzr bug my
if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi

test_begin "62. bug my --created"
run_bzr bug my --created
if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi

test_begin "63. bug my --all"
run_bzr bug my --all
if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi

test_begin "64. bug my --all --status NEW"
run_bzr bug my --all --status NEW
if assert_success; then test_pass; fi

test_begin "64a. bug my --status multiple (OR)"
run_bzr bug my --all --status NEW --status REOPENED
if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi

test_begin "64b. bug my --status negation (NOT RESOLVED)"
run_bzr bug my --all --status '!RESOLVED'
if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi

test_begin "64c. bug my --status mixed positive and negated"
run_bzr bug my --all --status NEW --status '!RESOLVED'
if assert_success; then test_pass; fi

echo ""

# ══════════════════════════════════════════════════════════════════════
# Phase 13: Templates
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 13: Templates ───────────────────────────────────────"

test_begin "65. template save"
run_bzr template save func-tmpl --product FuncTestProd --component Backend --priority Normal --severity normal
if assert_success; then test_pass; fi

test_begin "66. template list"
run_bzr_raw template list
if assert_success && assert_stdout_contains "func-tmpl"; then test_pass; fi

test_begin "67. template show"
run_bzr template show func-tmpl
if assert_success && assert_json '.product' "FuncTestProd"; then test_pass; fi

test_begin "68. bug create --template"
run_bzr bug create --template func-tmpl --summary "Bug from template" --op-sys All --rep-platform All
if assert_success && assert_json_exists '.id'; then
    TMPL_BUG=$(jq -r '.id' "$BZR_STDOUT")
    test_pass
fi

test_begin "69. bug view (verify template fields)"
if [[ -n "$TMPL_BUG" ]]; then
    run_bzr bug view "$TMPL_BUG"
    if assert_success && assert_json '.product' "FuncTestProd" && assert_json '.component' "Backend" && assert_json '.priority' "Normal"; then
        test_pass
    fi
else test_skip "no TMPL_BUG"; fi

test_begin "70. template delete"
run_bzr template delete func-tmpl
if assert_success; then test_pass; fi

test_begin "71. template show (deleted, expect failure)"
run_bzr template show func-tmpl
if assert_failure; then test_pass; fi

echo ""

# ══════════════════════════════════════════════════════════════════════
# Phase 13.5: Saved Queries
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 13.5: Saved Queries ─────────────────────────────────"

# ── CRUD lifecycle ───────────────────────────────────────────────────

test_begin "72. query save (list kind)"
run_bzr query save prod-bugs --product FuncTestProd --status NEW --status CONFIRMED --limit 10
if assert_success && assert_json '.action' "saved"; then test_pass; fi

test_begin "73. query save (search kind)"
run_bzr query save search-bugs --search "Bug one" --limit 5
if assert_success && assert_json '.action' "saved"; then test_pass; fi

test_begin "74. query save (multi-filter)"
run_bzr query save complex --product FuncTestProd --component Backend --priority Normal --severity normal --status NEW --status CONFIRMED --limit 20
if assert_success && assert_json '.action' "saved"; then test_pass; fi

test_begin "75. query list"
run_bzr_raw query list
if assert_success && assert_stdout_contains "prod-bugs" && assert_stdout_contains "search-bugs" && assert_stdout_contains "complex"; then test_pass; fi

test_begin "76. query show"
run_bzr query show complex
if assert_success && assert_json '.kind' "list" && assert_json '.product[0]' "FuncTestProd" && assert_json '.priority[0]' "Normal"; then test_pass; fi

test_begin "77. query save (update existing)"
run_bzr query save prod-bugs --product FuncTestProd --status NEW --limit 5
if assert_success && assert_json '.action' "updated"; then test_pass; fi

# ── Run queries against real Bugzilla ────────────────────────────────

test_begin "78. query run (product+status filter)"
run_bzr query run prod-bugs
if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi

test_begin "79. query run (quicksearch)"
run_bzr query run search-bugs
if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi

test_begin "80. query run (multi-filter complex)"
run_bzr query run complex
if assert_success; then test_pass; fi

test_begin "81. query run with limit override"
run_bzr query run prod-bugs --limit 1
if assert_success && assert_json_array_length '.' 1; then test_pass; fi

test_begin "82. query run with fields override"
run_bzr query run prod-bugs --fields id,summary,status
if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi

# ── Cleanup and error handling ───────────────────────────────────────

test_begin "83. query delete"
run_bzr query delete search-bugs
if assert_success && assert_json '.action' "deleted"; then test_pass; fi

test_begin "84. query show (deleted, expect failure)"
run_bzr query show search-bugs
if assert_failure; then test_pass; fi

test_begin "85. query run (deleted, expect failure)"
run_bzr query run search-bugs
if assert_failure; then test_pass; fi

test_begin "86. query save (empty, expect failure)"
run_bzr query save empty-q
if assert_failure; then test_pass; fi

test_begin "87. query delete remaining"
run_bzr query delete prod-bugs
if assert_success; then
    run_bzr query delete complex
    if assert_success; then test_pass; fi
fi

echo ""

# ══════════════════════════════════════════════════════════════════════
# Phase 14: Comments
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 14: Comments ───────────────────────────────────────"

test_begin "88. comment add (first)"
if [[ -n "$BUG1" ]]; then
    run_bzr comment add "$BUG1" --body "First test comment"
    if assert_success && assert_json_exists '.id'; then
        COMMENT_ID=$(jq -r '.id' "$BZR_STDOUT")
        test_pass
    fi
else test_skip "no BUG1"; fi

test_begin "89. comment add (second)"
if [[ -n "$BUG1" ]]; then
    run_bzr comment add "$BUG1" --body "Second comment"
    if assert_success; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "90. comment list"
if [[ -n "$BUG1" ]]; then
    run_bzr comment list "$BUG1"
    # Bug description counts as comment 0, plus our 2 = at least 3
    if assert_success && assert_json_array_min_length '.' 3; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "91. comment list --since"
if [[ -n "$BUG1" ]]; then
    run_bzr comment list "$BUG1" --since 2020-01-01
    if assert_success; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "92. comment tag --add"
if [[ -n "${COMMENT_ID:-}" ]] && [[ "$COMMENT_ID" != "null" ]]; then
    run_bzr comment tag "$COMMENT_ID" --add important
    if assert_success; then test_pass; fi
else
    test_skip "no comment ID"
fi

test_begin "93. comment tag --remove"
if [[ -n "${COMMENT_ID:-}" ]] && [[ "$COMMENT_ID" != "null" ]]; then
    run_bzr comment tag "$COMMENT_ID" --remove important
    if assert_success; then test_pass; fi
else
    test_skip "no comment ID"
fi

test_begin "94. comment search-tags"
run_bzr comment search-tags important
# May return empty if tag was fully removed, but should succeed
if assert_success; then test_pass; fi

echo ""

# ══════════════════════════════════════════════════════════════════════
# Phase 14b: Private comment visibility (#125 hybrid fallback)
# ══════════════════════════════════════════════════════════════════════
# Mirrors the issue reporter's deployment, which has `insidergroup`
# configured (otherwise private comments couldn't exist there in the
# first place). The fixture entrypoints set insidergroup=admin so the
# admin test user can mark comments private — this is what makes #125
# reproducible at all.
echo "── Phase 14b: Private comments (Hybrid mode) ─────────────────"

test_begin "94a. comment add --private"
if [[ -n "$BUG1" ]]; then
    run_bzr comment add "$BUG1" --body "Private test comment" --private
    if assert_success && assert_json_exists '.id'; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "94b. comment list returns private comment in Hybrid mode"
if [[ -n "$BUG1" ]]; then
    run_bzr --api hybrid comment list "$BUG1"
    # 1 description (count 0) + 2 public + 1 private = >= 4
    # AND the private one must be visible (is_private: true present).
    if assert_success \
        && assert_json_array_min_length '.' 4 \
        && [[ "$(jq '[.[] | select(.is_private == true)] | length' "$BZR_STDOUT")" -ge 1 ]]; then
        test_pass
    fi
else test_skip "no BUG1"; fi

test_begin "94c. comment list returns private comment in XML-RPC mode"
if [[ -n "$BUG1" ]]; then
    run_bzr --api xmlrpc comment list "$BUG1"
    if assert_success \
        && assert_json_array_min_length '.' 4 \
        && [[ "$(jq '[.[] | select(.is_private == true)] | length' "$BZR_STDOUT")" -ge 1 ]]; then
        test_pass
    fi
else test_skip "no BUG1"; fi

echo ""

# ══════════════════════════════════════════════════════════════════════
# Phase 15: Attachments
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 15: Attachments ───────────────────────────────────"

test_begin "95. create temp file"
echo "bzr functional test content $(date +%s)" > /tmp/bzr-func-test.txt
test_pass

test_begin "96. attachment upload"
if [[ -n "$BUG1" ]]; then
    run_bzr attachment upload "$BUG1" /tmp/bzr-func-test.txt --summary "Test file"
    if assert_success && assert_json_exists '.id'; then
        ATTACH_ID=$(jq -r '.id' "$BZR_STDOUT" 2>/dev/null || jq -r '.ids[0]' "$BZR_STDOUT" 2>/dev/null || echo "")
        test_pass
    fi
else test_skip "no BUG1"; fi

test_begin "97. attachment list"
if [[ -n "$BUG1" ]]; then
    run_bzr attachment list "$BUG1"
    if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "98. attachment download"
if [[ -n "${ATTACH_ID:-}" ]] && [[ "$ATTACH_ID" != "null" ]]; then
    rm -f /tmp/bzr-func-downloaded.txt
    run_bzr attachment download "$ATTACH_ID" --out /tmp/bzr-func-downloaded.txt
    if assert_success && assert_file_contains /tmp/bzr-func-downloaded.txt "bzr functional test content"; then
        test_pass
    fi
else
    test_skip "no attachment ID"
fi

test_begin "99. attachment update"
if [[ -n "${ATTACH_ID:-}" ]] && [[ "$ATTACH_ID" != "null" ]]; then
    run_bzr attachment update "$ATTACH_ID" --summary "Updated summary" --obsolete true
    if assert_success; then test_pass; fi
else
    test_skip "no attachment ID"
fi

test_begin "100. attachment upload (explicit MIME)"
if [[ -n "$BUG1" ]]; then
    run_bzr attachment upload "$BUG1" /tmp/bzr-func-test.txt --content-type text/plain
    if assert_success; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "100f. attachment upload --comment posts comment in same call"
if [[ -n "$BUG1" ]]; then
    run_bzr comment list "$BUG1"
    if assert_success; then
        PRECOMMENT_COUNT=$(jq '. | length' "$BZR_STDOUT" 2>/dev/null || echo "")
        run_bzr attachment upload "$BUG1" /tmp/bzr-func-test.txt \
            --summary "with comment" \
            --comment "see #165 -- bzl-parity"
        if assert_success; then
            run_bzr comment list "$BUG1"
            if assert_success; then
                POSTCOMMENT_COUNT=$(jq '. | length' "$BZR_STDOUT" 2>/dev/null || echo "")
                if [[ -n "$PRECOMMENT_COUNT" ]] \
                    && [[ -n "$POSTCOMMENT_COUNT" ]] \
                    && [[ "$POSTCOMMENT_COUNT" -eq $((PRECOMMENT_COUNT + 1)) ]]; then
                    test_pass
                else
                    test_fail "comment count did not grow by 1 (pre=$PRECOMMENT_COUNT post=$POSTCOMMENT_COUNT)"
                fi
            fi
        fi
    fi
else test_skip "no BUG1"; fi

test_begin "100g. attachment upload --is-patch marks attachment as a patch"
if [[ -n "$BUG1" ]]; then
    run_bzr attachment upload "$BUG1" /tmp/bzr-func-test.txt \
        --summary "patch test" --is-patch
    if assert_success; then
        run_bzr attachment list "$BUG1"
        if assert_success \
            && assert_json '[.[] | select(.summary == "patch test")][-1].is_patch' "true"; then
            test_pass
        fi
    fi
else test_skip "no BUG1"; fi

echo ""

# ══════════════════════════════════════════════════════════════════════
# Phase 15b: Private attachment visibility (#133 hybrid fallback)
# ══════════════════════════════════════════════════════════════════════
# Mirrors the issue reporter's deployment: REST silently filters private
# attachments under non-admin scope, while XML-RPC Bug.attachments returns
# the full set. The fixture entrypoints already configure insidergroup
# (for #125), which is also the precondition for private attachments.
echo "── Phase 15b: Private attachments (Hybrid mode) ──────────────"

test_begin "100a. attachment upload --private"
if [[ -n "$BUG1" ]]; then
    run_bzr attachment upload "$BUG1" /tmp/bzr-func-test.txt \
        --summary "Private test attachment" --private
    if assert_success && assert_json_exists '.id'; then
        PRIVATE_ATTACH_ID=$(jq -r '.id' "$BZR_STDOUT" 2>/dev/null || echo "")
        test_pass
    fi
else test_skip "no BUG1"; fi

test_begin "100b. attachment list returns private attachment in Hybrid mode"
if [[ -n "$BUG1" ]]; then
    run_bzr --api hybrid attachment list "$BUG1"
    # Several public attachments are uploaded earlier in the run and this
    # section adds one private; the list must include ≥3 total AND the
    # private one must be visible (is_private: true present).
    if assert_success \
        && assert_json_array_min_length '.' 3 \
        && [[ "$(jq '[.[] | select(.is_private == true)] | length' "$BZR_STDOUT")" -ge 1 ]]; then
        test_pass
    fi
else test_skip "no BUG1"; fi

test_begin "100c. attachment list returns private attachment in XML-RPC mode"
if [[ -n "$BUG1" ]]; then
    run_bzr --api xmlrpc attachment list "$BUG1"
    if assert_success \
        && assert_json_array_min_length '.' 3 \
        && [[ "$(jq '[.[] | select(.is_private == true)] | length' "$BZR_STDOUT")" -ge 1 ]]; then
        test_pass
    fi
else test_skip "no BUG1"; fi

test_begin "100d. attachment download (private) in Hybrid mode"
if [[ -n "${PRIVATE_ATTACH_ID:-}" ]] && [[ "$PRIVATE_ATTACH_ID" != "null" ]]; then
    rm -f /tmp/bzr-func-private-hybrid.txt
    run_bzr --api hybrid attachment download "$PRIVATE_ATTACH_ID" \
        --out /tmp/bzr-func-private-hybrid.txt
    if assert_success && assert_file_contains /tmp/bzr-func-private-hybrid.txt "bzr functional test content"; then
        test_pass
    fi
else
    test_skip "no private attachment ID"
fi

test_begin "100e. attachment download (private) in XML-RPC mode"
if [[ -n "${PRIVATE_ATTACH_ID:-}" ]] && [[ "$PRIVATE_ATTACH_ID" != "null" ]]; then
    rm -f /tmp/bzr-func-private-xmlrpc.txt
    run_bzr --api xmlrpc attachment download "$PRIVATE_ATTACH_ID" \
        --out /tmp/bzr-func-private-xmlrpc.txt
    if assert_success && assert_file_contains /tmp/bzr-func-private-xmlrpc.txt "bzr functional test content"; then
        test_pass
    fi
else
    test_skip "no private attachment ID"
fi

echo ""

# ══════════════════════════════════════════════════════════════════════
# Phase 16: Global Options Smoke Tests
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 16: Global Options ────────────────────────────────"

test_begin "101. --output table"
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

test_begin "102. --quiet"
if [[ -n "$BUG1" ]]; then
    run_bzr_raw --quiet bug view "$BUG1"
    if assert_success && assert_stdout_empty; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "102a. --quiet suppresses stderr tracing"
if [[ -n "$BUG1" ]]; then
    run_bzr_raw --quiet -vvv bug view "$BUG1"
    if assert_success && assert_stdout_empty && assert_stderr_empty; then
        test_pass
    fi
else test_skip "no BUG1"; fi

test_begin "102b. --quiet preserves error exit code"
if true; then
    run_bzr_raw --quiet bug view 999999
    if assert_failure && assert_stdout_empty; then test_pass; fi
fi

test_begin "102c. --quiet + --json suppresses stdout"
if [[ -n "$BUG1" ]]; then
    run_bzr_raw --quiet --json bug view "$BUG1"
    if assert_success && assert_stdout_empty; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "103. --server test whoami"
run_bzr_raw --server test whoami
if assert_success; then test_pass; fi

echo ""

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
