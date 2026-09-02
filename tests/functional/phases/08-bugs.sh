# 08-bugs
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 8: Bugs
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 8: Bugs ───────────────────────────────────────────"

test_begin "bug-create-bug-one" "bug create (bug one)"
run_bzr bug create --product FuncTestProd --component Backend --summary "Bug one" --description "Description of bug one" --priority Normal --severity normal --op-sys Linux --rep-platform PC
if assert_success && assert_json_exists '.id'; then
    BUG1=$(jq -r '.id' "$BZR_STDOUT")
    test_pass
fi

test_begin "bug-create-bug-two" "bug create (bug two)"
run_bzr bug create --product FuncTestProd --component Backend \
    --summary "Bug two searchable" --description "Description of bug two" \
    --op-sys All --rep-platform All
if assert_success && assert_json_exists '.id'; then
    BUG2=$(jq -r '.id' "$BZR_STDOUT")
    test_pass
fi

test_begin "bug-create-duplicate-target" "bug create (duplicate target)"
run_bzr bug create --product FuncTestProd --component Backend \
    --summary "Duplicate target" --description "Duplicate target description" \
    --op-sys All --rep-platform All
if assert_success && assert_json_exists '.id'; then
    BUG_DUP_TARGET=$(jq -r '.id' "$BZR_STDOUT")
    test_pass
fi

test_begin "bug-create-duplicate-source" "bug create (duplicate source)"
run_bzr bug create --product FuncTestProd --component Backend \
    --summary "Duplicate source" --description "Duplicate source description" \
    --op-sys All --rep-platform All
if assert_success && assert_json_exists '.id'; then
    BUG_DUP_SOURCE=$(jq -r '.id' "$BZR_STDOUT")
    test_pass
fi

test_begin "bug-update-dupe-of" "bug update --dupe-of"
if [[ -n "$BUG_DUP_SOURCE" ]] && [[ -n "$BUG_DUP_TARGET" ]]; then
    run_bzr bug update "$BUG_DUP_SOURCE" --dupe-of "$BUG_DUP_TARGET"
    if assert_success; then test_pass; fi
else test_skip "no duplicate source/target"; fi

test_begin "bug-view-verifies-duplicate-transition" "bug view verifies duplicate transition"
if [[ -n "$BUG_DUP_SOURCE" ]] && [[ -n "$BUG_DUP_TARGET" ]]; then
    run_bzr bug view "$BUG_DUP_SOURCE" --json
    if assert_success &&
        assert_json '.status' "RESOLVED" &&
        assert_json '.resolution' "DUPLICATE" &&
        assert_json '.dupe_of' "$BUG_DUP_TARGET"; then
        test_pass
    fi
else test_skip "no duplicate source/target"; fi

test_begin "bug-view" "bug view"
if [[ -n "$BUG1" ]]; then
    run_bzr bug view "$BUG1"
    if assert_success && assert_json '.summary' "Bug one"; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "credentialless-named-bug-view" "credentialless named bug view"
if [[ -n "$BUG1" ]]; then
    run_bzr_raw --json --server public bug view "$BUG1"
    if assert_success && assert_json '.summary' "Bug one"; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "inline-credentialless-bug-list" "inline credentialless bug list"
run_bzr_raw --json --server-url "$BZ_URL" bug list --product FuncTestProd --limit 1
if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi

test_begin "stateless-inline-from-url-search" "stateless inline from-url search"
if [[ -n "$BUG1" ]]; then
    _INLINE_SEARCH_CONFIG="$FUNC_CONFIG_DIR/inline-search-empty.toml"
    run_bzr_raw --json --config "$_INLINE_SEARCH_CONFIG" --server-url "$BZ_URL" \
        bug search --from-url "${BZ_URL}/buglist.cgi?bug_id=${BUG1}"
    if assert_success && assert_json_array_min_length '.' 1 &&
        assert_stderr_not_contains "does not match inline server hostname"; then
        test_pass
    fi
    unset _INLINE_SEARCH_CONFIG
else
    test_skip "no BUG1"
fi

test_begin "stateless-inline-from-url-mismatch-guidance" "stateless inline from-url mismatch guidance"
if [[ -n "$BUG1" ]]; then
    _INLINE_SEARCH_CONFIG="$FUNC_CONFIG_DIR/inline-search-empty.toml"
    run_bzr_raw --json --config "$_INLINE_SEARCH_CONFIG" --server-url "$BZ_URL" \
        bug search --from-url "http://localhost:${BZ_PORT}/buglist.cgi?bug_id=${BUG1}"
    if assert_success && assert_json_array_min_length '.' 1 &&
        assert_stderr_contains "URL hostname 'localhost'" &&
        assert_stderr_contains "inline server hostname '127.0.0.1'" &&
        assert_stderr_contains "using inline server"; then
        test_pass
    fi
    unset _INLINE_SEARCH_CONFIG
else
    test_skip "no BUG1"
fi

test_begin "bug-view-fields" "bug view --fields"
if [[ -n "$BUG1" ]]; then
    run_bzr bug view "$BUG1" --fields id,summary
    if assert_success && assert_json_exists '.id' && assert_json_exists '.summary'; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "bug-view-time-fields-round-trip" "bug update/view time fields round-trip"
if [[ -n "$BUG1" ]]; then
    run_bzr bug update "$BUG1" --estimated-time 8 --remaining-time 5
    if assert_success; then
        run_bzr bug view "$BUG1"
        if assert_success &&
            assert_json '.groups | length' "0" &&
            assert_json '.estimated_time' "8.0" &&
            assert_json '.remaining_time' "5.0"; then test_pass; fi
    fi
else test_skip "no BUG1"; fi

test_begin "bug-view-read-fields-project" "bug view projects groups and time fields"
if [[ -n "$BUG1" ]]; then
    run_bzr bug view "$BUG1" --fields groups,estimated_time,remaining_time
    if assert_success &&
        assert_json 'keys == ["estimated_time", "groups", "remaining_time"]' "true" &&
        assert_json '.estimated_time' "8.0" &&
        assert_json '.remaining_time' "5.0"; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "credentialless-bug-view-omits-time-fields" "credentialless bug view omits permission-gated time fields"
if [[ -n "$BUG1" ]]; then
    run_bzr_raw --json --server public bug view "$BUG1"
    if assert_success &&
        assert_json 'has("estimated_time")' "false" &&
        assert_json 'has("remaining_time")' "false"; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "bug-view-groups-round-trip-and-project" "bug update/view groups round-trip and project"
_READ_FIELDS_BUG=$(make_bug --product FuncTestProd --component Backend \
    --summary "Read fields group round-trip" --description "read fields" \
    --op-sys All --rep-platform All)
if [[ -n "$_READ_FIELDS_BUG" ]]; then
    run_bzr bug update "$_READ_FIELDS_BUG" --groups-add functest-grp
    if assert_success; then
        run_bzr bug view "$_READ_FIELDS_BUG"
        if assert_success &&
            assert_json '.groups | index("functest-grp") != null' "true"; then
            run_bzr bug view "$_READ_FIELDS_BUG" --fields groups
            if assert_success &&
                assert_json 'keys == ["groups"]' "true" &&
                assert_json '.groups | index("functest-grp") != null' "true"; then test_pass; fi
        fi
    fi
else test_skip "group round-trip bug was not created"; fi
unset _READ_FIELDS_BUG

test_begin "bug-list-product" "bug list --product"
run_bzr bug list --product FuncTestProd
if assert_success && assert_json_array_min_length '.' 2; then test_pass; fi

test_begin "bug-list-status-new-limit-1" "bug list --status NEW --limit 1"
run_bzr bug list --product FuncTestProd --status NEW --limit 1
if assert_success && assert_json_array_length '.' 1; then test_pass; fi

test_begin "bug-list-id-multiple" "bug list --id multiple"
if [[ -n "$BUG1" ]] && [[ -n "$BUG2" ]]; then
    run_bzr bug list --id "$BUG1" --id "$BUG2"
    if assert_success && assert_json_array_length '.' 2; then test_pass; fi
else test_skip "no BUG1/BUG2"; fi

test_begin "bug-search" "bug search"
run_bzr bug search "Bug two searchable"
if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi

test_begin "bug-list-status-multiple-or" "bug list --status multiple (OR)"
run_bzr bug list --product FuncTestProd --status NEW --status CONFIRMED
if assert_success; then test_pass; fi

test_begin "bug-list-status-negation-not" "bug list --status negation (NOT)"
run_bzr bug list --product FuncTestProd --status '!CONFIRMED'
if assert_success; then test_pass; fi

test_begin "bug-list-product-multiple-or" "bug list --product multiple (OR)"
run_bzr bug list --product FuncTestProd --product TestProduct
if assert_success; then test_pass; fi

test_begin "bug-update-priority-severity-whiteboard" "bug update (priority/severity/whiteboard)"
if [[ -n "$BUG1" ]]; then
    run_bzr bug update "$BUG1" --priority Highest --severity major --whiteboard "wip"
    if assert_success; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "bug-view-verify-update" "bug view (verify update)"
if [[ -n "$BUG1" ]]; then
    run_bzr bug view "$BUG1"
    if assert_success && assert_json '.priority' "Highest"; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "bug-update-deadline" "bug update --deadline"
if [[ -n "$BUG1" ]]; then
    run_bzr bug update "$BUG1" --deadline 2026-12-31
    if assert_success; then
        run_bzr bug view "$BUG1" --json
        if assert_success && assert_json '.deadline' "2026-12-31"; then
            test_pass
        fi
    fi
else test_skip "no BUG1"; fi

test_begin "bug-update-url-and-target-milestone" "bug update --url and --target-milestone"
if [[ -n "$BUG1" ]]; then
    run_bzr bug update "$BUG1" --url "http://example.com/updated-$BUG1" \
        --target-milestone=---
    if assert_success; then
        run_bzr bug view "$BUG1"
        if assert_json '.url' "http://example.com/updated-$BUG1" &&
            assert_json '.target_milestone' "---"; then test_pass; fi
    fi
else test_skip "no BUG1"; fi

test_begin "bug-update-reset-assignee-and-qa-contact" "bug update reset assignee and QA contact"
if [[ -n "$BUG1" ]]; then
    run_bzr bug update "$BUG1" --reset-assigned-to --reset-qa-contact
    if assert_success; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "bug-update-resolve" "bug update (resolve)"
if [[ -n "$BUG1" ]]; then
    run_bzr bug update "$BUG1" --status RESOLVED --resolution FIXED
    if assert_success; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "bug-history-flattened-json-change-records" "bug history (flattened JSON change records)"
if [[ -n "$BUG1" ]]; then
    # BUG1 has been mutated by earlier tests, so its history is non-empty. The
    # --json body must be a flat array of per-field records — never the grouped
    # {who, when, changes:[...]} wire shape.
    run_bzr bug history "$BUG1"
    if assert_success &&
        assert_json_array_min_length '.' 1 &&
        assert_json '.[0] | has("field")' "true" &&
        assert_json '.[0] | has("who")' "true" &&
        assert_json '.[0] | has("old_value")' "true" &&
        assert_json '.[0] | has("new_value")' "true" &&
        assert_json '.[0] | has("comment_id")' "true" &&
        assert_json '[.[] | has("changes")] | any' "false"; then
        test_pass
    fi
else test_skip "no BUG1"; fi

test_begin "credentialless-bug-history-public-server" "credentialless bug history (public server)"
if [[ -n "$BUG1" ]]; then
    run_bzr_raw --json --server public bug history "$BUG1"
    if assert_success &&
        assert_json '[.[] | has("field")] | all' "true" &&
        assert_json '[.[] | has("changes")] | any' "false"; then
        test_pass
    fi
else test_skip "no BUG1"; fi

test_begin "bug-history-since" "bug history --since"
if [[ -n "$BUG1" ]]; then
    run_bzr bug history "$BUG1" --since 2020-01-01
    if assert_success; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "bug-list-changed-since-recent-activity" "bug list --changed-since (recent activity)"
if [[ -n "$BUG2" ]]; then
    run_bzr bug view "$BUG2"
    if assert_success; then
        SINCE_TS=$(jq -r '.last_change_time // empty' "$BZR_STDOUT" 2>/dev/null)
        if [[ $SINCE_TS =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$ ]]; then
            echo "    changed-since boundary: bug=$BUG2 timestamp=$SINCE_TS"
            run_bzr bug list --product FuncTestProd --changed-since "$SINCE_TS"
            if assert_success &&
                assert_json "[.[] | select(.id == $BUG2)] | length" "1"; then
                test_pass
            fi
        else
            test_fail "bug #$BUG2 returned invalid last_change_time '$SINCE_TS'"
        fi
    fi
else test_skip "no BUG2"; fi

test_begin "bug-list-changed-since-malformed-exit-7" "bug list --changed-since (malformed -> exit 7)"
run_bzr bug list --product FuncTestProd --changed-since "tomorrow"
if assert_exit_code 7; then test_pass; fi

test_begin "bug-list-whiteboard-substring-positive-includes-bug" "bug list --whiteboard (substring positive includes bug)"
if [[ -n "$BUG1" ]]; then
    # BUG1 had --whiteboard "wip" set in test 41.
    run_bzr bug list --id "$BUG1" --product FuncTestProd --whiteboard wip
    if assert_success && assert_stdout_contains "$BUG1"; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "bug-list-whiteboard-notsubstring-excludes-bug" "bug list --whiteboard (notsubstring excludes bug)"
if [[ -n "$BUG1" ]] && [[ -n "$BUG2" ]]; then
    # BUG1 has whiteboard "wip"; BUG2 does not. Negation must exclude BUG1
    # and include BUG2.
    run_bzr bug list --id "$BUG1" --id "$BUG2" --product FuncTestProd --whiteboard '!wip'
    if assert_success &&
        assert_stdout_not_contains "\"id\":$BUG1" &&
        assert_stdout_contains "$BUG2"; then
        test_pass
    fi
else test_skip "no BUG1/BUG2"; fi

test_begin "bug-list-resolution-fixed-positive" "bug list --resolution FIXED (positive)"
if [[ -n "$BUG1" ]]; then
    # BUG1 was resolved FIXED in test 43.
    run_bzr bug list --id "$BUG1" --product FuncTestProd --resolution FIXED
    if assert_success && assert_stdout_contains "$BUG1"; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "bug-list-resolution-fixed-notequals-excludes-resolved" "bug list --resolution '!FIXED' (notequals excludes resolved)"
if [[ -n "$BUG1" ]] && [[ -n "$BUG2" ]]; then
    # BUG1 is FIXED; BUG2 is open (empty resolution). The notequals filter
    # must exclude BUG1 and include BUG2 (empty resolution !=
    # "FIXED" by Bugzilla's notequals semantics).
    run_bzr bug list --id "$BUG1" --id "$BUG2" --product FuncTestProd --resolution '!FIXED'
    if assert_success &&
        assert_stdout_not_contains "\"id\":$BUG1" &&
        assert_stdout_contains "$BUG2"; then
        test_pass
    fi
else test_skip "no BUG1/BUG2"; fi

test_begin "bug-view-999999-negative-test" "bug view 999999 (negative test)"
run_bzr bug view 999999
if assert_failure; then test_pass; fi

test_begin "bug-view-multi-id-all-succeed-json-wrapped-shape" "bug view multi-ID (all succeed, JSON wrapped shape)"
if [[ -n "$BUG1" ]] && [[ -n "$BUG2" ]]; then
    run_bzr bug view "$BUG1" "$BUG2"
    if assert_success &&
        assert_json_array_length '.bugs' 2 &&
        assert_json_array_length '.failed' 0; then
        test_pass
    fi
else test_skip "no BUG1/BUG2"; fi

test_begin "bug-view-multi-id-strict-bails-on-inaccessible-bug" "bug view multi-ID strict bails on inaccessible bug"
if [[ -n "$BUG1" ]]; then
    run_bzr bug view 999999 "$BUG1"
    if assert_failure; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "bug-view-multi-id-permissive-surfaces-per-bug-error" "bug view multi-ID --permissive surfaces per-bug error"
if [[ -n "$BUG1" ]] && [[ -n "$BUG2" ]]; then
    run_bzr bug view "$BUG1" "$BUG2" 999999 --permissive
    if assert_success &&
        assert_json_array_length '.bugs' 2 &&
        assert_json_array_length '.failed' 1 &&
        assert_json '.failed[0].id' "999999"; then
        test_pass
    fi
else test_skip "no BUG1/BUG2"; fi

test_begin "bug-create-bug-three-clone-source" "bug create (bug three — clone source)"
run_bzr bug create --product FuncTestProd --component Backend --summary "Clone source bug" --description "Description for cloning" --priority Highest --severity critical --op-sys Linux --rep-platform PC
if assert_success && assert_json_exists '.id'; then
    # shellcheck disable=SC2034 # consumed by later sourced phases
    BUG3=$(jq -r '.id' "$BZR_STDOUT")
    test_pass
fi

test_begin "bug-create-bug-four-with-relationships" "bug create (bug four — with relationships)"
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
test_begin "bug-create-description-file" "bug create --description-file"
DESC_FILE=$(mktemp /tmp/bzr-func-desc.XXXXXX)
echo "description from file" >"$DESC_FILE"
run_bzr bug create --product FuncTestProd --component Backend \
    --summary "From file" --description-file "$DESC_FILE" \
    --op-sys All --rep-platform All
if assert_success; then test_pass; fi
rm -f "$DESC_FILE"

test_begin "bug-create-description-and-description-file-conflict-clap-exit-2" "bug create --description and --description-file conflict (clap exit 2)"
DESC_FILE=$(mktemp /tmp/bzr-func-desc.XXXXXX)
echo "should-not-appear" >"$DESC_FILE"
run_bzr_raw bug create --product FuncTestProd --component Backend \
    --summary "Conflict test" --description literal \
    --description-file "$DESC_FILE" --op-sys All --rep-platform All
if assert_exit_code 2; then test_pass; fi
rm -f "$DESC_FILE"

test_begin "bug-create-stdin-description-piped" "bug create stdin description (piped)"
run_bzr bug create \
    --product FuncTestProd --component Backend \
    --summary "From stdin" --op-sys All --rep-platform All \
    <<<"description from stdin"
if assert_success; then test_pass; fi

test_begin "bug-create-description-file-wins-over-piped-stdin" "bug create --description-file wins over piped stdin"
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

test_begin "bug-create-description-file-missing-path-exit-7" "bug create --description-file missing path → exit 7"
run_bzr_raw bug create --product FuncTestProd --component Backend \
    --summary "Missing file" --description-file /nonexistent-bzr-path-xyz-123 \
    --op-sys All --rep-platform All
if assert_exit_code 7; then test_pass; fi

test_begin "bug-create-empty-piped-stdin-without-explicit-description-exit-7" "bug create empty piped stdin without explicit description → exit 7"
run_bzr_raw bug create \
    --product FuncTestProd --component Backend \
    --summary "Empty stdin" --op-sys All --rep-platform All \
    </dev/null
if assert_exit_code 7; then test_pass; fi

test_begin "bug-create-empty-fake-editor-exit-7-tty-conditional" "bug create empty fake-editor → exit 7 (TTY-conditional)"
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
