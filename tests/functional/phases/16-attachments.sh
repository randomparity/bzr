# 16-attachments
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 15: Attachments
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 15: Attachments ───────────────────────────────────"

test_begin "create-temp-file" "create temp file"
echo "bzr functional test content $(date +%s)" >"$FUNC_ATTACH_FILE"
test_pass

test_begin "attachment-upload" "attachment upload"
if [[ -n "$BUG1" ]]; then
    run_bzr attachment upload "$BUG1" "$FUNC_ATTACH_FILE" --summary "Test file"
    if assert_success && assert_json_exists '.id'; then
        ATTACH_ID=$(jq -r '.id' "$BZR_STDOUT" 2>/dev/null || jq -r '.ids[0]' "$BZR_STDOUT" 2>/dev/null || echo "")
        test_pass
    fi
else test_skip "no BUG1"; fi

test_begin "attachment-list" "attachment list"
if [[ -n "$BUG1" ]]; then
    run_bzr attachment list "$BUG1"
    if assert_success && assert_json_array_min_length '.' 1; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "attachment-list-fields-projects-keys" "attachment list --fields projects keys"
if [[ -n "$BUG1" ]]; then
    run_bzr attachment list "$BUG1" --fields file_name,size
    if assert_success && assert_json '.[0] | keys | length' 2 &&
        assert_json_exists '.[0].file_name'; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "attachment-list-fields-unknown-exits-7" "attachment list --fields unknown exits 7"
if [[ -n "$BUG1" ]]; then
    run_bzr attachment list "$BUG1" --fields bogus_xyz
    if assert_exit_code 7; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "attachment-download" "attachment download"
if [[ -n "${ATTACH_ID:-}" ]] && [[ "$ATTACH_ID" != "null" ]]; then
    rm -f "$FUNC_DOWNLOAD_FILE"
    run_bzr attachment download "$ATTACH_ID" --out "$FUNC_DOWNLOAD_FILE"
    if assert_success && assert_file_contains "$FUNC_DOWNLOAD_FILE" "bzr functional test content"; then
        test_pass
    fi
else
    test_skip "no attachment ID"
fi

test_begin "attachment-download-out-streams-raw-bytes" "attachment download --out - streams raw bytes"
if [[ -n "${ATTACH_ID:-}" ]] && [[ "$ATTACH_ID" != "null" ]]; then
    run_bzr_raw attachment download "$ATTACH_ID" --out -
    if assert_success && assert_stdout_equals_file "$FUNC_ATTACH_FILE"; then test_pass; fi
else
    test_skip "no attachment ID"
fi

test_begin "attachment-update" "attachment update"
if [[ -n "${ATTACH_ID:-}" ]] && [[ "$ATTACH_ID" != "null" ]]; then
    run_bzr attachment update "$ATTACH_ID" --summary "Updated summary" --obsolete
    if assert_success; then test_pass; fi
else
    test_skip "no attachment ID"
fi

test_begin "attachment-upload-explicit-mime" "attachment upload (explicit MIME)"
if [[ -n "$BUG1" ]]; then
    run_bzr attachment upload "$BUG1" "$FUNC_ATTACH_FILE" --content-type text/plain
    if assert_success; then test_pass; fi
else test_skip "no BUG1"; fi

test_begin "attachment-upload-comment-posts-comment-in-same-call" "attachment upload --comment posts comment in same call"
if [[ -n "$BUG1" ]]; then
    run_bzr comment list "$BUG1"
    if assert_success; then
        PRECOMMENT_COUNT=$(jq '. | length' "$BZR_STDOUT" 2>/dev/null || echo "")
        run_bzr attachment upload "$BUG1" "$FUNC_ATTACH_FILE" \
            --summary "with comment" \
            --comment "see #165 -- bzl-parity"
        if assert_success; then
            run_bzr comment list "$BUG1"
            if assert_success; then
                POSTCOMMENT_COUNT=$(jq '. | length' "$BZR_STDOUT" 2>/dev/null || echo "")
                if [[ -n "$PRECOMMENT_COUNT" ]] &&
                    [[ -n "$POSTCOMMENT_COUNT" ]] &&
                    [[ "$POSTCOMMENT_COUNT" -eq $((PRECOMMENT_COUNT + 1)) ]]; then
                    test_pass
                else
                    test_fail "comment count did not grow by 1 (pre=$PRECOMMENT_COUNT post=$POSTCOMMENT_COUNT)"
                fi
            fi
        fi
    fi
else test_skip "no BUG1"; fi

test_begin "attachment-upload-patch-marks-attachment-as-a-patch" "attachment upload --patch marks attachment as a patch"
if [[ -n "$BUG1" ]]; then
    run_bzr attachment upload "$BUG1" "$FUNC_ATTACH_FILE" \
        --summary "patch test" --patch
    if assert_success; then
        run_bzr attachment list "$BUG1"
        if assert_success &&
            assert_json '[.[] | select(.summary == "patch test")][-1].is_patch' "true"; then
            test_pass
        fi
    fi
else test_skip "no BUG1"; fi

test_begin "attachment-upload-comment-private-flips-comment-privacy" "attachment upload --comment-private flips comment privacy"
if [[ -n "$BUG1" ]]; then
    run_bzr attachment upload "$BUG1" "$FUNC_ATTACH_FILE" \
        --summary "private comment test" \
        --comment "sensitive context for #170" \
        --comment-private
    if assert_success && assert_json_exists '.id'; then
        ATTACH_PRIV_ID=$(jq -r '.id' "$BZR_STDOUT" 2>/dev/null || echo "")
        run_bzr comment list "$BUG1"
        if assert_success; then
            # The new comment is the one whose attachment_id matches
            # the just-uploaded attachment. Assert it's marked private.
            MATCHED=$(jq --arg a "$ATTACH_PRIV_ID" \
                '[.[] | select(.attachment_id == ($a | tonumber))] | last' \
                "$BZR_STDOUT" 2>/dev/null || echo "")
            IS_PRIVATE=$(echo "$MATCHED" | jq -r '.is_private' 2>/dev/null || echo "")
            if [[ "$IS_PRIVATE" == "true" ]]; then
                test_pass
            else
                test_fail "comment matching attachment #${ATTACH_PRIV_ID} not marked private (is_private=${IS_PRIVATE})"
            fi
        fi
    fi
else test_skip "no BUG1"; fi

test_begin "attachment-upload-comment-file" "attachment upload --comment-file"
if [[ -n "$BUG1" ]]; then
    _ACF=$(mktemp /tmp/bzr-func-attachment-comment.XXXXXX)
    printf 'attachment comment from file' >"$_ACF"
    run_bzr attachment upload "$BUG1" "$FUNC_ATTACH_FILE" \
        --summary "comment file upload" --comment-file "$_ACF"
    if assert_success; then
        run_bzr comment list "$BUG1"
        if assert_stdout_contains "attachment comment from file"; then test_pass; fi
    fi
    rm -f "$_ACF"
else test_skip "no BUG1"; fi

test_begin "attachment-upload-comment-file-stdin" "attachment upload --comment-file -"
if [[ -n "$BUG1" ]]; then
    _ACF=$(mktemp /tmp/bzr-func-attachment-comment.XXXXXX)
    printf 'attachment comment from stdin' >"$_ACF"
    run_bzr attachment upload "$BUG1" "$FUNC_ATTACH_FILE" \
        --summary "comment stdin upload" --comment-file - <"$_ACF"
    if assert_success; then
        run_bzr comment list "$BUG1"
        if assert_stdout_contains "attachment comment from stdin"; then test_pass; fi
    fi
    rm -f "$_ACF"
else test_skip "no BUG1"; fi

test_begin "attachment-upload-empty-comment-file-rejected" "attachment upload empty --comment-file rejected"
if [[ -n "$BUG1" ]]; then
    _ACF=$(mktemp /tmp/bzr-func-attachment-comment.XXXXXX)
    printf '   ' >"$_ACF"
    run_bzr attachment upload "$BUG1" "$FUNC_ATTACH_FILE" \
        --summary "empty comment upload" --comment-file "$_ACF"
    if assert_exit_code 7 && assert_stderr_contains "empty comment, aborting"; then test_pass; fi
    rm -f "$_ACF"
else test_skip "no BUG1"; fi
unset _ACF

test_begin "attachment-download-bug-bulk-into-per-bug-subdir" "attachment download --bug bulk into per-bug subdir"
if [[ -n "$BUG1" ]]; then
    BULK_DIR="$(mktemp -d /tmp/bzr-func-bulk.XXXXXX)"
    run_bzr attachment download --bug "$BUG1" --out-dir "$BULK_DIR"
    if assert_success; then
        # Per-bug subdir must exist
        if [[ -d "$BULK_DIR/$BUG1" ]]; then
            # Bug had at least 5 attachments uploaded earlier in Phase 15;
            # require ≥2 to be safe against per-deployment fixture drift.
            NUM_FILES=$(find "$BULK_DIR/$BUG1" -type f | wc -l | tr -d ' ')
            if [[ "$NUM_FILES" -ge 2 ]]; then
                test_pass
            else
                test_fail "expected ≥2 files in $BULK_DIR/$BUG1, found $NUM_FILES"
            fi
        else
            test_fail "expected per-bug subdir $BULK_DIR/$BUG1 not created"
        fi
    fi
    rm -rf "$BULK_DIR"
else test_skip "no BUG1"; fi

test_begin "attachment-download-mixes-bug-and-positional-ids" "attachment download mixes --bug and positional IDs"
if [[ -n "$BUG1" ]] && [[ -n "${ATTACH_ID:-}" ]] && [[ "$ATTACH_ID" != "null" ]]; then
    MIX_DIR="$(mktemp -d /tmp/bzr-func-mix.XXXXXX)"
    # Use BUG1 (multi-attachment) AND a specific ATTACH_ID (which also
    # belongs to BUG1, so positional + --bug both land in the same
    # per-bug subdir). The test verifies the dispatch handles both
    # input shapes; the disk layout is the same as the per-bug case.
    run_bzr attachment download --bug "$BUG1" "$ATTACH_ID" --out-dir "$MIX_DIR"
    if assert_success; then
        if [[ -d "$MIX_DIR/$BUG1" ]]; then
            # The positional ATTACH_ID's file must exist with the att-id
            # prefix even though it would have been included by --bug too.
            # The per-bug walk runs first, then positional — silent
            # overwrite means the second write wins, but the file must
            # exist either way.
            POS_FILE_COUNT=$(find "$MIX_DIR/$BUG1" -name "${ATTACH_ID}.*" -type f | wc -l | tr -d ' ')
            if [[ "$POS_FILE_COUNT" -ge 1 ]]; then
                test_pass
            else
                test_fail "expected file ${ATTACH_ID}.* in $MIX_DIR/$BUG1, found $POS_FILE_COUNT"
            fi
        else
            test_fail "expected per-bug subdir $MIX_DIR/$BUG1 not created"
        fi
    fi
    rm -rf "$MIX_DIR"
else test_skip "no BUG1 or no ATTACH_ID"; fi

# attachment view (metadata only) and attachment update --file-name. Self-
# contained: creates its own bug and attachment.
_AB=$(make_bug --product FuncTestProd --component Backend --op-sys Linux --rep-platform PC --description d --summary "att view host")
_AF=$(mktemp /tmp/bzr-func-att.XXXXXX)
printf 'attachment bytes' >"$_AF"

test_begin "attachment-view-metadata" "attachment view metadata"
run_bzr attachment upload "$_AB" "$_AF" --summary "viewme"
if assert_success; then
    _AID=$(jq -r '.id // .attachment_id // (.ids[0] // empty)' "$BZR_STDOUT" 2>/dev/null)
    run_bzr attachment view "$_AID"
    if assert_success && assert_json '.summary' "viewme"; then test_pass; fi
fi

test_begin "attachment-update-file-name-round-trips" "attachment update --file-name round-trips"
if [[ -n "${_AID:-}" ]]; then
    run_bzr attachment update "$_AID" --file-name "renamed-att.bin"
    if assert_success; then
        run_bzr attachment view "$_AID"
        if assert_json '.file_name' "renamed-att.bin"; then test_pass; fi
    fi
else test_skip "no attachment id"; fi

test_begin "attachment-update-content-type-and-flag" "attachment update --content-type and --flag"
if [[ -n "${_AID:-}" ]]; then
    run_bzr attachment update "$_AID" --content-type text/plain --flag 'bzr_attachment_review?'
    if assert_success; then
        run_bzr attachment view "$_AID"
        if assert_json '.content_type' "text/plain" &&
            assert_json_contains '[.flags[].name] | join(",")' "bzr_attachment_review"; then test_pass; fi
    fi
else test_skip "no attachment id"; fi

_assert_transport_timestamp_parity() {
    local resource="$1"
    local filter="$2"
    shift 2
    local rest="$FUNC_CONFIG_DIR/xmlrpc-parity-${resource}-rest.json"
    local xmlrpc="$FUNC_CONFIG_DIR/xmlrpc-parity-${resource}-xmlrpc.json"

    run_bzr --api rest "$@"
    if ! assert_success; then return; fi
    if ! jq -cS "$filter" "$BZR_STDOUT" >"$rest"; then
        test_fail "could not capture REST $resource timestamps"
        return
    fi

    run_bzr --api xmlrpc "$@"
    if ! assert_success; then return; fi
    if ! jq -cS "$filter" "$BZR_STDOUT" >"$xmlrpc"; then
        test_fail "could not capture XML-RPC $resource timestamps"
        return
    fi

    if cmp -s "$rest" "$xmlrpc"; then
        test_pass
    else
        test_fail "REST and XML-RPC $resource timestamps differ"
    fi
}

test_begin "bug-view-timestamps-match-rest-and-xmlrpc-on-bz50" "bug view timestamps match across bz50 transports"
if [[ "$BZ_VERSION" == "bz50" ]] && [[ -n "${_AB:-}" ]]; then
    _assert_transport_timestamp_parity bug \
        '{creation_time, last_change_time}' bug view "$_AB"
else test_skip "XML-RPC transport parity applies to bz50"; fi

test_begin "comment-list-timestamps-match-rest-and-xmlrpc-on-bz50" "comment list timestamps match across bz50 transports"
if [[ "$BZ_VERSION" == "bz50" ]] && [[ -n "${_AB:-}" ]]; then
    _assert_transport_timestamp_parity comment \
        '[.[] | {id, creation_time}] | sort_by(.id)' comment list "$_AB"
else test_skip "XML-RPC transport parity applies to bz50"; fi

test_begin "attachment-list-timestamps-match-rest-and-xmlrpc-on-bz50" "attachment list timestamps match across bz50 transports"
if [[ "$BZ_VERSION" == "bz50" ]] && [[ -n "${_AB:-}" ]]; then
    _assert_transport_timestamp_parity attachment \
        '[.[] | {id, creation_time, last_change_time}] | sort_by(.id)' \
        attachment list "$_AB"
else test_skip "XML-RPC transport parity applies to bz50"; fi

test_begin "attachment-list-and-view-flags-match-on-bz50" "XML-RPC attachment list and view flags match on bz50"
if [[ "$BZ_VERSION" == "bz50" ]] && [[ -n "${_AB:-}" ]] && [[ -n "${_AID:-}" ]]; then
    _XP_LIST="$FUNC_CONFIG_DIR/xmlrpc-parity-flags-list.json"
    _XP_VIEW="$FUNC_CONFIG_DIR/xmlrpc-parity-flags-view.json"
    run_bzr --api xmlrpc attachment list "$_AB"
    if assert_success; then
        if ! jq -ceS --argjson id "$_AID" \
            '[.[] | select(.id == $id)][0].flags | sort_by(.name, .status, .setter, .requestee)' \
            "$BZR_STDOUT" >"$_XP_LIST" ||
            ! jq -e 'length > 0 and any(.[]; .name == "bzr_attachment_review")' \
                "$_XP_LIST" >/dev/null; then
            test_fail "XML-RPC attachment list did not return the review flag"
        else
            run_bzr --api xmlrpc attachment view "$_AID"
            if assert_success; then
                if ! jq -ceS '.flags | sort_by(.name, .status, .setter, .requestee)' \
                    "$BZR_STDOUT" >"$_XP_VIEW" ||
                    ! jq -e 'length > 0 and any(.[]; .name == "bzr_attachment_review")' \
                        "$_XP_VIEW" >/dev/null; then
                    test_fail "XML-RPC attachment view did not return the review flag"
                elif cmp -s "$_XP_LIST" "$_XP_VIEW"; then
                    test_pass
                else
                    test_fail "XML-RPC attachment list and view flags differ"
                fi
            fi
        fi
    fi
else test_skip "XML-RPC attachment flag parity applies to bz50"; fi

rm -f "$_AF"
unset -f _assert_transport_timestamp_parity
unset _AB _AF _AID _XP_LIST _XP_VIEW
echo ""
