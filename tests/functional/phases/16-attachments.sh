# 16-attachments
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

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
    run_bzr attachment update "$ATTACH_ID" --summary "Updated summary" --obsolete
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

test_begin "100g. attachment upload --patch marks attachment as a patch"
if [[ -n "$BUG1" ]]; then
    run_bzr attachment upload "$BUG1" /tmp/bzr-func-test.txt \
        --summary "patch test" --patch
    if assert_success; then
        run_bzr attachment list "$BUG1"
        if assert_success \
            && assert_json '[.[] | select(.summary == "patch test")][-1].is_patch' "true"; then
            test_pass
        fi
    fi
else test_skip "no BUG1"; fi

test_begin "100h. attachment upload --comment-private flips comment privacy"
if [[ -n "$BUG1" ]]; then
    run_bzr attachment upload "$BUG1" /tmp/bzr-func-test.txt \
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

test_begin "100i. attachment download --bug bulk into per-bug subdir"
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

test_begin "100j. attachment download mixes --bug and positional IDs"
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

test_begin "150. attachment view metadata"
run_bzr attachment upload "$_AB" "$_AF" --summary "viewme"
if assert_success; then
    _AID=$(jq -r '.id // .attachment_id // (.ids[0] // empty)' "$BZR_STDOUT" 2>/dev/null)
    run_bzr attachment view "$_AID"
    if assert_success && assert_json '.summary' "viewme"; then test_pass; fi
fi

test_begin "151. attachment update --file-name round-trips"
if [[ -n "${_AID:-}" ]]; then
    run_bzr attachment update "$_AID" --file-name "renamed-att.bin"
    if assert_success; then
        run_bzr attachment view "$_AID"
        if assert_json '.file_name' "renamed-att.bin"; then test_pass; fi
    fi
else test_skip "no attachment id"; fi

rm -f "$_AF"
unset _AB _AF _AID
echo ""

