# 18-completion-schema
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# Reads: none. Creates: none. Local commands — no Bugzilla server needed.
# shellcheck shell=bash
# shellcheck disable=SC2016

# ══════════════════════════════════════════════════════════════════════
# Phase 18: Completion & Schema (local commands, no network)
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 18: Completion & Schema ───────────────────────────"

# completion: each shell emits a non-empty, shell-appropriate script.
test_begin "completion-bash" "completion bash"
run_bzr_raw completion bash
if assert_success && assert_stdout_contains "_bzr()"; then test_pass; fi

test_begin "completion-zsh" "completion zsh"
run_bzr_raw completion zsh
if assert_success && assert_stdout_contains "#compdef bzr"; then test_pass; fi

test_begin "completion-fish" "completion fish"
run_bzr_raw completion fish
if assert_success && assert_stdout_contains "complete -c bzr"; then test_pass; fi

test_begin "completion-powershell" "completion powershell"
run_bzr_raw completion powershell
if assert_success && assert_stdout_contains "Register-ArgumentCompleter"; then test_pass; fi

test_begin "completion-elvish" "completion elvish"
run_bzr_raw completion elvish
if assert_success && assert_stdout_contains "edit:completion"; then test_pass; fi

test_begin "completion-invalid-shell-clap-rejects-exit-2" "completion invalid shell (clap rejects, exit 2)"
run_bzr_raw completion notashell
if assert_exit_code 2 && assert_stderr_contains "invalid value"; then test_pass; fi

# schema: list mode + per-name schema is valid JSON (draft 2020-12).
test_begin "schema-list" "schema (list)"
run_bzr schema
if assert_success && assert_json_valid &&
    assert_schema_list_contains "bug" &&
    assert_schema_list_contains "bug-create-input" &&
    assert_schema_list_contains "bug-update-input" &&
    assert_schema_list_contains "product-create-input" &&
    assert_schema_list_contains "product-update-input" &&
    assert_schema_list_contains "component-create-input" &&
    assert_stdout_not_contains "component-update-input" &&
    assert_schema_list_contains "user-create-input" &&
    assert_schema_list_contains "user-update-input" &&
    assert_schema_list_contains "group-create-input" &&
    assert_schema_list_contains "group-update-input" &&
    assert_schema_list_contains "server-capabilities" &&
    assert_schema_list_contains "error"; then test_pass; fi

test_begin "schema-bug-valid-draft-2020-12-schema" "schema bug (valid draft 2020-12 schema)"
run_bzr schema bug
if assert_success && assert_json_valid &&
    assert_json '.["$schema"]' "https://json-schema.org/draft/2020-12/schema"; then test_pass; fi

test_begin "schema-bug-update-input-valid-draft-2020-12-schema" "schema bug-update-input (valid draft 2020-12 schema)"
run_bzr schema bug-update-input
if assert_success && assert_json_valid &&
    assert_json '.["$schema"]' "https://json-schema.org/draft/2020-12/schema" &&
    assert_json '.["$defs"].bugUpdateInputObject.additionalProperties' "false"; then test_pass; fi

test_begin "schema-product-create-input" "schema product-create-input"
run_bzr schema product-create-input
if assert_success && assert_json_valid &&
    assert_json '.["$schema"]' "https://json-schema.org/draft/2020-12/schema"; then test_pass; fi

test_begin "schema-component-update-input-removed" "removed component update schema"
run_bzr schema component-update-input
if assert_exit_code 7 && assert_stderr_contains "unknown schema"; then test_pass; fi

test_begin "schema-user-update-input" "schema user-update-input"
run_bzr schema user-update-input
if assert_success && assert_json_valid &&
    assert_json '.["$schema"]' "https://json-schema.org/draft/2020-12/schema"; then test_pass; fi

test_begin "schema-group-update-input" "schema group-update-input"
run_bzr schema group-update-input
if assert_success && assert_json_valid &&
    assert_json '.["$schema"]' "https://json-schema.org/draft/2020-12/schema"; then test_pass; fi

test_begin "schema-server-capabilities-closed-draft-2020-12-schema" "schema server-capabilities (closed draft 2020-12 schema)"
run_bzr schema server-capabilities
if assert_success && assert_json_valid &&
    assert_json '.["$schema"]' "https://json-schema.org/draft/2020-12/schema" &&
    assert_json '.additionalProperties' "false"; then test_pass; fi

test_begin "schema-error-valid-error-envelope-schema" "schema error (valid error envelope schema)"
run_bzr schema error
if assert_success && assert_json_valid &&
    assert_json '.["$schema"]' "https://json-schema.org/draft/2020-12/schema" &&
    assert_json '.properties.error.type' "object" &&
    assert_json_exists '.properties.error.required | index("type")' &&
    assert_json_exists '.properties.error.required | index("message")' &&
    assert_json_exists '.properties.error.required | index("exit_code")'; then test_pass; fi

# Error JSON routes to stderr even under --json, so assert there.
test_begin "schema-unknown-name-input-error-exit-7" "schema unknown name (input error, exit 7)"
run_bzr schema nosuchschema
if assert_exit_code 7 && assert_stderr_contains '"type":"input"'; then test_pass; fi

echo ""
