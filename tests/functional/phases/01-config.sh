# 01-config
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 1: Config Commands (no network needed)
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 1: Config Commands ────────────────────────────────"

test_begin "1. config set-server test"
run_bzr config set-server test --url "$BZ_URL" --api-key "$API_KEY" --auth-method query_param --email "$ADMIN_EMAIL"
if assert_success; then test_pass; fi

test_begin "2. config show"
run_bzr config show
if assert_success; then test_pass; fi

test_begin "3. config set-server alt"
run_bzr config set-server alt --url "http://localhost:9999" --api-key "fake-key-for-alt-server"
if assert_success; then test_pass; fi

test_begin "3a. config set-server auto-detect"
run_bzr config set-server auto --url "$BZ_URL" --api-key "$API_KEY" --email "$ADMIN_EMAIL"
if assert_success; then test_pass; fi

test_begin "4. config set-default alt"
run_bzr config set-default alt
if assert_success; then test_pass; fi

test_begin "5. config set-default test (restore)"
run_bzr config set-default test
if assert_success; then test_pass; fi

test_begin "6. config set-default nonexistent (expect failure)"
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

test_begin "6a. config rename-server (happy)"
run_bzr config rename-server other other2
if assert_success; then
    run_bzr config show
    if assert_json_exists '.servers.other2' && assert_json '.servers.other' "null"; then test_pass; fi
fi

test_begin "6b. config remove-server (happy)"
run_bzr config remove-server other2
if assert_success; then
    run_bzr config show
    if assert_json '.servers.other2' "null"; then test_pass; fi
fi

test_begin "6c. config remove-server nonexistent (exit 3)"
run_bzr config remove-server ghost
if assert_exit_code 3; then test_pass; fi

run_bzr config set-server companion --url "http://example.invalid:3" --api-key k3
test_begin "6d. config remove-server refuses current default (exit 3)"
run_bzr config remove-server keep
if assert_exit_code 3 && assert_stderr_contains "current default"; then test_pass; fi

test_begin "6e. config rename-server name collision (exit 3)"
run_bzr config rename-server keep companion
if assert_exit_code 3 && assert_stderr_contains "already exists"; then test_pass; fi

test_begin "6f. config rename-server updates default pointer"
run_bzr config rename-server keep keep2
if assert_success; then
    run_bzr config show
    if assert_json '.default_server' "keep2"; then test_pass; fi
fi

export XDG_CONFIG_HOME="$_CFG_MAIN"
rm -rf "$_CFG_SBX"
unset _CFG_MAIN _CFG_SBX

# --config reads an alternate file, bypassing XDG_CONFIG_HOME entirely.
test_begin "6g. --config reads an alternate config file"
_ALT_DIR=$(mktemp -d /tmp/bzr-func-altcfg.XXXXXX)
printf 'default_server = "altsrv"\n[servers.altsrv]\nurl = "http://example.invalid:9"\napi_key = "k"\n' >"$_ALT_DIR/alt.toml"
run_bzr --config "$_ALT_DIR/alt.toml" config show
if assert_success && assert_json '.default_server' "altsrv" &&
    assert_json '.servers.altsrv.url' "http://example.invalid:9"; then test_pass; fi
rm -rf "$_ALT_DIR"
unset _ALT_DIR

echo ""

