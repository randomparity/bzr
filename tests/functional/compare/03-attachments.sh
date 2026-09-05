#!/bin/bash
# Attachment persisted-state, content, flag, privacy, and known-gap comparisons.

printf -v ATTACHMENT_RUN_TOKEN '%x-%x-%x' "$$" "$RANDOM" "$RANDOM"
ATTACHMENT_RUN_TOKEN="${ATTACHMENT_RUN_TOKEN:0:18}"
ATTACHMENT_STEM="bzr-pybz-attachments-${BZ_VERSION}-${ATTACHMENT_RUN_TOKEN}"
ATTACHMENT_SOURCE="$COMPARE_EXCHANGE_DIR/attachment-source.txt"
ATTACHMENT_CREATED_BUG_ID=""
ATTACHMENT_BZR_BUG_ID=""
ATTACHMENT_PYBZ_BUG_ID=""
ATTACHMENT_BZR_ID=""
ATTACHMENT_PYBZ_ID=""
printf 'resource comparison attachment bytes %s\n' "$ATTACHMENT_RUN_TOKEN" >"$ATTACHMENT_SOURCE"
chmod 600 "$ATTACHMENT_SOURCE"

attachment_create_bug() {
    local name="$1" summary="$2" id

    ATTACHMENT_CREATED_BUG_ID=""
    if ! resource_bzr "$name" rest REST bug create \
        --product TestProduct --component TestComponent --summary "$summary" \
        --description "attachment comparison fixture" --op-sys Linux --platform PC \
        --priority Normal --severity normal; then
        return 1
    fi
    if ! id=$(resource_positive_id \
        "$COMPARE_EXCHANGE_DIR/${name}.bzr.stdout.json" '.id'); then
        test_fail "could not create attachment comparison bug"
        return 1
    fi
    ATTACHMENT_CREATED_BUG_ID="$id"
}

attachment_upload_pair() {
    local slug="$1" private="$2" pybz_transport="$3"
    local summary="$ATTACHMENT_STEM $slug" comment="$ATTACHMENT_STEM $slug comment"

    if ! attachment_create_bug "$slug-bzr-create" "$summary [bzr]"; then
        return 1
    fi
    ATTACHMENT_BZR_BUG_ID="$ATTACHMENT_CREATED_BUG_ID"
    if ! attachment_create_bug "$slug-pybz-create" "$summary [pybz]"; then
        return 1
    fi
    ATTACHMENT_PYBZ_BUG_ID="$ATTACHMENT_CREATED_BUG_ID"
    if [[ $private == true ]]; then
        resource_bzr "$slug-bzr-upload" rest REST attachment upload \
            "$ATTACHMENT_BZR_BUG_ID" "$ATTACHMENT_SOURCE" --summary "$summary" \
            --content-type text/plain --comment "$comment" --private || return 1
    else
        resource_bzr "$slug-bzr-upload" rest REST attachment upload \
            "$ATTACHMENT_BZR_BUG_ID" "$ATTACHMENT_SOURCE" --summary "$summary" \
            --content-type text/plain --comment "$comment" || return 1
    fi
    ATTACHMENT_BZR_ID=$(resource_positive_id \
        "$COMPARE_EXCHANGE_DIR/${slug}-bzr-upload.bzr.stdout.json" '.id') || {
        test_fail "bzr returned an invalid attachment ID"
        return 1
    }
    resource_pybz "$slug-pybz-upload" attachment_upload \
        "$(jq -cn --arg transport "$pybz_transport" \
            --argjson bug_id "$ATTACHMENT_PYBZ_BUG_ID" \
            --arg source '/work/compare/attachment-source.txt' --arg summary "$summary" \
            --arg comment "$comment" --argjson private "$private" \
            '{transport:$transport,bug_ids:[$bug_id],source:$source,summary:$summary,
              file_name:"attachment-source.txt",content_type:"text/plain",comment:$comment,
              is_private:$private}')" "$pybz_transport" || return 1
    ATTACHMENT_PYBZ_ID=$(resource_positive_id \
        "$COMPARE_EXCHANGE_DIR/${slug}-pybz-upload.pybz.result.json" \
        '.attachment_ids[0]') || {
        test_fail "python-bugzilla returned an invalid attachment ID"
        return 1
    }
}

attachment_normalize_lists() {
    local slug="$1" summary="$2" private="$3"
    local bzr_list="$COMPARE_EXCHANGE_DIR/${slug}.bzr.metadata.json"
    local pybz_list="$COMPARE_EXCHANGE_DIR/${slug}.pybz.metadata.json"

    jq --argjson id "$ATTACHMENT_BZR_ID" --arg summary "$summary" \
        '[.[] | select(.id == $id and .summary == $summary) |
          {file_name,summary,content_type,
           is_private:(.is_private == true or .is_private == 1),
           is_obsolete:(.is_obsolete == true or .is_obsolete == 1)}]' \
        "$COMPARE_EXCHANGE_DIR/${slug}-bzr-list.bzr.stdout.json" >"$bzr_list"
    jq --argjson bug_id "$ATTACHMENT_PYBZ_BUG_ID" \
        --argjson id "$ATTACHMENT_PYBZ_ID" --arg summary "$summary" \
        '[.bugs[($bug_id | tostring)][] |
          select((.id | tonumber) == $id and .summary == $summary) |
          {file_name,summary,content_type,
           is_private:(.is_private == true or .is_private == 1),
           is_obsolete:(.is_obsolete == true or .is_obsolete == 1)}]' \
        "$COMPARE_EXCHANGE_DIR/${slug}-pybz-list.pybz.result.json" >"$pybz_list"
    if ! jq -e --argjson private "$private" \
        'length == 1 and .[0].file_name == "attachment-source.txt" and
         .[0].content_type == "text/plain" and .[0].is_private == $private and
         .[0].is_obsolete == false' "$bzr_list" >/dev/null ||
        ! jq -e --argjson private "$private" \
            'length == 1 and .[0].file_name == "attachment-source.txt" and
             .[0].content_type == "text/plain" and .[0].is_private == $private and
             .[0].is_obsolete == false' "$pybz_list" >/dev/null; then
        test_fail "attachment metadata or privacy positive control is missing"
        return 1
    fi
    resource_equal "$slug-metadata" "$bzr_list" "$pybz_list"
}

attachment_digest() {
    shasum -a 256 "$1" | awk '{print $1}'
}

test_begin "upload-metadata-comment" "attachment upload metadata and linked comment"
_ATTACH_PUBLIC_SUMMARY="$ATTACHMENT_STEM public"
_ATTACH_PUBLIC_COMMENT="$ATTACHMENT_STEM public comment"
if attachment_upload_pair public false REST &&
    resource_bzr public-bzr-list rest REST attachment list "$ATTACHMENT_BZR_BUG_ID" &&
    resource_pybz public-pybz-list attachment_list \
        "$(jq -cn --argjson id "$ATTACHMENT_PYBZ_BUG_ID" \
            '{transport:"REST",bug_ids:[$id]}')" REST &&
    attachment_normalize_lists public "$_ATTACH_PUBLIC_SUMMARY" false &&
    resource_bzr public-bzr-comments rest REST comment list "$ATTACHMENT_BZR_BUG_ID" &&
    resource_pybz public-pybz-comments comment_list \
        "$(jq -cn --argjson id "$ATTACHMENT_PYBZ_BUG_ID" \
            '{transport:"REST",bug_id:$id}')" REST; then
    jq --arg text "$_ATTACH_PUBLIC_COMMENT" \
        '[.[] | select(.text | endswith($text)) | {text:$text}]' \
        "$COMPARE_EXCHANGE_DIR/public-bzr-comments.bzr.stdout.json" \
        >"$COMPARE_EXCHANGE_DIR/public.bzr.comment.json"
    jq --argjson id "$ATTACHMENT_PYBZ_BUG_ID" --arg text "$_ATTACH_PUBLIC_COMMENT" \
        '[.bugs[($id | tostring)].comments[] |
          select(.text | endswith($text)) | {text:$text}]' \
        "$COMPARE_EXCHANGE_DIR/public-pybz-comments.pybz.result.json" \
        >"$COMPARE_EXCHANGE_DIR/public.pybz.comment.json"
    if jq -e 'length == 1' "$COMPARE_EXCHANGE_DIR/public.bzr.comment.json" >/dev/null &&
        jq -e 'length == 1' "$COMPARE_EXCHANGE_DIR/public.pybz.comment.json" >/dev/null &&
        resource_equal public-comment "$COMPARE_EXCHANGE_DIR/public.bzr.comment.json" \
            "$COMPARE_EXCHANGE_DIR/public.pybz.comment.json"; then
        test_pass
    elif [[ $TEST_RESULT_PENDING -eq 0 ]]; then
        test_fail "attachment comment persisted outcome differs"
    fi
fi

test_begin "download-content" "single and per-bug attachment download content"
_ATTACH_BZR_DOWNLOAD="$COMPARE_EXCHANGE_DIR/public-bzr-download.txt"
_ATTACH_PYBZ_DOWNLOAD="$COMPARE_EXCHANGE_DIR/public-pybz-download.txt"
_ATTACH_BULK_DIR="$COMPARE_EXCHANGE_DIR/public-bzr-bulk"
if [[ -n $ATTACHMENT_BZR_ID && -n $ATTACHMENT_PYBZ_ID ]] &&
    resource_bzr public-bzr-download rest REST attachment download "$ATTACHMENT_BZR_ID" \
        --out "$_ATTACH_BZR_DOWNLOAD" &&
    resource_pybz public-pybz-download attachment_download \
        "$(jq -cn --argjson id "$ATTACHMENT_PYBZ_ID" \
            '{transport:"REST",attachment_id:$id,
              destination:"/work/compare/public-pybz-download.txt"}')" REST &&
    resource_bzr public-bzr-bulk rest REST attachment download \
        --bug "$ATTACHMENT_BZR_BUG_ID" --out-dir "$_ATTACH_BULK_DIR"; then
    _ATTACH_BULK_FILE=$(jq -r --argjson id "$ATTACHMENT_BZR_ID" \
        '.bug_results[].files[] | select(.attachment_id == $id).path' \
        "$COMPARE_EXCHANGE_DIR/public-bzr-bulk.bzr.stdout.json")
    if [[ -f $_ATTACH_BULK_FILE ]] &&
        [[ $(attachment_digest "$ATTACHMENT_SOURCE") == \
            "$(attachment_digest "$_ATTACH_BZR_DOWNLOAD")" ]] &&
        [[ $(attachment_digest "$ATTACHMENT_SOURCE") == \
            "$(attachment_digest "$_ATTACH_PYBZ_DOWNLOAD")" ]] &&
        [[ $(attachment_digest "$ATTACHMENT_SOURCE") == \
            "$(attachment_digest "$_ATTACH_BULK_FILE")" ]]; then
        test_pass
    else
        test_fail "single or per-bug attachment content digest differs"
    fi
elif [[ $TEST_RESULT_PENDING -eq 0 ]]; then
    test_fail "public attachment precondition or download failed"
fi

test_begin "attachment-flags" "attachment flag persisted outcome"
if [[ -n $ATTACHMENT_BZR_ID && -n $ATTACHMENT_PYBZ_ID ]] &&
    resource_bzr flag-bzr-update rest REST attachment update "$ATTACHMENT_BZR_ID" \
        --flag 'bzr_compare_attachment_review?' &&
    resource_pybz flag-pybz-update attachment_flag \
        "$(jq -cn --argjson bug_id "$ATTACHMENT_PYBZ_BUG_ID" \
            --argjson attachment_id "$ATTACHMENT_PYBZ_ID" \
            '{transport:"REST",bug_id:$bug_id,attachment_id:$attachment_id,
              flag_name:"bzr_compare_attachment_review",status:"?"}')" REST &&
    resource_bzr flag-bzr-view rest REST attachment view "$ATTACHMENT_BZR_ID" &&
    resource_pybz flag-pybz-view attachment_get \
        "$(jq -cn --argjson id "$ATTACHMENT_PYBZ_ID" \
            '{transport:"REST",attachment_ids:[$id]}')" REST; then
    jq '[.flags[] | select(.name == "bzr_compare_attachment_review") |
          {name,status,requestee:(.requestee // null)}]' \
        "$COMPARE_EXCHANGE_DIR/flag-bzr-view.bzr.stdout.json" \
        >"$COMPARE_EXCHANGE_DIR/flag.bzr.json"
    jq --argjson id "$ATTACHMENT_PYBZ_ID" \
        '[.attachments[($id | tostring)].flags[] |
          select(.name == "bzr_compare_attachment_review") |
          {name,status,requestee:(.requestee // null)}]' \
        "$COMPARE_EXCHANGE_DIR/flag-pybz-view.pybz.result.json" \
        >"$COMPARE_EXCHANGE_DIR/flag.pybz.json"
    if jq -e 'length == 1 and .[0].status == "?"' \
        "$COMPARE_EXCHANGE_DIR/flag.bzr.json" >/dev/null &&
        jq -e 'length == 1 and .[0].status == "?"' \
            "$COMPARE_EXCHANGE_DIR/flag.pybz.json" >/dev/null &&
        resource_equal attachment-flag "$COMPARE_EXCHANGE_DIR/flag.bzr.json" \
            "$COMPARE_EXCHANGE_DIR/flag.pybz.json"; then
        test_pass
    elif [[ $TEST_RESULT_PENDING -eq 0 ]]; then
        test_fail "attachment flag readback differs"
    fi
fi

attachment_private_case() {
    local slug="$1" api="$2" transport="$3"
    local summary="$ATTACHMENT_STEM $slug"
    local bzr_download="$COMPARE_EXCHANGE_DIR/${slug}-bzr-download.txt"
    local pybz_download="$COMPARE_EXCHANGE_DIR/${slug}-pybz-download.txt"

    resource_gap_reset
    if ! attachment_upload_pair "$slug" true "$transport" ||
        ! resource_bzr "$slug-bzr-list" "$api" "$transport" \
            attachment list "$ATTACHMENT_BZR_BUG_ID" ||
        ! resource_pybz "$slug-pybz-list" attachment_list \
            "$(jq -cn --arg transport "$transport" \
                --argjson id "$ATTACHMENT_PYBZ_BUG_ID" \
                '{transport:$transport,bug_ids:[$id]}')" "$transport" ||
        ! attachment_normalize_lists "$slug" "$summary" true ||
        ! resource_bzr "$slug-bzr-view" "$api" "$transport" \
            attachment view "$ATTACHMENT_BZR_ID" ||
        ! resource_pybz "$slug-pybz-view" attachment_get \
            "$(jq -cn --arg transport "$transport" --argjson id "$ATTACHMENT_PYBZ_ID" \
                '{transport:$transport,attachment_ids:[$id]}')" "$transport" ||
        ! resource_bzr "$slug-bzr-download" "$api" "$transport" attachment download \
            "$ATTACHMENT_BZR_ID" --out "$bzr_download" ||
        ! resource_pybz "$slug-pybz-download" attachment_download \
            "$(jq -cn --arg transport "$transport" --argjson id "$ATTACHMENT_PYBZ_ID" \
                --arg destination "/work/compare/${pybz_download##*/}" \
                '{transport:$transport,attachment_id:$id,destination:$destination}')" \
            "$transport"; then
        return 0
    fi
    if jq -e --argjson id "$ATTACHMENT_BZR_ID" \
        '.id == $id and (.is_private == true or .is_private == 1)' \
        "$COMPARE_EXCHANGE_DIR/${slug}-bzr-view.bzr.stdout.json" >/dev/null &&
        jq -e --argjson id "$ATTACHMENT_PYBZ_ID" \
            '.attachments[($id | tostring)] |
             ((.id | tonumber) == $id and (.is_private == true or .is_private == 1))' \
            "$COMPARE_EXCHANGE_DIR/${slug}-pybz-view.pybz.result.json" >/dev/null &&
        [[ $(attachment_digest "$ATTACHMENT_SOURCE") == \
            "$(attachment_digest "$bzr_download")" ]] &&
        [[ $(attachment_digest "$ATTACHMENT_SOURCE") == \
            "$(attachment_digest "$pybz_download")" ]]; then
        test_pass
    else
        test_fail "private attachment list, view, or content differs"
    fi
}

test_begin "private-attachments-rest" "private attachment visibility over REST"
attachment_private_case private-rest rest REST

test_begin "private-attachments-xmlrpc" "private attachment visibility over XML-RPC"
attachment_private_case private-xmlrpc xmlrpc XMLRPC

attachment_parser_gap() {
    local issue="$1" diagnostic="$2" usage="$3"

    if [[ $BZR_EXIT -eq 0 ]]; then
        test_pass
    elif [[ $BZR_EXIT -eq 2 ]] && grep -Fxq "$diagnostic" "$BZR_STDERR" &&
        grep -Fxq "$usage" "$BZR_STDERR"; then
        test_fail "bzr attachment capability is not implemented"
        resource_gap_allow
    else
        test_fail "bzr attachment parser result was not the controlled gap"
    fi
    resource_expect_gap "$issue"
}

test_begin "multi-bug-upload" "attachment upload accepts multiple bug targets"
resource_gap_reset
_ATTACH_MULTI_BUG=""
if attachment_create_bug multi-pybz-second "$ATTACHMENT_STEM multi second"; then
    _ATTACH_MULTI_BUG="$ATTACHMENT_CREATED_BUG_ID"
    if resource_pybz multi-pybz-upload attachment_upload \
        "$(jq -cn --argjson first "$ATTACHMENT_PYBZ_BUG_ID" \
            --argjson second "$_ATTACH_MULTI_BUG" \
            '{transport:"REST",bug_ids:[$first,$second],
              source:"/work/compare/attachment-source.txt",summary:"multi upload",
              file_name:"attachment-source.txt",content_type:"text/plain",comment:"multi",
              is_private:false}')" REST &&
        jq -e '.attachment_ids | length == 2 and all(.[]; type == "number" and . > 0)' \
            "$COMPARE_EXCHANGE_DIR/multi-pybz-upload.pybz.result.json" >/dev/null; then
        run_bzr --server "$RESOURCE_SERVER" attachment upload "$ATTACHMENT_BZR_BUG_ID" \
            "$_ATTACH_MULTI_BUG" "$ATTACHMENT_SOURCE"
        attachment_parser_gap 674 \
            "error: unexpected argument '$ATTACHMENT_SOURCE' found" \
            'Usage: bzr attachment upload [OPTIONS] <BUG_ID> <FILE>'
    elif [[ $TEST_RESULT_PENDING -eq 0 ]]; then
        test_fail "python-bugzilla multi-bug upload evidence is invalid"
    fi
elif [[ $TEST_RESULT_PENDING -eq 0 ]]; then
    test_fail "multi-bug upload precondition failed"
fi

test_begin "ignore-obsolete" "bulk attachment download ignores obsolete records"
resource_gap_reset
if [[ -n $ATTACHMENT_PYBZ_ID ]] &&
    resource_bzr obsolete-setup rest REST attachment update "$ATTACHMENT_PYBZ_ID" --obsolete &&
    resource_pybz obsolete-pybz-download attachment_cli_download_bug \
        "$(jq -cn --argjson id "$ATTACHMENT_PYBZ_BUG_ID" \
            '{transport:"REST",bug_id:$id,
              destination:"/work/compare/obsolete-pybz",ignore_obsolete:true}')" REST &&
    jq -e '.files | length == 1 and
        all(.[]; startswith("attachment-source.txt"))' \
        "$COMPARE_EXCHANGE_DIR/obsolete-pybz-download.pybz.result.json" >/dev/null; then
    run_bzr --server "$RESOURCE_SERVER" attachment download \
        --bug "$ATTACHMENT_BZR_BUG_ID" --ignore-obsolete \
        --out-dir "$COMPARE_EXCHANGE_DIR/obsolete-bzr"
    attachment_parser_gap 674 \
        "error: unexpected argument '--ignore-obsolete' found" \
        'Usage: bzr attachment download --bug <BUG_ID> [ID]...'
elif [[ $TEST_RESULT_PENDING -eq 0 ]]; then
    test_fail "python-bugzilla obsolete-filter evidence is invalid"
fi
