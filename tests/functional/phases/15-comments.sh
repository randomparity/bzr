# 15-comments
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

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

# ─ Issue #161: bug update --comment / --comment-file / --comment-private ─

test_begin "94d. bug update --comment posts atomically"
if [[ -n "$BUG1" ]]; then
    # Capture pre-update comment count.
    run_bzr comment list "$BUG1"
    pre_count=$(jq '. | length' "$BZR_STDOUT")
    run_bzr bug update "$BUG1" --whiteboard "atomic-comment-test" \
        --comment "atomic comment from #161 test"
    if assert_success; then
        run_bzr comment list "$BUG1"
        post_count=$(jq '. | length' "$BZR_STDOUT")
        if [[ "$post_count" -eq $((pre_count + 1)) ]] \
            && jq -e '.[-1].text == "atomic comment from #161 test"' "$BZR_STDOUT" >/dev/null; then
            test_pass
        else
            test_fail "comment not appended atomically (pre=$pre_count post=$post_count)"
        fi
    fi
else test_skip "no BUG1"; fi

test_begin "94e. bug update --comment --comment-private"
if [[ -n "$BUG1" ]]; then
    run_bzr bug update "$BUG1" --comment "private atomic comment" --comment-private
    if assert_success; then
        run_bzr --api hybrid comment list "$BUG1"
        if jq -e '.[-1].is_private == true' "$BZR_STDOUT" >/dev/null \
            && jq -e '.[-1].text == "private atomic comment"' "$BZR_STDOUT" >/dev/null; then
            test_pass
        else
            test_fail "last comment not private or text mismatch"
        fi
    fi
else test_skip "no BUG1"; fi

test_begin "94f. bug update --comment-file"
if [[ -n "$BUG1" ]]; then
    tmpfile=$(mktemp)
    printf 'comment from file\nsecond line\n' > "$tmpfile"
    run_bzr bug update "$BUG1" --comment-file "$tmpfile"
    if assert_success; then
        run_bzr comment list "$BUG1"
        if jq -e '.[-1].text | contains("comment from file")' "$BZR_STDOUT" >/dev/null; then
            test_pass
        else
            test_fail "file comment not posted"
        fi
    fi
    rm -f "$tmpfile"
else test_skip "no BUG1"; fi

echo ""

