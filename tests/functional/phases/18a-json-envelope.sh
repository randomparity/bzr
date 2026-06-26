# 18a-json-envelope
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# Reads: BZ_URL. Creates: none. Exercises the --json schema_version envelope
# (#464) against the real server, including the credentialless read path.
# shellcheck shell=bash
# shellcheck disable=SC2016

# ══════════════════════════════════════════════════════════════════════
# Phase 18a: JSON schema_version envelope
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 18a: JSON schema_version envelope ─────────────────"

# Keep in lockstep with output::SCHEMA_VERSION.
SCHEMA_VERSION="0.6.0"

# bzr schema --json reports the contract version and wraps the names in `data`.
test_begin "120a. schema --json reports schema_version"
run_bzr schema
if assert_success &&
    assert_raw_json '.schema_version' "$SCHEMA_VERSION" &&
    assert_raw_json '.data | type' "array" &&
    assert_schema_list_contains "envelope"; then test_pass; fi

# An authenticated read wraps its payload in the versioned envelope.
test_begin "120b. read output is enveloped"
run_bzr bug list --limit 1
if assert_success &&
    assert_raw_json '.schema_version' "$SCHEMA_VERSION" &&
    assert_raw_json '.data | type' "array"; then test_pass; fi

# Credentialless read (inline URL, no API key) is enveloped too.
test_begin "120c. credentialless read is enveloped"
run_bzr_raw --json --server-url "$BZ_URL" server info
if assert_success &&
    assert_raw_json '.schema_version' "$SCHEMA_VERSION" &&
    assert_raw_json '.data.version != null' "true"; then test_pass; fi

# --output ndjson stays bare: no top-level schema_version envelope.
test_begin "120d. ndjson output is not enveloped"
run_bzr_raw --output ndjson --server-url "$BZ_URL" server info
if assert_success &&
    assert_raw_json 'has("schema_version")' "false"; then test_pass; fi

echo ""
