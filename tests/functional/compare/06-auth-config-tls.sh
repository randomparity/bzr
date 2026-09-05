#!/bin/bash
# Authentication, bugzillarc, and TLS comparison contracts.
r11_auth_evidence_is() {
    local expected="$1" log="$2" line count=0
    while IFS= read -r line; do
        [[ $line != *"$BZR_COMPARE_API_KEY"* ]] || return 1
        if [[ $line == auth-kind\ * ]]; then
            [[ $line == "auth-kind $expected count=1" ]] || return 1
            count=$((count + 1))
        fi
    done <"$log"
    [[ $count -gt 0 ]]
}
r11_adapter_result_is() {
    local operation="$1" input="$2" output="$3" filter="$4"
    run_pybz_adapter "$operation" "/work/compare/${input##*/}" \
        "/work/compare/${output##*/}"
    [[ $BZR_EXIT -eq 0 && -r $output ]] && jq -e "$filter" "$output" >/dev/null
}
r11_expected_bzr_auth_kind() {
    case "$1" in
    bz50 | bz52) printf 'query\n' ;;
    bz53) printf 'header\n' ;;
    *) return 2 ;;
    esac
}
declare -F r11_api_key_control >/dev/null || r11_api_key_control() {
    local expected request output host_ok=0 pybz_ok=0
    expected=$(r11_expected_bzr_auth_kind "$BZ_VERSION") || return 1
    BZR_FUNC_REDHAT_MODE=bearer-auth redhat_shape_start "$BZ_PORT" || return 1
    run_bzr config set-server r11-auth --url "http://127.0.0.1:${REDHAT_SHAPE_PORT}" \
        --api-key-env BZR_COMPARE_API_KEY --email "$COMPARE_ADMIN_EMAIL"
    if [[ $BZR_EXIT -eq 0 ]]; then
        run_bzr --server r11-auth whoami
    fi
    redhat_shape_stop || return 1
    BZR_FUNC_REDHAT_MODE=bearer-auth redhat_shape_start "$BZ_PORT" || return 1
    run_bzr --server r11-auth whoami
    if [[ $BZR_EXIT -eq 0 ]] && r11_auth_evidence_is "$expected" "$REDHAT_SHAPE_LOG"; then
        host_ok=1
    else
        printf 'r11 API-key host evidence expected %s; exit=%s\n' "$expected" "$BZR_EXIT" >&2
        awk '/^auth-kind / { print "host " $0 }' "$REDHAT_SHAPE_LOG" >&2
    fi
    redhat_shape_stop || return 1
    pybz_proxy_start redhat 18080 >/dev/null || return 1
    request=$(pybz_write_api_key_identity_request r11-api-key \
        'http://127.0.0.1:18080' "$BZR_COMPARE_API_KEY" "$COMPARE_ADMIN_EMAIL") || return 1
    output="$COMPARE_EXCHANGE_DIR/r11-api-key.pybz.output.json"
    if r11_adapter_result_is api_key_identity "$request" "$output" \
        '.transport == "REST" and .result.authenticated and .result.identity_matched' &&
        r11_auth_evidence_is query "$COMPARE_EXCHANGE_DIR/redhat.proxy.log"; then
        pybz_ok=1
    else
        printf 'r11 API-key python-bugzilla evidence expected query; exit=%s\n' "$BZR_EXIT" >&2
        awk '/^auth-kind / { print "pybz " $0 }' \
            "$COMPARE_EXCHANGE_DIR/redhat.proxy.log" >&2
    fi
    pybz_proxy_stop redhat || return 1
    [[ $host_ok -eq 1 && $pybz_ok -eq 1 ]]
}
r11_write_auth_request() {
    local operation="$1" output="$COMPARE_EXCHANGE_DIR/r11-${1}.input.json"
    [[ $COMPARE_ADMIN_EMAIL =~ ^[A-Za-z0-9@._+-]+$ &&
        $COMPARE_ADMIN_PASSWORD =~ ^[A-Za-z0-9._!+-]+$ ]] || return 2
    case "$operation" in
    login)
        printf '{"url":"http://127.0.0.1","username":"%s","password":"%s","restrict_login":true}\n' \
            "$COMPARE_ADMIN_EMAIL" "$COMPARE_ADMIN_PASSWORD" >"$output"
        ;;
    cached_auth)
        printf '{"url":"http://127.0.0.1","username":"%s"}\n' \
            "$COMPARE_ADMIN_EMAIL" >"$output"
        ;;
    logout) printf '{"url":"http://127.0.0.1"}\n' >"$output" ;;
    *) return 2 ;;
    esac
    chmod 600 "$output"
    printf '%s\n' "$output"
}
r11_auth_control() {
    local operation="$1" filter="$2" input
    input=$(r11_write_auth_request "$operation") || return 1
    r11_adapter_result_is "$operation" "$input" \
        "$COMPARE_EXCHANGE_DIR/r11-${operation}.output.json" "$filter"
}
declare -F r11_login_control >/dev/null || r11_login_control() {
    rm -f "$COMPARE_EXCHANGE_DIR/python-bugzilla-token"
    r11_auth_control login \
        '.transport == "REST" and .result == {authenticated:true,cache_written:true,restricted:true}'
}
declare -F r11_cached_control >/dev/null || r11_cached_control() {
    r11_auth_control cached_auth \
        '.transport == "REST" and .result == {authenticated:true,cache_used:true}'
}
declare -F r11_logout_control >/dev/null || r11_logout_control() {
    r11_auth_control logout \
        '.transport == "REST" and .result == {cache_cleared:true,logged_out:true}'
}
declare -F r11_bugzillarc_control >/dev/null || r11_bugzillarc_control() {
    local aspect="$1" sidecar output="$COMPARE_EXCHANGE_DIR/r11-rc.json"
    sidecar=$(pybz_sidecar_name) || return 1
    printf '[DEFAULT]\nurl=http://system.invalid\n[127.0.0.1]\nuser=system\n' \
        >"$COMPARE_EXCHANGE_DIR/r11-system.rc"
    printf '[DEFAULT]\nurl=http://home.invalid\n[127.0.0.1]\nuser=home\n' \
        >"$COMPARE_EXCHANGE_DIR/r11-home.rc"
    printf '[DEFAULT]\nurl=http://127.0.0.1\n[127.0.0.1]\nuser=config\n[fixture.invalid/rest]\nuser=substring\n' \
        >"$COMPARE_EXCHANGE_DIR/r11-config.rc"
    chmod 600 "$COMPARE_EXCHANGE_DIR"/r11-*.rc
    # shellcheck disable=SC2016 # HOME expands in the sidecar shell.
    "$PYBZ_RUNTIME" exec "$sidecar" sh -c \
        'mkdir -p "$HOME/.config/python-bugzilla"; cp /work/compare/r11-system.rc /etc/bugzillarc; cp /work/compare/r11-home.rc "$HOME/.bugzillarc"; cp /work/compare/r11-config.rc "$HOME/.config/python-bugzilla/bugzillarc"'
    "$PYBZ_RUNTIME" exec "$sidecar" python -c \
        'import json; from bugzilla import Bugzilla; b=Bugzilla(None); print(json.dumps({"url":b.get_rcfile_default_url(),"precedence":b._rcfile.parse("http://127.0.0.1").get("user"),"substring":b._rcfile.parse("http://fixture.invalid/rest/bug").get("user")}))' \
        >"$output"
    case "$aspect" in
    precedence) jq -e '.precedence == "config"' "$output" >/dev/null ;;
    default) jq -e '.url == "http://127.0.0.1"' "$output" >/dev/null ;;
    substring) jq -e '.substring == "substring"' "$output" >/dev/null ;;
    *) return 2 ;;
    esac
}
declare -F r11_tls_control >/dev/null || r11_tls_control() {
    local cert_dir tls_url host_ok=0 pybz_ok=0
    tls_fixture_start "$BZ_PORT" || return 1
    tls_url="https://127.0.0.1:${TLS_PORT}"
    run_bzr_raw --json --server-url "$tls_url" server info </dev/null
    [[ $BZR_EXIT -ne 0 ]] || return 1
    run_bzr_raw --json --server-url "$tls_url" --server-tls-insecure server info </dev/null
    [[ $BZR_EXIT -eq 0 ]] && host_ok=1
    cert_dir=${TLS_FIXTURE_DIR#"$FUNC_CONFIG_DIR"/}
    pybz_proxy_start tls 18443 "$cert_dir" >/dev/null || return 1
    run_pybz --bugzilla https://127.0.0.1:18443 info --products
    [[ $BZR_EXIT -ne 0 ]] || return 1
    run_pybz --nosslverify --bugzilla https://127.0.0.1:18443 info --products
    [[ $BZR_EXIT -eq 0 ]] && pybz_ok=1
    pybz_proxy_stop tls || return 1
    _tls_cleanup
    [[ $host_ok -eq 1 && $pybz_ok -eq 1 ]]
}
declare -F r11_certificate_control >/dev/null || r11_certificate_control() {
    local input="$COMPARE_EXCHANGE_DIR/r11-cert.input.json"
    local output="$COMPARE_EXCHANGE_DIR/r11-cert.output.json"
    printf 'comparison certificate surface\n' >"$COMPARE_EXCHANGE_DIR/r11-client-cert.pem"
    printf '{"certificate":"/work/compare/r11-client-cert.pem"}\n' >"$input"
    chmod 600 "$COMPARE_EXCHANGE_DIR/r11-client-cert.pem" "$input"
    r11_adapter_result_is client_certificate_surface "$input" "$output" \
        '.transport == null and .result.configured == true'
}
declare -F r11_bearer_control >/dev/null || r11_bearer_control() {
    local request output="$COMPARE_EXCHANGE_DIR/r11-bearer.pybz.output.json" ok=0
    pybz_redhat_alias_install || return 1
    pybz_proxy_start redhat 18082 >/dev/null || return 1
    request=$(pybz_write_api_key_identity_request r11-bearer \
        'http://bugzilla.redhat.com:18082' "$BZR_COMPARE_API_KEY" \
        "$COMPARE_ADMIN_EMAIL") || return 1
    if r11_adapter_result_is api_key_identity "$request" "$output" \
        '.transport == "REST" and .result.authenticated and .result.identity_matched' &&
        r11_auth_evidence_is bearer "$COMPARE_EXCHANGE_DIR/redhat.proxy.log"; then ok=1; fi
    pybz_proxy_stop redhat || return 1
    [[ $ok -eq 1 ]]
}
declare -F r11_parser_gap >/dev/null || r11_parser_gap() {
    local kind="$1"
    case "$kind" in
    token | bearer)
        run_bzr config set-server "r11-${kind}-gap" --url "$BZ_URL" \
            --api-key-env BZR_COMPARE_API_KEY --auth-method "$kind"
        [[ $BZR_EXIT -eq 2 ]] && grep -Fxq \
            "error: invalid value '$kind' for '--auth-method <AUTH_METHOD>': invalid auth method '$kind': expected 'header', 'query_param', or 'query-param'" "$BZR_STDERR"
        ;;
    login)
        run_bzr auth login
        [[ $BZR_EXIT -eq 2 ]] && grep -Fxq "error: unrecognized subcommand 'auth'" "$BZR_STDERR"
        ;;
    bugzillarc)
        run_bzr config import-bugzillarc
        [[ $BZR_EXIT -eq 2 ]] && grep -Fxq "error: unrecognized subcommand 'import-bugzillarc'" "$BZR_STDERR"
        ;;
    certificate)
        run_bzr --server-url "$BZ_URL" --server-tls-client-cert fixture.pem server info
        [[ $BZR_EXIT -eq 2 ]] && grep -Fxq \
            "error: unexpected argument '--server-tls-client-cert' found" "$BZR_STDERR"
        ;;
    *) return 2 ;;
    esac
}
r11_pass_test() { if "$@"; then test_pass; else test_fail "positive control failed"; fi; }
r11_gap_test() {
    local issue="$1" gap="$2"
    shift 2
    if ! "$@"; then
        test_fail "positive control failed"
        return
    fi
    if r11_parser_gap "$gap"; then
        test_fail "controlled bzr surface is absent"
        expect_gap "$issue"
    else
        test_fail "exact bzr absence did not match"
    fi
}
r11_token_control() { r11_login_control && r11_cached_control; }
test_begin "api-key-placement" "API-key placement by server version"
r11_pass_test r11_api_key_control
test_begin "restricted-login" "restricted password login"
r11_pass_test r11_login_control
test_begin "cached-token" "cached login token reuse"
r11_pass_test r11_cached_control
test_begin "logout" "logout token invalidation"
r11_pass_test r11_logout_control
test_begin "bugzillarc-precedence" "three-file bugzillarc precedence"
r11_pass_test r11_bugzillarc_control precedence
test_begin "bugzillarc-default-url" "bugzillarc default URL"
r11_pass_test r11_bugzillarc_control default
test_begin "bugzillarc-substring-section" "bugzillarc URL-substring section"
r11_pass_test r11_bugzillarc_control substring
test_begin "nosslverify" "disable TLS verification"
r11_pass_test r11_tls_control
test_begin "token-transport-gap" "login-token request transport"
r11_gap_test 676 token r11_token_control
test_begin "login-command-gap" "login and logout commands"
r11_gap_test 681 login r11_login_control
test_begin "bugzillarc-import-gap" "bugzillarc import"
r11_gap_test 682 bugzillarc r11_bugzillarc_control precedence
test_begin "client-certificate-surface-gap" "client certificate configuration"
r11_gap_test 677 certificate r11_certificate_control
test_begin "bearer-gap" "Red Hat Bearer API-key transport"
r11_gap_test 678 bearer r11_bearer_control
