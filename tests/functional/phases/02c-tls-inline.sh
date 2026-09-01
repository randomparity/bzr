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

# Tear down now and restore the runner's plain cleanup trap for later phases.
_tls_cleanup
trap cleanup EXIT
echo ""
