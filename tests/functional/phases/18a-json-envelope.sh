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
SCHEMA_VERSION="3.0.1"

# bzr schema --json reports the contract version and wraps the names in `data`.
test_begin "schema-json-reports-schema-version" "schema --json reports schema_version"
run_bzr schema
if assert_success &&
    assert_raw_json '.schema_version' "$SCHEMA_VERSION" &&
    assert_raw_json '.data | type' "array" &&
    assert_schema_list_contains "envelope"; then test_pass; fi

# An authenticated read wraps its payload in the versioned envelope.
test_begin "read-output-is-enveloped" "read output is enveloped"
run_bzr bug list --limit 1
if assert_success &&
    assert_raw_json '.schema_version' "$SCHEMA_VERSION" &&
    assert_raw_json '.data | type' "array"; then test_pass; fi

# Credentialless read (inline URL, no API key) is enveloped too.
test_begin "credentialless-read-is-enveloped" "credentialless read is enveloped"
run_bzr_raw --json --server-url "$BZ_URL" server info
if assert_success &&
    assert_raw_json '.schema_version' "$SCHEMA_VERSION" &&
    assert_raw_json '.data.version != null' "true"; then test_pass; fi

# --output ndjson stays bare: no top-level schema_version envelope.
test_begin "ndjson-output-is-not-enveloped" "ndjson output is not enveloped"
run_bzr_raw --output ndjson --server-url "$BZ_URL" server info
if assert_success &&
    assert_raw_json 'has("schema_version")' "false"; then test_pass; fi

# --json failure: structured error object on STDERR carries the universal keys
# plus the schema_version envelope (the date filter is validated client-side, so
# this is deterministic and does not depend on server data).
test_begin "json-error-body-is-structured-on-stderr" "--json error body is structured on stderr (#482)"
run_bzr bug list --created-since notadate
if assert_exit_code 7 &&
    assert_stderr_json '.schema_version' "$SCHEMA_VERSION" &&
    assert_stderr_json '.error.type' "input" &&
    assert_stderr_json '.error.exit_code' "7"; then test_pass; fi

# Input-validation errors carry field/value attribution.
test_begin "input-error-names-field-and-value" "input error names field and value"
run_bzr bug list --created-since notadate
if assert_stderr_json '.error.field' "--created-since" &&
    assert_stderr_json '.error.value' "notadate"; then test_pass; fi

# Uniform-projection verbs attribute an unknown --fields token to flag + value.
test_begin "unknown-fields-token-carries-field-value" "unknown --fields token carries field/value"
run_bzr comment list 1 --fields bogus
if assert_exit_code 7 &&
    assert_stderr_json '.error.field' "--fields" &&
    assert_stderr_json '.error.value' "bogus"; then test_pass; fi

# Server-side API faults carry api_code (credentialless not-found read path).
test_begin "server-api-error-carries-api-code" "server API error carries api_code"
run_bzr_raw --json --server-url "$BZ_URL" bug view 999999999
if assert_exit_code 4 &&
    assert_stderr_json '.error.type' "api" &&
    assert_stderr_json '.error.api_code' "101"; then test_pass; fi

# --output ndjson failure: error stays a bare structured object (no envelope).
test_begin "ndjson-error-is-bare-structured-object-on-stderr" "ndjson error is bare structured object on stderr"
run_bzr_raw --output ndjson bug list --created-since notadate
if assert_exit_code 7 &&
    assert_stderr_json '.error.type' "input" &&
    assert_stderr_json 'has("schema_version")' "false"; then test_pass; fi

echo ""
