# 08-bugs
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

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
run_bzr bug create --product FuncTestProd --component Backend \
    --summary "Bug two searchable" --description "Description of bug two" \
    --op-sys All --rep-platform All
if assert_success && assert_json_exists '.id'; then
    BUG2=$(jq -r '.id' "$BZR_STDOUT")
    test_pass
fi

test_begin "34a. bug create (duplicate target)"
run_bzr bug create --product FuncTestProd --component Backend \
    --summary "Duplicate target" --description "Duplicate target description" \
    --op-sys All --rep-platform All
if assert_success && assert_json_exists '.id'; then
    BUG_DUP_TARGET=$(jq -r '.id' "$BZR_STDOUT")
    test_pass
fi

test_begin "34b. bug create (duplicate source)"
run_bzr bug create --product FuncTestProd --component Backend \
    --summary "Duplicate source" --description "Duplicate source description" \
    --op-sys All --rep-platform All
if assert_success && assert_json_exists '.id'; then
    BUG_DUP_SOURCE=$(jq -r '.id' "$BZR_STDOUT")
    test_pass
fi

test_begin "34c. bug update --dupe-of"
if [[ -n "$BUG_DUP_SOURCE" ]] && [[ -n "$BUG_DUP_TARGET" ]]; then
    run_bzr bug update "$BUG_DUP_SOURCE" --dupe-of "$BUG_DUP_TARGET"
    if assert_success; then test_pass; fi
else test_skip "no duplicate source/target"; fi

test_begin "34d. bug view verifies duplicate transition"
if [[ -n "$BUG_DUP_SOURCE" ]] && [[ -n "$BUG_DUP_TARGET" ]]; then
    run_bzr bug view "$BUG_DUP_SOURCE" --json
    if assert_success &&
        assert_json '.status' "RESOLVED" &&
        assert_json '.resolution' "DUPLICATE" &&
        assert_json '.dupe_of' "$BUG_DUP_TARGET"; then
        test_pass
    fi
else test_skip "no duplicate source/target"; fi

test_begin "35. bug view"
if [[ -n "$BUG1" ]]; then
    run_bzr bug view "$BUG1"
    if assert_success && assert_json '.summary' "Bug one"; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "35a. credentialless named bug view"
if [[ -n "$BUG1" ]]; then
    run_bzr_raw --json --server public bug view "$BUG1"
    if assert_success && assert_json '.summary' "Bug one"; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "35b. inline credentialless bug list"
run_bzr_raw --json --server-url "$BZ_URL" bug list --product FuncTestProd --limit 1
if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi

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

test_begin "42a. bug update --deadline"
if [[ -n "$BUG1" ]]; then
    run_bzr bug update "$BUG1" --deadline 2026-12-31
    if assert_success; then
        run_bzr bug view "$BUG1" --json
        if assert_success && assert_json '.deadline' "2026-12-31"; then
            test_pass
        fi
    fi
else test_skip "no BUG1"; fi

test_begin "42c. bug update --url and --target-milestone"
if [[ -n "$BUG1" ]]; then
    run_bzr bug update "$BUG1" --url "http://example.com/updated-$BUG1" \
        --target-milestone=---
    if assert_success; then
        run_bzr bug view "$BUG1"
        if assert_json '.url' "http://example.com/updated-$BUG1" &&
            assert_json '.target_milestone' "---"; then test_pass; fi
    fi
else test_skip "no BUG1"; fi

test_begin "42b. bug update reset assignee and QA contact"
if [[ -n "$BUG1" ]]; then
    run_bzr bug update "$BUG1" --reset-assigned-to --reset-qa-contact
    if assert_success; then test_pass; fi
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
    SINCE_TS=$(date -u -d '5 minutes ago' '+%Y-%m-%dT%H:%M:%SZ' 2>/dev/null ||
        date -u -v-5M '+%Y-%m-%dT%H:%M:%SZ')
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
    if assert_success &&
        assert_stdout_not_contains "\"id\":$BUG1" &&
        assert_stdout_contains "$BUG2"; then
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
    if assert_success &&
        assert_stdout_not_contains "\"id\":$BUG1" &&
        assert_stdout_contains "$BUG2"; then
        test_pass
    fi
else test_skip "no BUG1/BUG2"; fi

test_begin "46. bug view 999999 (negative test)"
run_bzr bug view 999999
if assert_failure; then test_pass; fi

test_begin "46a. bug view multi-ID (all succeed, JSON wrapped shape)"
if [[ -n "$BUG1" ]] && [[ -n "$BUG2" ]]; then
    run_bzr bug view "$BUG1" "$BUG2"
    if assert_success &&
        assert_json_array_length '.bugs' 2 &&
        assert_json_array_length '.failed' 0; then
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
    if assert_success &&
        assert_json_array_length '.bugs' 2 &&
        assert_json_array_length '.failed' 1 &&
        assert_json '.failed[0].id' "999999"; then
        test_pass
    fi
else test_skip "no BUG1/BUG2"; fi

test_begin "47. bug create (bug three — clone source)"
run_bzr bug create --product FuncTestProd --component Backend --summary "Clone source bug" --description "Description for cloning" --priority Highest --severity critical --op-sys Linux --rep-platform PC
if assert_success && assert_json_exists '.id'; then
    # shellcheck disable=SC2034 # consumed by later sourced phases
    BUG3=$(jq -r '.id' "$BZR_STDOUT")
    test_pass
fi

test_begin "48. bug create (bug four — with relationships)"
if [[ -n "$BUG1" ]] && [[ -n "$BUG2" ]]; then
    run_bzr bug create --product FuncTestProd --component Backend \
        --summary "Bug with relationships" --description "Relationship test description" \
        --blocks "$BUG1" --depends-on "$BUG2" --op-sys All --rep-platform All
    if assert_success && assert_json_exists '.id'; then
        # shellcheck disable=SC2034 # consumed by later sourced phases
        BUG4=$(jq -r '.id' "$BZR_STDOUT")
        test_pass
    fi
else test_skip "no BUG1/BUG2"; fi

# ── Bug create: description-source precedence ────────────────────────
test_begin "48a. bug create --description-file"
DESC_FILE=$(mktemp /tmp/bzr-func-desc.XXXXXX)
echo "description from file" >"$DESC_FILE"
run_bzr bug create --product FuncTestProd --component Backend \
    --summary "From file" --description-file "$DESC_FILE" \
    --op-sys All --rep-platform All
if assert_success; then test_pass; fi
rm -f "$DESC_FILE"

test_begin "48b. bug create --description and --description-file conflict (clap exit 2)"
DESC_FILE=$(mktemp /tmp/bzr-func-desc.XXXXXX)
echo "should-not-appear" >"$DESC_FILE"
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
echo "from file" >"$DESC_FILE"
run_bzr bug create \
    --product FuncTestProd --component Backend \
    --summary "Precedence file>stdin" --description-file "$DESC_FILE" \
    --op-sys All --rep-platform All \
    <<<"from stdin"
if assert_success; then
    BUG_ID=$(jq -r '.id' "$BZR_STDOUT")
    # Verify the description that landed is from the file, not stdin
    run_bzr comment list "$BUG_ID"
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
cat >"$EDITOR_SCRIPT" <<'SH'
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
    --op-sys All --rep-platform All </dev/null
if assert_exit_code 7; then test_pass; fi
rm -f "$EDITOR_SCRIPT"

echo ""
