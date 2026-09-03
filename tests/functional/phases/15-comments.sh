# 15-comments
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 14: Comments
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 14: Comments ───────────────────────────────────────"

test_begin "comment-add-first" "comment add (first)"
if [[ -n "$BUG1" ]]; then
    run_bzr comment add "$BUG1" --body "First test comment"
    if assert_success && assert_json_exists '.id'; then
        COMMENT_ID=$(jq -r '.id' "$BZR_STDOUT")
        test_pass
    fi
else test_skip "no BUG1"; fi

test_begin "comment-add-second" "comment add (second)"
if [[ -n "$BUG1" ]]; then
    run_bzr comment add "$BUG1" --body "Second comment"
    if assert_success; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "comment-list" "comment list"
if [[ -n "$BUG1" ]]; then
    run_bzr comment list "$BUG1"
    # Bug description counts as comment 0, plus our 2 = at least 3
    if assert_success && assert_json_array_min_length '.' 3; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "comment-list-since" "comment list --since"
if [[ -n "$BUG1" ]]; then
    run_bzr comment list "$BUG1" --since 2020-01-01
    if assert_success; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "comment-list-fields-projects-keys" "comment list --fields projects keys"
if [[ -n "$BUG1" ]]; then
    run_bzr comment list "$BUG1" --fields id,creator
    if assert_success && assert_json '.[0] | keys | length' 2 &&
        assert_json_exists '.[0].id'; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "comment-list-fields-unknown-exits-7" "comment list --fields unknown exits 7"
if [[ -n "$BUG1" ]]; then
    run_bzr comment list "$BUG1" --fields bogus_xyz
    if assert_exit_code 7; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "comment-tag-add" "comment tag --add"
if [[ -n "${COMMENT_ID:-}" ]] && [[ "$COMMENT_ID" != "null" ]]; then
    run_bzr comment tag "$COMMENT_ID" --add important
    if assert_success; then test_pass; fi
else
    test_skip "no comment ID"
fi

test_begin "comment-tag-remove" "comment tag --remove"
if [[ -n "${COMMENT_ID:-}" ]] && [[ "$COMMENT_ID" != "null" ]]; then
    run_bzr comment tag "$COMMENT_ID" --remove important
    if assert_success; then test_pass; fi
else
    test_skip "no comment ID"
fi

test_begin "comment-search-tags" "comment search-tags"
run_bzr comment search-tags important
# May return empty if tag was fully removed, but should succeed
if assert_success; then test_pass; fi

# ─ Issue #161: bug update --comment / --comment-file / --comment-private ─

test_begin "bug-update-comment-posts-atomically" "bug update --comment posts atomically"
if [[ -n "$BUG1" ]]; then
    # Capture pre-update comment count.
    run_bzr comment list "$BUG1"
    pre_count=$(jq '. | length' "$BZR_STDOUT")
    run_bzr bug update "$BUG1" --whiteboard "atomic-comment-test" \
        --comment "atomic comment from #161 test"
    if assert_success; then
        run_bzr comment list "$BUG1"
        post_count=$(jq '. | length' "$BZR_STDOUT")
        if [[ "$post_count" -eq $((pre_count + 1)) ]] &&
            jq -e '.[-1].text == "atomic comment from #161 test"' "$BZR_STDOUT" >/dev/null; then
            test_pass
        else
            test_fail "comment not appended atomically (pre=$pre_count post=$post_count)"
        fi
    fi
else test_skip "no BUG1"; fi

test_begin "bug-update-comment-comment-private" "bug update --comment --comment-private"
if [[ -n "$BUG1" ]]; then
    run_bzr bug update "$BUG1" --comment "private atomic comment" --comment-private
    if assert_success; then
        run_bzr --api hybrid comment list "$BUG1"
        if jq -e '.[-1].is_private == true' "$BZR_STDOUT" >/dev/null &&
            jq -e '.[-1].text == "private atomic comment"' "$BZR_STDOUT" >/dev/null; then
            test_pass
        else
            test_fail "last comment not private or text mismatch"
        fi
    fi
else test_skip "no BUG1"; fi

test_begin "bug-update-comment-file" "bug update --comment-file"
if [[ -n "$BUG1" ]]; then
    tmpfile=$(mktemp)
    printf 'comment from file\nsecond line\n' >"$tmpfile"
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

# comment add --body-file (and stdin via `-`). Self-contained fixtures; stdin is
# fed by redirection, not a pipe, so run_bzr's exit capture stays in this shell.
_CB=$(make_bug --product FuncTestProd --component Backend --op-sys Linux --platform PC --description d --summary "comment bodyfile host")
_CBF=$(mktemp /tmp/bzr-func-cbody.XXXXXX)

test_begin "comment-add-body-file" "comment add --body-file"
printf 'body from a file' >"$_CBF"
run_bzr comment add "$_CB" --body-file "$_CBF"
if assert_success; then
    run_bzr comment list "$_CB"
    if assert_stdout_contains "body from a file"; then test_pass; fi
fi

test_begin "comment-add-body-file-stdin" "comment add --body-file - (stdin)"
printf 'body via stdin dash' >"$_CBF"
run_bzr comment add "$_CB" --body-file - <"$_CBF"
if assert_success; then
    run_bzr comment list "$_CB"
    if assert_stdout_contains "body via stdin dash"; then test_pass; fi
fi

rm -f "$_CBF"
unset _CB _CBF

# Exercise the integer privacy response seen on production Bugzilla servers
# through the public REST command path. The proxy mode is opt-in so the default
# functional consumers continue to see the container's native response.
_AC_COMMENT_BUG=$(make_bug --product FuncTestProd --component Backend \
    --op-sys Linux --platform PC --description d \
    --summary "attachment-comment proxy comment host")
run_bzr comment add "$_AC_COMMENT_BUG" --body "proxy private comment" --private

test_begin "production-shaped-comment-privacy" "production-shaped integer comment privacy"
export BZR_FUNC_REDHAT_MODE=attachment-comment
if redhat_shape_start "$BZ_PORT"; then
    unset BZR_FUNC_REDHAT_MODE
    trap 'cleanup; redhat_shape_stop' EXIT
    export BZR_FUNC_INLINE_KEY="$API_KEY"
    run_bzr --api rest \
        --server-url "http://127.0.0.1:${REDHAT_SHAPE_PORT}" \
        --server-api-key-env BZR_FUNC_INLINE_KEY --server-email "$ADMIN_EMAIL" \
        comment list "$_AC_COMMENT_BUG"
    _AC_COMMENT_OK=1
    if ! assert_success ||
        ! jq -e 'any(.[]; .text == "proxy private comment" and .is_private == true)' \
            "$BZR_STDOUT" >/dev/null ||
        ! grep -Fq "attachment-comment shaped route=comment-privacy count=" \
            "$REDHAT_SHAPE_LOG"; then
        _AC_COMMENT_OK=0
    fi
    redhat_shape_stop || _AC_COMMENT_OK=0
    trap cleanup EXIT
    unset BZR_FUNC_INLINE_KEY
    if [[ $_AC_COMMENT_OK -eq 1 ]]; then
        test_pass
    else
        test_fail "production-shaped comment proof failed; proxy log: $REDHAT_SHAPE_LOG"
    fi
else
    unset BZR_FUNC_REDHAT_MODE
    test_fail "attachment-comment response-shape proxy did not become ready: $REDHAT_SHAPE_LOG"
fi
unset _AC_COMMENT_BUG _AC_COMMENT_OK
echo ""
