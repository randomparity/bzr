# 08e-bugs-restricted-access
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 8e: Restricted-bug access (issue #504)
# ══════════════════════════════════════════════════════════════════════
# #504: `bug view` reported "bug not found" (exit 2) for a bug in an
# access-restricted product the caller could see. ADR 0015 settled that bzr
# relays whatever the server said and never substitutes an empty result.
#
# Scope note, so the coverage is not overread: a *stock* Bugzilla cannot
# reproduce #504. Both masking paths need a server-side quirk — an error
# payload carrying an empty `bugs: []`, or a 100500 extension crash on the
# direct lookup — and those are covered by wiremock unit tests in
# `src/client/response_tests.rs` and `src/client/resources/bug_tests.rs`.
# What this phase does is pin the *contract* against a real server, so a
# regression that reintroduces masking (exit 2 / not_found) reddens the
# suite instead of passing a bare "expected non-zero exit" check.
#
# The three directions below were verified against Bugzilla 5.0.6:
#   authenticated group member  → exit 0, bug returned
#   authenticated non-member    → exit 4, api_code 102
#   anonymous                   → exit 4, api_code 102
#
# #715 (ADR 0057): the 401 alternate-auth retry used to judge its outcome by
# HTTP status alone. Bugzilla maps fifteen distinct WebService error codes onto
# HTTP 401, so a policy refusal read as "auth failed again", the retried
# response was discarded, and the user was told to log in on a request the
# server had authenticated.
#
# Same scope limit as the #504 tests above, for the same kind of reason: a stock
# Bugzilla cannot reproduce the masking. It needs the FIRST attempt to fail
# authentication while the retry succeeds — the server-side condition in #713 —
# and a stock container authenticates both. Neither test below can be reddened
# by mutating `code_proves_auth_failure`; the divergent path is driven by
# wiremock in `src/client/transport_tests.rs`. What these two pin is the
# user-visible contract, in both directions, so a regression that reports a
# policy refusal as a login failure reddens the suite.
echo "── Phase 8e: Restricted-bug access (#504) ───────────────────"

# ── Fixture: a second credentialed identity ──────────────────────────
# Every version entrypoint seeds exactly one API key, bound to the admin.
# With only that identity, the "member sees the restricted bug" direction is
# untestable: the admin is the bug's own reporter and an admin besides, so a
# passing view proves reporter/admin access, never group membership.
RESTRICTED_USER="restricted-member@test.bzr"
RESTRICTED_KEY="FuncTestRestricted0123456789abcdef012345"
RESTRICTED_GROUP=$(unique_name restrict-grp)
_RESTRICTED_ALIASES_OK=1
# 08c unsets its own fixture array, so declare the shared create args here.
_RA=(--product FuncTestProd --component Backend --op-sys Linux
    --platform PC --description d)

test_begin "fixture-second-credentialed-identity" "fixture: second credentialed identity"
# Create best-effort: the account survives in a reused container, and
# Bugzilla's duplicate-account wording ("There is already an account with the
# login name …") differs from other resources' "already exists". Verify the
# outcome — that the identity authenticates — instead of pattern-matching the
# error, so the fixture is idempotent on a warm container without depending on
# any server message.
run_bzr user create --email "$RESTRICTED_USER" --full-name "Restricted Member" \
    --password "RestrictPass1!"
_RU_SQL=$(mktemp /tmp/bzr-func-restricted-key.XXXXXX.sql)
cat >"$_RU_SQL" <<SQL
INSERT IGNORE INTO user_api_keys (user_id, api_key, description, revoked)
SELECT userid, '${RESTRICTED_KEY}', 'functional-test-restricted', 0
FROM profiles
WHERE login_name = '${RESTRICTED_USER}'
LIMIT 1;
SQL
if run_bugzilla_sql_file "$_RU_SQL"; then
    run_bzr config set-server restricted --url "$BZ_URL" \
        --api-key "$RESTRICTED_KEY" --auth-method query_param \
        --email "$RESTRICTED_USER"
    if assert_success; then
        # Prove the credential works before the access tests lean on it; a
        # silently-unseeded key would otherwise surface as a confusing
        # config error four tests later.
        run_bzr_raw --json --server restricted whoami
        # `name` and `login` are both nullable and which one carries the login
        # depends on the probe that detected auth, so match against either.
        if assert_success && assert_json_exists '.id' &&
            assert_json_contains \
                '[.name, .login] | map(select(. != null)) | join(",")' \
                "$RESTRICTED_USER"; then
            test_pass
        fi
    fi
else
    test_fail "could not seed API key for $RESTRICTED_USER"
fi
rm -f "$_RU_SQL"
unset _RU_SQL

test_begin "fixture-explicit-restricted-rest-and-xml-rpc-aliases" "fixture: explicit restricted REST and XML-RPC aliases"
for _RESTRICTED_MODE in rest xmlrpc; do
    run_bzr config set-server "restricted-$_RESTRICTED_MODE" --url "$BZ_URL" \
        --api-key "$RESTRICTED_KEY" --auth-method query_param \
        --email "$RESTRICTED_USER" --api "$_RESTRICTED_MODE"
    [[ $BZR_EXIT -eq 0 ]] || _RESTRICTED_ALIASES_OK=0
done
if [[ $_RESTRICTED_ALIASES_OK -eq 1 ]]; then
    test_pass
else
    test_fail "could not configure explicit restricted transport aliases"
fi

test_begin "fixture-group-restricted-bug" "fixture: group-restricted bug"
# The group name is per-run unique, so a plain success assertion is correct
# here — unlike the account above, this cannot collide on a warm container.
run_bzr group create --name "$RESTRICTED_GROUP" --description "restricted access"
if ! assert_success; then
    : # assert_success already recorded the failure
else
    _RG_SQL=$(mktemp /tmp/bzr-func-restricted-ctl.XXXXXX.sql)
    cat >"$_RG_SQL" <<SQL
INSERT INTO group_control_map
    (group_id, product_id, entry, membercontrol, othercontrol, canedit,
     editcomponents, editbugs, canconfirm)
SELECT g.id, p.id, 0, 1, 1, 1, 0, 1, 1
FROM groups AS g
JOIN products AS p ON p.name = 'FuncTestProd'
WHERE g.name = '${RESTRICTED_GROUP}'
ON DUPLICATE KEY UPDATE membercontrol = 1, othercontrol = 1;
SQL
    if run_bugzilla_sql_file "$_RG_SQL"; then
        RESTRICTED_BUG=$(make_bug "${_RA[@]}" --summary "restricted access probe" \
            --groups "$RESTRICTED_GROUP")
        if [[ -n "$RESTRICTED_BUG" ]]; then test_pass; else
            test_fail "could not create restricted bug"
        fi
    else
        test_fail "could not enable $RESTRICTED_GROUP on FuncTestProd"
    fi
    rm -f "$_RG_SQL"
    unset _RG_SQL
fi

# ── The three access directions ──────────────────────────────────────

for _RESTRICTED_MODE in rest xmlrpc; do
    case "$_RESTRICTED_MODE" in
    rest)
        test_begin "rest-credentialed-adjacency-proves-access-error-after-valid-login" "credentialed rest adjacency proves access error after valid_login"
        ;;
    xmlrpc)
        test_begin "xmlrpc-credentialed-adjacency-proves-access-error-after-valid-login" "credentialed xmlrpc adjacency proves access error after valid_login"
        ;;
    *)
        printf 'unexpected restricted adjacency mode: %s\n' "$_RESTRICTED_MODE" >&2
        return 1
        ;;
    esac
    if [[ -n "$RESTRICTED_BUG" ]] && [[ $_RESTRICTED_ALIASES_OK -eq 1 ]]; then
        # A credentialed code-102 result is emitted only after adjacency has
        # proved these exact credentials through live rest/valid_login.
        run_bzr_raw --json --server "restricted-$_RESTRICTED_MODE" \
            bug adjacency "$RESTRICTED_BUG"
        if assert_exit_code 0 &&
            assert_raw_json '.schema_version' '3.0.2' &&
            assert_json '. == {
                requests: [{
                    requested: "'"$RESTRICTED_BUG"'",
                    error: {type: "inaccessible", api_code: 102}
                }],
                bugs: []
            }' 'true'; then
            test_pass
        fi
    else
        test_skip "no restricted bug or explicit transport alias"
    fi
done

test_begin "anonymous-view-of-a-restricted-bug-reports-access-not-absence" "anonymous view of a restricted bug reports access, not absence"
if [[ -n "$RESTRICTED_BUG" ]]; then
    run_bzr_raw --json --server public bug view "$RESTRICTED_BUG"
    # The contract #504 broke: an access failure must not be reported as
    # not-found. Assert the code, not merely "non-zero".
    if assert_exit_code 4 &&
        assert_stderr_json '.error.type' "api" &&
        assert_stderr_json '.error.api_code' "102" &&
        assert_stderr_not_contains "not found"; then
        test_pass
    fi
else test_skip "no restricted bug"; fi

test_begin "authenticated-non-member-gets-an-access-error-not-absence" "authenticated non-member gets an access error, not absence"
if [[ -n "$RESTRICTED_BUG" ]]; then
    run_bzr_raw --json --server restricted bug view "$RESTRICTED_BUG"
    if assert_exit_code 4 &&
        assert_stderr_json '.error.type' "api" &&
        assert_stderr_json '.error.api_code' "102" &&
        assert_stderr_not_contains "not found"; then
        test_pass
    fi
else test_skip "no restricted bug"; fi

test_begin "group-member-sees-the-restricted-bug" "group member sees the restricted bug"
# The reporter's actual scenario, and the direction the suite could not
# reach before: access granted purely by group membership, not by being the
# bug's reporter or an admin.
if [[ -n "$RESTRICTED_BUG" ]]; then
    run_bzr group add-user --group "$RESTRICTED_GROUP" --user "$RESTRICTED_USER"
    if assert_success; then
        run_bzr_raw --json --server restricted bug view "$RESTRICTED_BUG"
        if assert_exit_code 0 && assert_json '.id' "$RESTRICTED_BUG"; then
            test_pass
        fi
    fi
else test_skip "no restricted bug"; fi

test_begin "member-view-is-stable-across-repeated-invocations" "member view is stable across repeated invocations"
# #504 was reported as intermittent. A loop cannot prove absence of a race,
# but it does catch a fix that only works on a cold cache or first request.
if [[ -n "$RESTRICTED_BUG" ]]; then
    _RB_FAILURES=0
    for _ in 1 2 3 4 5; do
        run_bzr_raw --json --server restricted bug view "$RESTRICTED_BUG"
        if [[ $BZR_EXIT -ne 0 ]]; then
            _RB_FAILURES=$((_RB_FAILURES + 1))
        fi
    done
    if [[ $_RB_FAILURES -eq 0 ]]; then
        test_pass
    else
        test_fail "$_RB_FAILURES/5 member views failed"
    fi
    unset _RB_FAILURES
else test_skip "no restricted bug"; fi

# ── #719: `bug links` reads its root on the faultable direct path ─────
# Bugzilla's search endpoint filters a bug the caller cannot see into a 200
# carrying an empty list and no error at all, while the direct endpoint
# faults. Reading the root through search therefore reported a permission
# outcome as absence — `bug not found` (exit 2) for a bug `bug view` and
# `bug history` display in the same session. `--api xmlrpc` already read the
# faultable path and is the reference oracle these assertions pin REST to.

test_begin "anonymous-links-of-a-restricted-bug-reports-access-not-absence" "anonymous links of a restricted bug reports access, not absence"
# The credentialless direction. Before the fix this exited 2 with
# "bug not found"; the search endpoint gave bzr nothing else to report.
if [[ -n "$RESTRICTED_BUG" ]]; then
    run_bzr_raw --json --server public bug links "$RESTRICTED_BUG"
    if assert_exit_code 4 &&
        assert_stderr_json '.error.type' "api" &&
        assert_stderr_json '.error.api_code' "102" &&
        assert_stderr_not_contains "not found"; then
        test_pass
    fi
else test_skip "no restricted bug"; fi

# A second bug in the same group, wired as a dependency of the first. Without
# an edge the root's graph is empty, and a REST-vs-XML-RPC stdout comparison
# over two empty graphs asserts nothing about content. One edge makes it
# discriminating: both arms have to return the same neighbour, not merely the
# same emptiness.
RESTRICTED_DEP_BUG=""
test_begin "fixture-restricted-dependency-edge" "fixture: restricted dependency edge"
if [[ -n "$RESTRICTED_BUG" ]]; then
    RESTRICTED_DEP_BUG=$(make_bug "${_RA[@]}" --summary "restricted dependency probe" \
        --groups "$RESTRICTED_GROUP")
    if [[ -z "$RESTRICTED_DEP_BUG" ]]; then
        test_fail "could not create the restricted dependency bug"
    else
        run_bzr bug update "$RESTRICTED_BUG" --depends-on-add "$RESTRICTED_DEP_BUG"
        if assert_success; then test_pass; fi
    fi
else test_skip "no restricted bug"; fi

test_begin "group-member-links-the-restricted-bug" "group member links the restricted bug"
# Criterion 1: the member reads the root and the walk reaches the neighbour.
#
# A no-regression guard, not a reproduction of #719: the `restricted` alias
# authenticates by query parameter, which a stock Bugzilla honours on the search
# endpoint as well as the direct one, so this direction exited 0 before the fix
# too. The directions that actually redden on pre-fix code are the anonymous one
# above, the header-auth one below, and 09c's nonexistent root.
if [[ -n "$RESTRICTED_BUG" ]] && [[ -n "$RESTRICTED_DEP_BUG" ]]; then
    run_bzr_raw --json --server restricted bug links "$RESTRICTED_BUG"
    if assert_exit_code 0 &&
        assert_json 'length' "1" &&
        assert_json '.[0].id' "$RESTRICTED_DEP_BUG" &&
        assert_json '.[0].relation' "depends_on"; then
        test_pass
    fi
else test_skip "no restricted bug or dependency edge"; fi

# A restricted alias authenticating by header. On a stock Bugzilla the direct
# endpoint answers a header-auth key with 401/102 — which bzr's alternate-auth
# retry repairs — while the *search* endpoint answers the same credentials 200
# with no rows, which no retry can see. That asymmetry is the reported defect,
# so this alias is the configuration where a member's `bug links` actually
# failed while `bug view` on the same bug succeeded.
_RESTRICTED_HEADER_OK=0
test_begin "fixture-restricted-header-auth-alias" "fixture: restricted header-auth alias"
run_bzr config set-server "restricted-header" --url "$BZ_URL" \
    --api-key "$RESTRICTED_KEY" --auth-method header \
    --email "$RESTRICTED_USER" --api rest
if assert_success; then
    _RESTRICTED_HEADER_OK=1
    test_pass
fi

test_begin "header-auth-member-reads-the-restricted-root" "header-auth member reads the restricted root"
# The issue's headline scenario. Reverting the root read to the search endpoint
# reddens this at exit 2.
#
# It asserts the exit code and the root's readability, not the graph's contents,
# and the boundary is deliberate: the root read draws the 401 that the
# alternate-auth retry repairs, while the *related*-id batch stays on the search
# endpoint, which answers the same header credential 200-with-no-rows and never
# faults. So on this alias the neighbour is silently dropped and REST returns a
# shorter graph than XML-RPC. That gap is the related-id half of the same
# structural defect, left in place on purpose (a related-id omission is
# skippable by design), and it is asserted against on the query-parameter
# alias below, where both endpoints honour the credential.
#
# The two guards are graded deliberately. A missing bug or edge is the
# best-effort fixture every consumer of $RESTRICTED_BUG in this phase skips on;
# a failed transport alias is a setup failure this file already treats as one
# (see the alias fixture above).
if [[ $_RESTRICTED_HEADER_OK -ne 1 ]] || [[ $_RESTRICTED_ALIASES_OK -ne 1 ]]; then
    test_fail "restricted transport alias setup failed"
elif [[ -z "$RESTRICTED_BUG" ]] || [[ -z "$RESTRICTED_DEP_BUG" ]]; then
    test_skip "no restricted bug or dependency edge"
else
    run_bzr_raw --json --server restricted-header bug links "$RESTRICTED_BUG"
    _RL_REST_EXIT=$BZR_EXIT
    run_bzr_raw --json --server restricted-xmlrpc bug links "$RESTRICTED_BUG"
    if [[ $_RL_REST_EXIT -ne 0 ]]; then
        test_fail "header-auth rest links exited $_RL_REST_EXIT, expected 0"
    elif [[ $_RL_REST_EXIT -ne $BZR_EXIT ]]; then
        test_fail "rest exit $_RL_REST_EXIT != xmlrpc exit $BZR_EXIT"
    else
        test_pass
    fi
    unset _RL_REST_EXIT
fi

test_begin "credentialed-rest-links-match-the-xmlrpc-oracle" "credentialed rest links match the xmlrpc oracle"
# Criterion 4 with content behind it: on the query-parameter alias both
# endpoints honour the credential, so the two arms must agree on the whole
# graph, edge included — not merely on the same emptiness.
if [[ $_RESTRICTED_ALIASES_OK -ne 1 ]]; then
    test_fail "restricted transport alias setup failed"
elif [[ -z "$RESTRICTED_BUG" ]] || [[ -z "$RESTRICTED_DEP_BUG" ]]; then
    test_skip "no restricted bug or dependency edge"
else
    run_bzr_raw --json --server restricted-rest bug links "$RESTRICTED_BUG"
    _RL_REST_EXIT=$BZR_EXIT
    _RL_REST_OUT=$(cat "$BZR_STDOUT")
    run_bzr_raw --json --server restricted-xmlrpc bug links "$RESTRICTED_BUG"
    _RL_XMLRPC_EXIT=$BZR_EXIT
    _RL_XMLRPC_OUT=$(cat "$BZR_STDOUT")
    if [[ $_RL_REST_EXIT -ne 0 ]]; then
        test_fail "rest links exited $_RL_REST_EXIT, expected 0"
    elif [[ $_RL_REST_EXIT -ne $_RL_XMLRPC_EXIT ]]; then
        test_fail "rest exit $_RL_REST_EXIT != xmlrpc exit $_RL_XMLRPC_EXIT"
    elif [[ "$_RL_REST_OUT" != "$_RL_XMLRPC_OUT" ]]; then
        test_fail "rest stdout '$_RL_REST_OUT' != xmlrpc stdout '$_RL_XMLRPC_OUT'"
    elif ! grep -q "$RESTRICTED_DEP_BUG" <<<"$_RL_REST_OUT"; then
        test_fail "both arms agreed but neither returned the edge: '$_RL_REST_OUT'"
    else
        test_pass
    fi
    unset _RL_REST_EXIT _RL_REST_OUT _RL_XMLRPC_EXIT _RL_XMLRPC_OUT
fi

# ── Product-level restriction ────────────────────────────────────────
# "Access restricted product" in the report. A mandatory group control
# (membercontrol=3) puts every bug in the product into the group
# automatically — a different Bugzilla path from per-bug group assignment.

RESTRICTED_PRODUCT=$(unique_name RestrictProd)
RESTRICTED_PROD_BUG=""

test_begin "fixture-product-with-a-mandatory-group" "fixture: product with a mandatory group"
run_bzr product create --name "$RESTRICTED_PRODUCT" \
    --description "product-level restriction" --version 1.0
if assert_success; then
    run_bzr component create --product "$RESTRICTED_PRODUCT" --name RestrictComp \
        --description "restricted component" --default-assignee "$ADMIN_EMAIL"
    if assert_success; then
        _RP_SQL=$(mktemp /tmp/bzr-func-restricted-prod.XXXXXX.sql)
        cat >"$_RP_SQL" <<SQL
INSERT INTO group_control_map
    (group_id, product_id, entry, membercontrol, othercontrol, canedit,
     editcomponents, editbugs, canconfirm)
SELECT g.id, p.id, 1, 3, 3, 0, 0, 0, 0
FROM groups AS g
JOIN products AS p ON p.name = '${RESTRICTED_PRODUCT}'
WHERE g.name = '${RESTRICTED_GROUP}'
ON DUPLICATE KEY UPDATE entry = 1, membercontrol = 3, othercontrol = 3;
SQL
        if run_bugzilla_sql_file "$_RP_SQL"; then test_pass; else
            test_fail "could not apply mandatory group control"
        fi
        rm -f "$_RP_SQL"
        unset _RP_SQL
    fi
fi

test_begin "bug-in-a-restricted-product-member-sees-it-anonymous-gets-102" "bug in a restricted product: member sees it, anonymous gets 102"
RESTRICTED_PROD_BUG=$(make_bug --product "$RESTRICTED_PRODUCT" \
    --component RestrictComp --summary "product-restricted probe" \
    --version 1.0 --op-sys Linux --platform PC --description d)
if [[ -n "$RESTRICTED_PROD_BUG" ]]; then
    run_bzr_raw --json --server restricted bug view "$RESTRICTED_PROD_BUG"
    if assert_exit_code 0 && assert_json '.id' "$RESTRICTED_PROD_BUG"; then
        run_bzr_raw --json --server public bug view "$RESTRICTED_PROD_BUG"
        if assert_exit_code 4 &&
            assert_stderr_json '.error.api_code' "102" &&
            assert_stderr_not_contains "not found"; then
            test_pass
        fi
    fi
else test_skip "no product-restricted bug"; fi

test_begin "anonymous-product-view-relays-the-server-s-empty-result" "anonymous product view relays the server's empty result"
# Contrast with 146m, and the reason ADR 0015 is about *masking* rather than
# about not-found: Bugzilla answers an anonymous product lookup with
# `{"products":[]}` and HTTP 200 — no error payload at all. Reporting
# not-found there is faithful relaying, and must stay exit 2.
run_bzr_raw --json --server public product view "$RESTRICTED_PRODUCT"
if assert_exit_code 2 && assert_stderr_json '.error.type' "not_found"; then
    test_pass
fi

# ── Structured empty groups on a default-control product ─────────────
# `membercontrol = othercontrol = 2` is CONTROLMAPDEFAULT: without a groups
# key, Bugzilla applies functest-grp; an explicit empty array must override
# that default. This is deliberately distinct from the mandatory-control
# fixture above, where every created bug belongs to the group regardless of
# the payload.
DEFAULT_GROUP_PRODUCT=$(unique_name GroupDefaultProd)
DEFAULT_GROUP_COMPONENT=GroupDefaultComp
_DG_JSON_DIR=$(mktemp -d /tmp/bzr-func-default-groups.XXXXXX)

test_begin "fixture-product-with-a-default-group" "fixture: product with a default group control"
run_bzr product create --name "$DEFAULT_GROUP_PRODUCT" \
    --description "default group control" --version 1.0
if assert_success; then
    run_bzr component create --product "$DEFAULT_GROUP_PRODUCT" \
        --name "$DEFAULT_GROUP_COMPONENT" --description "default group component" \
        --default-assignee "$ADMIN_EMAIL"
    if assert_success; then
        _DG_SQL=$(mktemp /tmp/bzr-func-default-group.XXXXXX.sql)
        cat >"$_DG_SQL" <<SQL
INSERT INTO group_control_map
    (group_id, product_id, entry, membercontrol, othercontrol, canedit,
     editcomponents, editbugs, canconfirm)
SELECT g.id, p.id, 1, 2, 2, 0, 0, 0, 0
FROM groups AS g
JOIN products AS p ON p.name = '${DEFAULT_GROUP_PRODUCT}'
WHERE g.name = 'functest-grp'
ON DUPLICATE KEY UPDATE entry = 1, membercontrol = 2, othercontrol = 2;
SQL
        if run_bugzilla_sql_file "$_DG_SQL"; then test_pass; else
            test_fail "could not apply default group control"
        fi
        rm -f "$_DG_SQL"
        unset _DG_SQL
    fi
fi

write_json_fixture "$_DG_JSON_DIR/groups-omitted.json" \
    "{\"product\":\"$DEFAULT_GROUP_PRODUCT\",\"component\":\"$DEFAULT_GROUP_COMPONENT\",\"summary\":\"default groups omitted\",\"version\":\"1.0\",\"op_sys\":\"Linux\",\"platform\":\"PC\",\"description\":\"d\"}"
test_begin "bug-create-omitted-groups-applies-default-control" "bug create omitting groups applies default control"
run_bzr bug create --from-json "$_DG_JSON_DIR/groups-omitted.json"
if assert_success; then
    DEFAULT_GROUP_OMITTED_BUG=$(jq -r '.id' "$BZR_STDOUT")
    run_bzr bug view "$DEFAULT_GROUP_OMITTED_BUG"
    if assert_success &&
        assert_json '.groups | index("functest-grp") != null' "true"; then test_pass; fi
fi

write_json_fixture "$_DG_JSON_DIR/groups-empty.json" \
    "{\"product\":\"$DEFAULT_GROUP_PRODUCT\",\"component\":\"$DEFAULT_GROUP_COMPONENT\",\"summary\":\"default groups explicit empty\",\"version\":\"1.0\",\"op_sys\":\"Linux\",\"platform\":\"PC\",\"description\":\"d\",\"groups\":[]}"
test_begin "bug-create-explicit-empty-groups-overrides-default-control" "bug create groups [] overrides default control"
run_bzr bug create --from-json "$_DG_JSON_DIR/groups-empty.json"
if assert_success; then
    DEFAULT_GROUP_EMPTY_BUG=$(jq -r '.id' "$BZR_STDOUT")
    run_bzr bug view "$DEFAULT_GROUP_EMPTY_BUG"
    if assert_success && assert_json '.groups | length' "0"; then test_pass; fi
fi

rm -r "$_DG_JSON_DIR"
unset DEFAULT_GROUP_PRODUCT DEFAULT_GROUP_COMPONENT DEFAULT_GROUP_OMITTED_BUG
unset DEFAULT_GROUP_EMPTY_BUG _DG_JSON_DIR

# ── Credentialed error output (issue #505) ───────────────────────────
# The `restricted` server authenticates with `--auth-method query_param`, so
# its key travels in the request URL — the shape that leaks when a deployment
# quotes the URL back in its error text. A stock Bugzilla does not echo it, so
# what this pins is the no-regression direction: the server's own message still
# reaches the user intact on both output paths, and the key appears on neither.
# The redaction itself is asserted by unit tests (`src/error_tests.rs`,
# `src/client/response_tests.rs`), which can synthesize the echo no real server
# produces.
test_begin "credentialed-api-error-keeps-its-message-and-omits-the-key" "credentialed API error keeps its message and omits the key (#505, #509)"
run_bzr_raw --json --server restricted bug view 999999999
if assert_exit_code 4 &&
    assert_stderr_json '.error.api_code' "101" &&
    assert_stderr_json '.error.message | length > 0' "true" &&
    assert_stderr_not_contains "$RESTRICTED_KEY"; then
    # `--output table` explicitly: `run_bzr_raw` redirects stdout to a file, so
    # dropping `--json` alone still resolves to JSON (`resolve_format` falls back
    # to JSON when stdout is not a TTY) and would re-test the path above.
    run_bzr_raw --output table --server restricted bug view 999999999
    if assert_exit_code 4 &&
        assert_stderr_contains "error: Bugzilla API error" &&
        assert_stderr_not_contains "$RESTRICTED_KEY"; then
        run_bzr_raw --progress ndjson --output table --server restricted \
            bug view 999999999
        if assert_exit_code 4 &&
            assert_stderr_contains '"event":"error"' &&
            assert_stderr_contains '"error_type":"api"' &&
            assert_stderr_contains "error: Bugzilla API error" &&
            assert_stderr_not_contains "$RESTRICTED_KEY"; then
            test_pass
        fi
    fi
fi

# ── #715: a policy refusal is not a login failure ────────────────────
UNAVAILABLE_GROUP=$(unique_name unavail-grp)

test_begin "fixture-group-not-enabled-on-the-product" "fixture: group not enabled on the product"
# Deliberately no `group_control_map` row for this group: it exists, and
# FuncTestProd does not permit restricting bugs to it. That is the exact shape
# Bugzilla refuses with `group_restriction_not_allowed`.
run_bzr group create --name "$UNAVAILABLE_GROUP" --description "not enabled on any product"
if assert_success; then test_pass; fi

test_begin "authenticated-policy-refusal-is-not-reported-as-a-login-failure" "authenticated policy refusal is not reported as a login failure"
if [[ -n "$RESTRICTED_BUG" ]]; then
    run_bzr_raw --json bug update "$RESTRICTED_BUG" --groups-add "$UNAVAILABLE_GROUP"
    # The contract #715 broke: the server's own refusal must survive the
    # alternate-auth fallback. Assert the negative too — "must log in" is the
    # wrong answer this issue was filed about.
    if assert_exit_code 4 &&
        assert_stderr_json '.error.type' "api" &&
        assert_stderr_json '.error.api_code' "120" &&
        assert_stderr_not_contains "must log in"; then
        test_pass
    fi
else test_skip "no restricted bug"; fi

test_begin "credentialless-write-is-refused-before-any-request" "credentialless write is refused before any request"
if [[ -n "$RESTRICTED_BUG" ]]; then
    run_bzr_raw --json --server public bug update "$RESTRICTED_BUG" \
        --groups-add "$UNAVAILABLE_GROUP"
    # Measured, not predicted: `bug update` requires credentials and refuses
    # locally (exit 3, type config) before any HTTP request, so there is no
    # server answer on this path and the 401 fallback is never reached. What
    # this pins for #715 is the negative — a credentialless write must keep
    # reporting the local credential precondition, and must not start reporting
    # an api/auth error once the fallback classifies bodies.
    if assert_exit_code 3 &&
        assert_stderr_json '.error.type' "config" &&
        assert_stderr_not_contains "must log in"; then
        test_pass
    fi
else test_skip "no restricted bug"; fi

unset RESTRICTED_USER RESTRICTED_KEY RESTRICTED_GROUP UNAVAILABLE_GROUP
unset RESTRICTED_PRODUCT RESTRICTED_PROD_BUG _RA
unset _RESTRICTED_ALIASES_OK _RESTRICTED_MODE
unset RESTRICTED_DEP_BUG _RESTRICTED_HEADER_OK

echo ""
