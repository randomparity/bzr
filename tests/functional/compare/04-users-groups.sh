#!/bin/bash
# User, group, and membership persisted-state comparisons.

printf -v USER_GROUP_RUN_TOKEN '%x-%x-%x' "$$" "$RANDOM" "$RANDOM"
USER_GROUP_RUN_TOKEN="${USER_GROUP_RUN_TOKEN:0:18}"
USER_GROUP_BZR_EMAIL="bzr-${USER_GROUP_RUN_TOKEN}@test.bzr"
USER_GROUP_PYBZ_EMAIL="pybz-${USER_GROUP_RUN_TOKEN}@test.bzr"
USER_GROUP_FIXTURE="compare-${USER_GROUP_RUN_TOKEN}"
USER_GROUP_DESCRIPTION="Comparison group ${USER_GROUP_RUN_TOKEN}"

user_group_normalize_bzr() {
    local source="$1" email="$2" destination="$3"

    jq --arg email "$email" \
        '[.[] | select((.email // .name) == $email) |
          {email:"paired",real_name:(.real_name // ""),can_login,
           groups:([.groups[]? | if type == "object" then .name else . end] | sort)}]' \
        "$source" >"$destination"
}

user_group_normalize_pybz() {
    local source="$1" email="$2" destination="$3" expression="$4"

    jq --arg email "$email" "$expression |
        [select(.email == \$email) |
          {email:\"paired\",real_name:(.real_name // \"\"),can_login,
           groups:(.groups | sort)}]" "$source" >"$destination"
}

user_group_require_one() {
    jq -e 'length == 1 and .[0].can_login == true' "$1" >/dev/null
}

test_begin "user-create-get-search" "user create, exact get, and search"
if resource_bzr user-bzr-create xmlrpc XMLRPC user create \
    --email "$USER_GROUP_BZR_EMAIL" --password 'ComparePass1!' &&
    resource_require_positive_id \
        "$COMPARE_EXCHANGE_DIR/user-bzr-create.bzr.stdout.json" '.id' bzr-user-create &&
    resource_pybz user-pybz-create user_create \
        "$(jq -cn --arg email "$USER_GROUP_PYBZ_EMAIL" \
            '{transport:"XMLRPC",email:$email,password:"ComparePass1!"}')" XMLRPC &&
    resource_require_positive_id \
        "$COMPARE_EXCHANGE_DIR/user-pybz-create.pybz.result.json" '.id' \
        python-bugzilla-user-create &&
    resource_bzr user-bzr-search rest REST user search "$USER_GROUP_BZR_EMAIL" --details &&
    resource_pybz user-pybz-get user_get \
        "$(jq -cn --arg email "$USER_GROUP_PYBZ_EMAIL" \
            '{transport:"REST",email:$email}')" REST &&
    resource_pybz user-pybz-search user_search \
        "$(jq -cn --arg pattern "$USER_GROUP_PYBZ_EMAIL" \
            '{transport:"REST",pattern:$pattern}')" REST; then
    user_group_normalize_bzr \
        "$COMPARE_EXCHANGE_DIR/user-bzr-search.bzr.stdout.json" \
        "$USER_GROUP_BZR_EMAIL" "$COMPARE_EXCHANGE_DIR/user.bzr.json"
    user_group_normalize_pybz \
        "$COMPARE_EXCHANGE_DIR/user-pybz-get.pybz.result.json" \
        "$USER_GROUP_PYBZ_EMAIL" "$COMPARE_EXCHANGE_DIR/user.pybz.get.json" '.'
    user_group_normalize_pybz \
        "$COMPARE_EXCHANGE_DIR/user-pybz-search.pybz.result.json" \
        "$USER_GROUP_PYBZ_EMAIL" "$COMPARE_EXCHANGE_DIR/user.pybz.search.json" '.[]'
    if user_group_require_one "$COMPARE_EXCHANGE_DIR/user.bzr.json" &&
        user_group_require_one "$COMPARE_EXCHANGE_DIR/user.pybz.get.json" &&
        user_group_require_one "$COMPARE_EXCHANGE_DIR/user.pybz.search.json" &&
        resource_equal user-get "$COMPARE_EXCHANGE_DIR/user.bzr.json" \
            "$COMPARE_EXCHANGE_DIR/user.pybz.get.json" &&
        resource_equal user-search "$COMPARE_EXCHANGE_DIR/user.bzr.json" \
            "$COMPARE_EXCHANGE_DIR/user.pybz.search.json"; then
        test_pass
    elif [[ $TEST_RESULT_PENDING -eq 0 ]]; then
        test_fail "user create, exact get, or search outcome differs"
    fi
fi

test_begin "group-get-and-list" "group get and list"
if resource_bzr group-bzr-create rest REST group create --name "$USER_GROUP_FIXTURE" \
    --description "$USER_GROUP_DESCRIPTION" &&
    resource_require_positive_id \
        "$COMPARE_EXCHANGE_DIR/group-bzr-create.bzr.stdout.json" '.id' bzr-group-create &&
    resource_bzr group-bzr-view xmlrpc XMLRPC group view "$USER_GROUP_FIXTURE" &&
    resource_pybz group-pybz-get group_get \
        "$(jq -cn --arg name "$USER_GROUP_FIXTURE" \
            '{transport:"XMLRPC",name:$name,membership:false}')" XMLRPC &&
    resource_pybz group-pybz-list group_list \
        "$(jq -cn --arg name "$USER_GROUP_FIXTURE" \
            '{transport:"XMLRPC",names:[$name],membership:false}')" XMLRPC; then
    jq '{name,description,is_active}' \
        "$COMPARE_EXCHANGE_DIR/group-bzr-view.bzr.stdout.json" \
        >"$COMPARE_EXCHANGE_DIR/group.bzr.json"
    jq '{name,description,is_active}' \
        "$COMPARE_EXCHANGE_DIR/group-pybz-get.pybz.result.json" \
        >"$COMPARE_EXCHANGE_DIR/group.pybz.get.json"
    jq 'map({name,description,is_active})' \
        "$COMPARE_EXCHANGE_DIR/group-pybz-list.pybz.result.json" \
        >"$COMPARE_EXCHANGE_DIR/group.pybz.list.json"
    if jq -e --arg name "$USER_GROUP_FIXTURE" '.name == $name' \
        "$COMPARE_EXCHANGE_DIR/group.bzr.json" >/dev/null &&
        jq -e --arg name "$USER_GROUP_FIXTURE" '.name == $name' \
            "$COMPARE_EXCHANGE_DIR/group.pybz.get.json" >/dev/null &&
        jq -e 'length == 1' "$COMPARE_EXCHANGE_DIR/group.pybz.list.json" >/dev/null &&
        resource_equal group-get "$COMPARE_EXCHANGE_DIR/group.bzr.json" \
            "$COMPARE_EXCHANGE_DIR/group.pybz.get.json" &&
        jq -e --slurpfile expected "$COMPARE_EXCHANGE_DIR/group.bzr.json" \
            'length == 1 and .[0] == $expected[0]' \
            "$COMPARE_EXCHANGE_DIR/group.pybz.list.json" >/dev/null; then
        test_pass
    elif [[ $TEST_RESULT_PENDING -eq 0 ]]; then
        test_fail "group get or list outcome differs"
    fi
fi

user_group_membership_present_bzr() {
    local email="$1" want="$2"

    resource_bzr membership-bzr-read rest REST user search "$email" --details || return 1
    jq -e --arg email "$email" --arg group "$USER_GROUP_FIXTURE" --argjson want "$want" \
        '[.[] | select((.email // .name) == $email)][0] as $user |
         $user != null and
         (any($user.groups[]?; (if type == "object" then .name else . end) == $group)
          == $want)' "$COMPARE_EXCHANGE_DIR/membership-bzr-read.bzr.stdout.json" >/dev/null
}

user_group_membership_present_pybz() {
    local email="$1" want="$2"

    resource_pybz membership-pybz-read user_get \
        "$(jq -cn --arg email "$email" '{transport:"REST",email:$email}')" REST || return 1
    jq -e --arg group "$USER_GROUP_FIXTURE" --argjson want "$want" \
        '((.groups | index($group)) != null) == $want' \
        "$COMPARE_EXCHANGE_DIR/membership-pybz-read.pybz.result.json" >/dev/null
}

test_begin "membership-add-remove" "membership add, prove, remove, and prove absent"
if resource_bzr membership-bzr-add rest REST group add-user \
    --group "$USER_GROUP_FIXTURE" --user "$USER_GROUP_BZR_EMAIL"; then
    resource_membership_record "$USER_GROUP_BZR_EMAIL" "$USER_GROUP_FIXTURE"
    if resource_pybz membership-pybz-add user_groups \
        "$(jq -cn --arg email "$USER_GROUP_PYBZ_EMAIL" --arg group "$USER_GROUP_FIXTURE" \
            '{transport:"REST",email:$email,action:"add",groups:[$group]}')" REST; then
        resource_membership_record "$USER_GROUP_PYBZ_EMAIL" "$USER_GROUP_FIXTURE"
        if user_group_membership_present_bzr "$USER_GROUP_BZR_EMAIL" true &&
            user_group_membership_present_pybz "$USER_GROUP_PYBZ_EMAIL" true &&
            resource_bzr membership-bzr-remove rest REST group remove-user \
                --group "$USER_GROUP_FIXTURE" --user "$USER_GROUP_BZR_EMAIL" &&
            user_group_membership_present_bzr "$USER_GROUP_BZR_EMAIL" false; then
            resource_membership_clear "$USER_GROUP_BZR_EMAIL" "$USER_GROUP_FIXTURE"
            if resource_pybz membership-pybz-remove user_groups \
                "$(jq -cn --arg email "$USER_GROUP_PYBZ_EMAIL" \
                    --arg group "$USER_GROUP_FIXTURE" \
                    '{transport:"REST",email:$email,action:"remove",groups:[$group]}')" REST &&
                user_group_membership_present_pybz "$USER_GROUP_PYBZ_EMAIL" false; then
                resource_membership_clear "$USER_GROUP_PYBZ_EMAIL" "$USER_GROUP_FIXTURE"
                test_pass
            elif [[ $TEST_RESULT_PENDING -eq 0 ]]; then
                test_fail "python-bugzilla membership removal did not persist"
            fi
        elif [[ $TEST_RESULT_PENDING -eq 0 ]]; then
            test_fail "membership add or bzr removal did not persist"
        fi
    fi
fi
