#!/bin/bash
# Persisted public and private comment comparisons.

printf -v COMMENT_RUN_TOKEN '%x-%x-%x' "$$" "$RANDOM" "$RANDOM"
COMMENT_RUN_TOKEN="${COMMENT_RUN_TOKEN:0:18}"
COMMENT_STEM="bzr-pybz-comments-${BZ_VERSION}-${COMMENT_RUN_TOKEN}"
COMMENT_CREATED_BUG_ID=""

comment_create_bug() {
    local name="$1" summary="$2" id

    COMMENT_CREATED_BUG_ID=""
    if ! resource_bzr "$name" rest REST bug create \
        --product TestProduct --component TestComponent --summary "$summary" \
        --description "comment comparison fixture" --op-sys Linux --platform PC \
        --priority Normal --severity normal; then
        return 1
    fi
    if ! id=$(resource_positive_id \
        "$COMPARE_EXCHANGE_DIR/${name}.bzr.stdout.json" '.id'); then
        test_fail "could not create comment comparison bug"
        return 1
    fi
    COMMENT_CREATED_BUG_ID="$id"
}

comment_compare_case() {
    local slug="$1" api="$2" transport="$3" private="$4"
    local bzr_id pybz_id text bzr_normalized pybz_normalized bzr_status=0

    resource_gap_reset
    text="$COMMENT_STEM $slug"
    if ! comment_create_bug "$slug-bzr-create" "$COMMENT_STEM $slug [bzr]"; then
        return 0
    fi
    bzr_id="$COMMENT_CREATED_BUG_ID"
    if ! comment_create_bug "$slug-pybz-create" "$COMMENT_STEM $slug [pybz]"; then
        return 0
    fi
    pybz_id="$COMMENT_CREATED_BUG_ID"
    if [[ $private == true ]]; then
        resource_bzr "$slug-add" rest REST comment add "$bzr_id" \
            --body "$text" --private || bzr_status=$?
    else
        resource_bzr "$slug-add" rest REST comment add "$bzr_id" \
            --body "$text" || bzr_status=$?
    fi
    if [[ $bzr_status -ne 0 ]] ||
        ! resource_pybz "$slug-add" comment_add \
            "$(jq -cn --arg transport "$transport" --argjson id "$pybz_id" \
                --arg text "$text" --argjson private "$private" \
                '{transport:$transport,bug_id:$id,text:$text,is_private:$private}')" \
            "$transport"; then
        return 0
    fi
    if ! resource_bzr "$slug-list" "$api" "$transport" comment list "$bzr_id" ||
        ! resource_pybz "$slug-list" comment_list \
            "$(jq -cn --arg transport "$transport" --argjson id "$pybz_id" \
                '{transport:$transport,bug_id:$id}')" "$transport"; then
        return 0
    fi
    bzr_normalized="$COMPARE_EXCHANGE_DIR/${slug}.bzr.comments.json"
    pybz_normalized="$COMPARE_EXCHANGE_DIR/${slug}.pybz.comments.json"
    jq --arg text "$text" \
        '[.[] | select(.text == $text) |
          {text, is_private:(.is_private == true or .is_private == 1)}]' \
        "$COMPARE_EXCHANGE_DIR/${slug}-list.bzr.stdout.json" >"$bzr_normalized"
    jq --argjson id "$pybz_id" --arg text "$text" \
        '[.bugs[($id | tostring)].comments[] | select(.text == $text) |
          {text, is_private:(.is_private == true or .is_private == 1)}]' \
        "$COMPARE_EXCHANGE_DIR/${slug}-list.pybz.result.json" >"$pybz_normalized"
    if ! jq -e --argjson private "$private" \
        'length == 1 and .[0].is_private == $private' "$bzr_normalized" >/dev/null ||
        ! jq -e --argjson private "$private" \
            'length == 1 and .[0].is_private == $private' "$pybz_normalized" >/dev/null; then
        test_fail "comment privacy or positive-control record is missing"
        return 0
    fi
    if resource_equal "$slug" "$bzr_normalized" "$pybz_normalized"; then
        test_pass
    fi
}

test_begin "public-comments" "public comment persisted outcome"
comment_compare_case public-comments rest REST false

test_begin "private-comments-rest" "private comment visibility over REST"
comment_compare_case private-comments-rest rest REST true

test_begin "private-comments-xmlrpc" "private comment visibility over XML-RPC"
comment_compare_case private-comments-xmlrpc xmlrpc XMLRPC true
