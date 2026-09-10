# 01-config
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 1: Config Commands (no network needed)
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 1: Config Commands ────────────────────────────────"

test_begin "config-set-server-test" "config set-server test"
run_bzr config set-server test --url "$BZ_URL" --api-key "$API_KEY" --auth-method query_param --email "$ADMIN_EMAIL"
if assert_success; then test_pass; fi

test_begin "config-show" "config show"
run_bzr config show
if assert_success; then test_pass; fi

test_begin "config-set-server-alt" "config set-server alt"
run_bzr config set-server alt --url "http://localhost:9999" --api-key "fake-key-for-alt-server"
if assert_success; then test_pass; fi

test_begin "config-set-server-auto-detect" "config set-server auto-detect"
run_bzr config set-server auto --url "$BZ_URL" --api-key "$API_KEY" --email "$ADMIN_EMAIL"
if assert_success; then test_pass; fi

test_begin "config-set-server-public-without-credentials" "config set-server public without credentials"
run_bzr config set-server public --url "$BZ_URL"
if assert_success; then
    run_bzr config show
    if assert_json '.servers.public.url' "$BZ_URL" &&
        assert_json '.servers.public.api_key_source' "none"; then test_pass; fi
fi

test_begin "config-set-default-alt" "config set-default alt"
run_bzr config set-default alt
if assert_success; then test_pass; fi

test_begin "config-set-default-test-restore" "config set-default test (restore)"
run_bzr config set-default test
if assert_success; then test_pass; fi

test_begin "config-set-default-nonexistent-expect-failure" "config set-default nonexistent (expect failure)"
run_bzr config set-default nonexistent
if assert_exit_code 3; then test_pass; fi

# ── config server lifecycle (remove/rename) — isolated sandbox so these
#    mutation tests don't disturb the suite's shared servers/default ──
_CFG_MAIN="$XDG_CONFIG_HOME"
_CFG_SBX=$(mktemp -d /tmp/bzr-func-cfgsbx.XXXXXX)
export XDG_CONFIG_HOME="$_CFG_SBX"
run_bzr config set-server keep --url "http://example.invalid:1" --api-key k1
run_bzr config set-server other --url "http://example.invalid:2" --api-key k2
run_bzr config set-default keep

test_begin "config-rename-server-happy" "config rename-server (happy)"
run_bzr config rename-server other other2
if assert_success; then
    run_bzr config show
    if assert_json_exists '.servers.other2' && assert_json '.servers.other' "null"; then test_pass; fi
fi

test_begin "config-remove-server-happy" "config remove-server (happy)"
run_bzr config remove-server other2
if assert_success; then
    run_bzr config show
    if assert_json '.servers.other2' "null"; then test_pass; fi
fi

test_begin "config-remove-server-nonexistent-exit-3" "config remove-server nonexistent (exit 3)"
run_bzr config remove-server ghost
if assert_exit_code 3; then test_pass; fi

run_bzr config set-server companion --url "http://example.invalid:3" --api-key k3
test_begin "config-remove-server-refuses-current-default-exit-3" "config remove-server refuses current default (exit 3)"
run_bzr config remove-server keep
if assert_exit_code 3 && assert_stderr_contains "current default"; then test_pass; fi

test_begin "config-rename-server-name-collision-exit-3" "config rename-server name collision (exit 3)"
run_bzr config rename-server keep companion
if assert_exit_code 3 && assert_stderr_contains "already exists"; then test_pass; fi

test_begin "config-rename-server-updates-default-pointer" "config rename-server updates default pointer"
run_bzr config rename-server keep keep2
if assert_success; then
    run_bzr config show
    if assert_json '.default_server' "keep2"; then test_pass; fi
fi

export XDG_CONFIG_HOME="$_CFG_MAIN"
rm -rf "$_CFG_SBX"
unset _CFG_MAIN _CFG_SBX

# --config reads an alternate file, bypassing XDG_CONFIG_HOME entirely.
test_begin "config-reads-an-alternate-config-file" "--config reads an alternate config file"
_ALT_DIR=$(mktemp -d /tmp/bzr-func-altcfg.XXXXXX)
printf 'default_server = "altsrv"\n[servers.altsrv]\nurl = "http://example.invalid:9"\napi_key = "k"\n' >"$_ALT_DIR/alt.toml"
run_bzr --config "$_ALT_DIR/alt.toml" config show
if assert_success && assert_json '.default_server' "altsrv" &&
    assert_json '.servers.altsrv.url' "http://example.invalid:9"; then test_pass; fi
rm -rf "$_ALT_DIR"
unset _ALT_DIR

# config show provenance display (ADR-0069): the three auth-method-source states
# plus the credentialless path. Self-contained (no network) — writes --config files.
_PROV_DIR=$(mktemp -d /tmp/bzr-func-altcfg.XXXXXX)

test_begin "config-show-auth-source-detected" "config show: detected provenance"
printf 'default_server = "det"\n[servers.det]\nurl = "http://example.invalid:1"\napi_key = "k"\nauth_method = "header"\nauth_method_source = "differential-probe"\n' >"$_PROV_DIR/det.toml"
run_bzr --config "$_PROV_DIR/det.toml" config show
if assert_success && assert_json '.servers.det.auth_method_source' "detected"; then
    run_bzr_raw --config "$_PROV_DIR/det.toml" config show --output table
    if assert_success && assert_stdout_contains "Auth Source"; then test_pass; fi
fi

test_begin "config-show-auth-source-pinned" "config show: pinned provenance"
printf 'default_server = "pin"\n[servers.pin]\nurl = "http://example.invalid:2"\napi_key = "k"\nauth_method = "header"\nauth_method_source = "pinned"\n' >"$_PROV_DIR/pin.toml"
run_bzr --config "$_PROV_DIR/pin.toml" config show
if assert_success && assert_json '.servers.pin.auth_method_source' "pinned"; then test_pass; fi

test_begin "config-show-auth-source-unstamped" "config show: unstamped provenance"
printf 'default_server = "uns"\n[servers.uns]\nurl = "http://example.invalid:3"\napi_key = "k"\nauth_method = "header"\n' >"$_PROV_DIR/uns.toml"
run_bzr --config "$_PROV_DIR/uns.toml" config show
if assert_success && assert_json '.servers.uns.auth_method_source' "unstamped"; then test_pass; fi

test_begin "config-show-auth-source-credentialless-none" "config show: credentialless has no provenance"
printf 'default_server = "cred"\n[servers.cred]\nurl = "http://example.invalid:4"\n' >"$_PROV_DIR/cred.toml"
run_bzr --config "$_PROV_DIR/cred.toml" config show
if assert_success && assert_json '.servers.cred.auth_method' "null" &&
    assert_json '.servers.cred.auth_method_source' "null"; then test_pass; fi

rm -rf "$_PROV_DIR"
unset _PROV_DIR

echo ""
