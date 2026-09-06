# 08f-bug-saved-search
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# Reads: BZ_URL. Creates: nothing.
# shellcheck shell=bash
#
# Exercises `bug search --saved-search` / `--sharer` against a real container.
# Resolving a saved search is a Red Hat Bugzilla extension that no supported
# image implements, so the refusal path (ADR-0052, exit 15) is the primary case
# here rather than an edge case, and the success path cannot be exercised — the
# wiremock tests in src/commands/bug/search_tests.rs carry that. The phase seeds
# nothing, so every test is unconditional and a SKIP would itself be a defect.

# ══════════════════════════════════════════════════════════════════════
# Phase 8f: Bug search --saved-search (capability refusal)
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 8f: Bug search --saved-search ─────────────────────"

_SS_NAME="bzr-func-saved-search"

# Ordering matters: the capability answer is cached per server in the shared
# XDG config this run uses, so the FIRST --saved-search invocation is the only
# cache miss. Run the XML-RPC case first, so it is the one that actually issues
# the REST-only `/rest/extensions` probe under `--api xmlrpc`; the REST case
# then covers the cache-hit path. `RUST_LOG` assertions pin which is which
# rather than leaving it to position.
test_begin "bug-search-saved-search-refused-over-xmlrpc" "bug search --saved-search refused over XML-RPC (cache miss, REST probe)"
RUST_LOG=bzr=debug run_bzr --api xmlrpc bug search --saved-search "$_SS_NAME"
if assert_exit_code 15 &&
    assert_stderr_contains 'unsupported_server_capability' &&
    assert_stderr_contains "${BZ_URL}/rest/extensions"; then
    test_pass
fi

test_begin "bug-search-saved-search-refused-over-rest" "bug search --saved-search refused over REST (cache hit)"
RUST_LOG=bzr=debug run_bzr --api rest bug search --saved-search "$_SS_NAME"
if assert_exit_code 15 &&
    assert_stderr_contains 'unsupported_server_capability' &&
    assert_stderr_contains 'RedHat' &&
    assert_stderr_not_contains "${BZ_URL}/rest/extensions"; then
    test_pass
fi

test_begin "bug-search-saved-search-refusal-names-the-search" "refusal names the requested saved search"
run_bzr bug search --saved-search "$_SS_NAME"
if assert_exit_code 15 && assert_stderr_contains "$_SS_NAME"; then
    test_pass
fi

test_begin "bug-search-saved-search-sharer-refused" "bug search --saved-search --sharer refused"
run_bzr bug search --saved-search "$_SS_NAME" --sharer 112233
if assert_exit_code 15 && assert_stderr_contains 'unsupported_server_capability'; then
    test_pass
fi

test_begin "bug-search-saved-search-count-refused" "bug search --saved-search --count refused before counting"
run_bzr bug search --saved-search "$_SS_NAME" --count
if assert_exit_code 15 && assert_stdout_empty; then
    test_pass
fi

test_begin "inline-server-url-bug-search-saved-search-refused" "inline --server-url bug search --saved-search refused"
run_bzr --server-url "$BZ_URL" bug search --saved-search "$_SS_NAME"
if assert_exit_code 15 && assert_stderr_contains 'unsupported_server_capability'; then
    test_pass
fi

test_begin "bug-search-saved-search-rejects-query" "bug search rejects --saved-search with a query"
run_bzr bug search "some text" --saved-search "$_SS_NAME"
if assert_exit_code 2; then test_pass; fi

test_begin "bug-search-saved-search-rejects-from-url" "bug search rejects --saved-search with --from-url"
run_bzr bug search --from-url "${BZ_URL}/buglist.cgi?bug_id=1" --saved-search "$_SS_NAME"
if assert_exit_code 2; then test_pass; fi

test_begin "bug-search-sharer-requires-saved-search" "bug search --sharer requires --saved-search"
run_bzr bug search "some text" --sharer 1
if assert_exit_code 2; then test_pass; fi

test_begin "bug-search-sharer-rejects-non-numeric" "bug search --sharer rejects a non-numeric ID"
run_bzr bug search --saved-search "$_SS_NAME" --sharer not-a-number
if assert_exit_code 2; then test_pass; fi

test_begin "bug-search-saved-search-rejects-empty-name" "bug search rejects an empty --saved-search name"
run_bzr bug search --saved-search ""
if assert_exit_code 7 && assert_stderr_contains 'non-empty'; then
    test_pass
fi
