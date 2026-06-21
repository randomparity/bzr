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
test_begin "110. completion bash"
run_bzr_raw completion bash
if assert_success && assert_stdout_contains "_bzr()"; then test_pass; fi

test_begin "111. completion zsh"
run_bzr_raw completion zsh
if assert_success && assert_stdout_contains "#compdef bzr"; then test_pass; fi

test_begin "112. completion fish"
run_bzr_raw completion fish
if assert_success && assert_stdout_contains "complete -c bzr"; then test_pass; fi

test_begin "113. completion powershell"
run_bzr_raw completion powershell
if assert_success && assert_stdout_contains "Register-ArgumentCompleter"; then test_pass; fi

test_begin "114. completion elvish"
run_bzr_raw completion elvish
if assert_success && assert_stdout_contains "edit:completion"; then test_pass; fi

test_begin "115. completion invalid shell (clap rejects, exit 2)"
run_bzr_raw completion notashell
if assert_exit_code 2 && assert_stderr_contains "invalid value"; then test_pass; fi

# schema: list mode + per-name schema is valid JSON (draft 2020-12).
test_begin "116. schema (list)"
run_bzr schema
if assert_success && assert_json_valid &&
	assert_json_exists 'index("bug")' &&
	assert_json_exists 'index("bug-create-input")'; then test_pass; fi

test_begin "117. schema bug (valid draft 2020-12 schema)"
run_bzr schema bug
if assert_success && assert_json_valid &&
	assert_json '.["$schema"]' "https://json-schema.org/draft/2020-12/schema"; then test_pass; fi

test_begin "118. schema bug-update-input (valid draft 2020-12 schema)"
run_bzr schema bug-update-input
if assert_success && assert_json_valid &&
	assert_json '.["$schema"]' "https://json-schema.org/draft/2020-12/schema" &&
	assert_json '.["$defs"].bugUpdateInputObject.additionalProperties' "false"; then test_pass; fi

# Error JSON routes to stderr even under --json, so assert there.
test_begin "119. schema unknown name (input error, exit 7)"
run_bzr schema nosuchschema
if assert_exit_code 7 && assert_stderr_contains '"type":"input"'; then test_pass; fi

echo ""
