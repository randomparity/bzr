# 02c-tls-inline
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 2c: Ad-hoc TLS trust flags (HTTPS fixture, issue #406)
# ══════════════════════════════════════════════════════════════════════
# Exercise the stateless --server-tls-* trust controls against a real Bugzilla
# server fronted by a TLS-terminating reverse proxy (tls-proxy.py). The
# clap-level mutual-exclusion and require---server-url checks live in
# 17b-arg-validation.sh (tests 125b/125c) and are not duplicated here.
echo "── Phase 2c: Ad-hoc TLS trust flags ────────────────────────"

if ! tls_tools_available; then
    test_begin "https-fixture-tools-unavailable" "ad-hoc TLS HTTPS fixture"
    test_skip "requires python3, openssl, and curl"
    echo ""
    return 0
fi

if ! tls_fixture_start "$BZ_PORT"; then
    test_begin "https-fixture-start-failed" "ad-hoc TLS HTTPS fixture"
    test_fail "TLS reverse proxy did not become ready"
    echo ""
    return 0
fi

# Compose the proxy/cert teardown onto the runner's existing EXIT trap so an
# early exit still stops the proxy. _tls_cleanup is idempotent.
trap 'cleanup; _tls_cleanup' EXIT

TLS_URL="https://127.0.0.1:${TLS_PORT}"

# Snapshot the isolated config to prove no ad-hoc TLS option persists state.
_tls_cfg="${XDG_CONFIG_HOME}/bzr/config.toml"
_tls_cfg_existed=0
if [[ -f "$_tls_cfg" ]]; then
    _tls_cfg_existed=1
    cp "$_tls_cfg" "${TLS_FIXTURE_DIR}/config.snapshot"
fi

# stdin is redirected from /dev/null on each call so the non-interactive
# TOFU/rotation prompts never block when the suite is run from a terminal.

test_begin "server-tls-insecure-accepts-self-signed-cert" "--server-tls-insecure accepts self-signed cert"
run_bzr_raw --json --server-url "$TLS_URL" --server-tls-insecure server info </dev/null
if assert_success && assert_json_exists '.version'; then test_pass; fi

test_begin "server-tls-ca-cert-trusts-the-provided-ca" "--server-tls-ca-cert trusts the provided CA"
run_bzr_raw --json --server-url "$TLS_URL" --server-tls-ca-cert "$TLS_CA_CERT" server info </dev/null
if assert_success && assert_json_exists '.version'; then test_pass; fi

test_begin "server-tls-ca-cert-verifies-localhost-dns-name" "--server-tls-ca-cert verifies a DNS name"
run_bzr_raw --json --server-url "https://localhost:${TLS_PORT}" \
    --server-tls-ca-cert "$TLS_CA_CERT" server info </dev/null
if assert_success && assert_json_exists '.version'; then test_pass; fi

test_begin "server-tls-pin-sha256-accepts-the-correct-pin" "--server-tls-pin-sha256 accepts the correct pin"
run_bzr_raw --json --server-url "$TLS_URL" --server-tls-pin-sha256 "$TLS_GOOD_PIN" server info </dev/null
if assert_success && assert_json_exists '.version'; then test_pass; fi

test_begin "server-tls-pin-sha256-rejects-a-wrong-pin" "--server-tls-pin-sha256 rejects a wrong pin"
run_bzr_raw --json --server-url "$TLS_URL" \
    --server-tls-pin-sha256 "sha256//AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=" \
    server info </dev/null
# Contract: a wrong pin is rejected — non-zero exit and no version payload. The
# exact code is currently 5 (transport error), not the issue's stated 13; see
# the design spec. We assert rejection without hard-coding the number. On
# rejection bzr writes the error to stderr and leaves stdout empty, so the
# version field must be absent.
if assert_failure; then
    if [[ -z "$(jq -r '.version // empty' "$BZR_STDOUT" 2>/dev/null)" ]]; then
        test_pass
    else
        test_fail "wrong pin unexpectedly returned a version payload"
    fi
fi

test_begin "server-tls-pin-now-pins-the-presented-cert-for-the-run" "--server-tls-pin-now pins the presented cert for the run"
run_bzr_raw --json --server-url "$TLS_URL" --server-tls-pin-now server info </dev/null
if assert_success && assert_json_exists '.version'; then test_pass; fi

test_begin "ad-hoc-tls-options-do-not-persist-config" "ad-hoc TLS options do not persist config"
_tls_cfg_ok=1
if [[ "$_tls_cfg_existed" -eq 1 ]]; then
    if [[ ! -f "$_tls_cfg" ]] || ! cmp -s "${TLS_FIXTURE_DIR}/config.snapshot" "$_tls_cfg"; then
        _tls_cfg_ok=0
    fi
elif [[ -f "$_tls_cfg" ]]; then
    _tls_cfg_ok=0
fi
if [[ "$_tls_cfg_ok" -eq 1 ]]; then
    test_pass
else
    test_fail "config.toml changed after ad-hoc TLS runs"
fi

test_begin "untrusted-cert-hint-keeps-its-lines" "the TLS trust hint renders multi-line, not escaped \\n"
# With no trust flag and no interactive answer, the TOFU prompt reaches EOF and
# bzr fails with the "TLS certificate not trusted" error. Its body is a
# bzr-authored, indented, multi-line remediation hint that reaches the terminal
# through main.rs's `error: …` rendering — the one place terminal-control
# escaping is applied to a whole BzrError display. Escaping it in one call turns
# every newline into a literal \n and collapses the hint to one line (ADR 0070),
# so this asserts the line structure a real untrusted cert produces, which no
# wiremock fixture can reach.
run_bzr_raw --output table --server-url "$TLS_URL" server info </dev/null
if assert_failure &&
    assert_stderr_contains 'TLS certificate not trusted' &&
    assert_stderr_contains '^  bzr config set-server <NAME> --tls-insecure$' &&
    assert_stderr_contains '^  bzr config set-server <NAME> --tls-pin-sha256 <PIN>$' &&
    assert_stderr_not_contains '\\n'; then
    test_pass
fi

# Use a separate saved config so these replacement tests cannot alter later phases.
_tls_ack_config="${TLS_FIXTURE_DIR}/ack.toml"
test_begin "saved-tls-default-warning" "saved insecure server warns without acknowledgment"
run_bzr --config "$_tls_ack_config" config set-server ack --url "$TLS_URL" --tls-insecure
if assert_success; then
    run_bzr --config "$_tls_ack_config" --server ack server info </dev/null
    if assert_success && assert_json_exists '.version' &&
        assert_stderr_contains 'TLS certificate verification disabled'; then
        cp "$BZR_STDOUT" "${TLS_FIXTURE_DIR}/default-output.json"
        test_pass
    fi
fi

test_begin "saved-tls-acknowledged-warning" "acknowledgment suppresses only the TLS warning"
run_bzr --config "$_tls_ack_config" config set-server ack --url "$TLS_URL" \
    --tls-insecure --tls-insecure-acknowledged
if assert_success; then
    run_bzr --config "$_tls_ack_config" --server ack -vv server info </dev/null
    if assert_success && assert_stderr_not_contains 'TLS certificate verification disabled' &&
        assert_stderr_contains 'created Bugzilla client'; then
        if cmp -s "$BZR_STDOUT" "${TLS_FIXTURE_DIR}/default-output.json"; then
            test_pass
        else
            test_fail "acknowledgment changed server info stdout"
        fi
    fi
fi

test_begin "saved-tls-acknowledgment-isolation" "another named server still warns"
run_bzr --config "$_tls_ack_config" config set-server other --url "$TLS_URL" --tls-insecure
if assert_success; then
    run_bzr --config "$_tls_ack_config" --server other server info </dev/null
    if assert_success && assert_stderr_contains 'TLS certificate verification disabled'; then test_pass; fi
fi

test_begin "inline-tls-cannot-inherit-acknowledgment" "inline server still warns with acknowledged saved config"
run_bzr --config "$_tls_ack_config" --server-url "$TLS_URL" --server-tls-insecure server info </dev/null
if assert_success && assert_stderr_contains 'TLS certificate verification disabled'; then test_pass; fi

test_begin "saved-tls-acknowledgment-revocation" "replacement without acknowledgment restores warning"
run_bzr --config "$_tls_ack_config" config set-server ack --url "$TLS_URL" --tls-insecure
if assert_success; then
    run_bzr --config "$_tls_ack_config" --server ack server info </dev/null
    if assert_success && assert_stderr_contains 'TLS certificate verification disabled'; then test_pass; fi
fi

test_begin "credentialed-tls-acknowledgment" "credentialed HTTPS preserves diagnostics and stdout"
run_bzr --config "$_tls_ack_config" config set-server ack --url "$TLS_URL" \
    --api-key "$API_KEY" --email "$ADMIN_EMAIL" --tls-insecure --tls-insecure-acknowledged
if assert_success; then
    run_bzr --config "$_tls_ack_config" --server ack -vv server info </dev/null
    if assert_success && assert_json_exists '.version' &&
        assert_stderr_not_contains 'TLS certificate verification disabled' &&
        assert_stderr_contains 'created Bugzilla client'; then test_pass; fi
fi

# Keep the chain trusted while changing only the leaf's DNS identity. Connecting
# to localhost requires no extra DNS alias and keeps resolution out of cargo tests.
_tls_cleanup
test_begin "server-tls-ca-cert-rejects-wrong-dns-name" "--server-tls-ca-cert rejects a wrong DNS name"
if tls_fixture_start "$BZ_PORT" "wrong-name.invalid"; then
    run_bzr_raw --output table --server-url "https://localhost:${TLS_PORT}" \
        --server-tls-ca-cert "$TLS_CA_CERT" server info </dev/null
    if assert_failure &&
        assert_stderr_contains 'certificate not valid for name' &&
        assert_stderr_contains 'localhost'; then
        test_pass
    fi
else
    test_fail "wrong-name TLS reverse proxy did not become ready"
fi

# Tear down now and restore the runner's plain cleanup trap for later phases.
_tls_cleanup
trap cleanup EXIT
echo ""
