# 17b-arg-validation
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# Reads: none (needs only the configured default server for the dispatch-level
# checks). Creates: none. CLI-parse invariants — these fail before any network,
# so they are grouped here rather than scattered across server-dependent phases.
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 17b: Argument validation (parse-level, no network)
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 17b: Argument validation ──────────────────────────"

# clap mutual-exclusion conflicts → exit 2, message on stderr.
test_begin "bug-create-description-description-file-conflict" "bug create --description + --description-file conflict"
run_bzr bug create --product P --component C --summary S --description x --description-file /tmp/none
if assert_exit_code 2 && assert_stderr_contains "cannot be used with"; then test_pass; fi

test_begin "comment-add-body-body-file-conflict" "comment add --body + --body-file conflict"
run_bzr comment add 1 --body x --body-file /tmp/none
if assert_exit_code 2 && assert_stderr_contains "cannot be used with"; then test_pass; fi

test_begin "bug-list-offset-paginate-conflict" "bug list --offset + --paginate conflict"
run_bzr bug list --offset 1 --paginate
if assert_exit_code 2 && assert_stderr_contains "cannot be used with"; then test_pass; fi

test_begin "query-run-count-offset-conflict" "query run --count + --offset conflict"
run_bzr query run prod-bugs --count --offset 1
if assert_exit_code 7 &&
    assert_stderr_contains "cannot be combined with --offset or --paginate"; then test_pass; fi

test_begin "query-run-count-paginate-conflict" "query run --count + --paginate conflict"
run_bzr query run prod-bugs --count --paginate
if assert_exit_code 7 &&
    assert_stderr_contains "cannot be combined with --offset or --paginate"; then test_pass; fi

test_begin "bug-create-template-from-json-conflict" "bug create --template + --from-json conflict"
run_bzr bug create --template t --from-json /tmp/none
if assert_exit_code 2 && assert_stderr_contains "cannot be used with"; then test_pass; fi

test_begin "bug-update-status-dupe-of-conflict" "bug update --status + --dupe-of conflict"
run_bzr bug update 1 --status RESOLVED --dupe-of 2
if assert_exit_code 2 && assert_stderr_contains "cannot be used with"; then test_pass; fi

test_begin "server-url-server-conflict" "--server-url + --server conflict"
run_bzr --server-url http://example.invalid:1 --server test whoami
if assert_exit_code 2 && assert_stderr_contains "cannot be used with"; then test_pass; fi

test_begin "server-api-key-env-requires-server-url" "--server-api-key-env requires --server-url"
run_bzr --server-api-key-env BZR_FUNC_INLINE_KEY server info
if assert_exit_code 2 && assert_stderr_contains "required"; then test_pass; fi

test_begin "server-tls-insecure-requires-server-url" "--server-tls-insecure requires --server-url"
run_bzr --server-tls-insecure server info
if assert_exit_code 2 && assert_stderr_contains "required"; then test_pass; fi

test_begin "ad-hoc-tls-flags-are-mutually-exclusive" "ad-hoc TLS flags are mutually exclusive"
run_bzr --server-url "$BZ_URL" --server-tls-insecure \
    --server-tls-pin-now server info
if assert_exit_code 2 && assert_stderr_contains "cannot be used with"; then test_pass; fi

# `whoami show` subcommand removed (#323): bare `whoami` only.
test_begin "whoami-show-removed-subcommand-exit-2" "whoami show (removed subcommand → exit 2)"
run_bzr whoami show
if assert_exit_code 2 && assert_stderr_contains "unexpected argument"; then test_pass; fi

# --dry-run rejected on a non-mutation command (input error, exit 7).
test_begin "dry-run-on-non-mutation-exit-7" "--dry-run on non-mutation (exit 7)"
run_bzr --dry-run server info
if assert_exit_code 7 && assert_stderr_contains "only supported for bug create"; then test_pass; fi

test_begin "attachment-update-no-fields-rejected" "attachment update no fields rejected"
run_bzr attachment update 1
if assert_exit_code 7 && assert_stderr_contains "no attachment fields to update"; then test_pass; fi

test_begin "comment-tag-no-changes-rejected" "comment tag no changes rejected"
run_bzr comment tag 1
if assert_exit_code 7 && assert_stderr_contains "no comment tag changes"; then test_pass; fi

echo ""
