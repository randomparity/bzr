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
#
# What this phase can and cannot establish (ADR-0052 amendment, 2026-09-06).
# The probe now follows the transport in use, and no supported image advertises
# the RedHat extension, so a POSITIVE capability verdict is not obtainable here
# at all — that lives in the wiremock tier, which can mount an advertising
# XML-RPC response beside a failing REST surface. What a real container proves
# is the probe MECHANISM and the negative verdict: which transport carried the
# probe, from the RUST_LOG debug trace, and that the response was really
# received and parsed. (ADR 0061's shaped proxy can synthesize a positive
# advertisement, but it rewrites /rest/extensions only and carries no XML-RPC
# handling, so it does not cover the XML-RPC path asserted here.) The latter is why these tests assert the *absent*
# wording rather than only exit 15 — absent and undetermined share exit 15, so
# the exit code alone would pass whether the probe worked, silently failed, or
# never fired.

# ══════════════════════════════════════════════════════════════════════
# Phase 8f: Bug search --saved-search (capability refusal)
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 8f: Bug search --saved-search ─────────────────────"

_SS_NAME="bzr-func-saved-search"

# Ordering matters for the two CONFIGURED-server arms: the capability answer is
# cached per server in the shared XDG config this run uses, so the FIRST
# --saved-search invocation is the only cache miss. Run the XML-RPC case first,
# so it is the one that actually issues the probe; the REST case then covers the
# cache-hit path. `RUST_LOG` assertions pin which is which rather than leaving
# it to position.
#
# The two --server-url arms further down are deliberately NOT part of that
# ordering: an inline connection has no config entry, so the gate never reads or
# writes the cache (capability.rs short-circuits on ctx.inline_server()), and
# each is a fresh probe regardless of position.
test_begin "bug-search-saved-search-refused-over-xmlrpc" "bug search --saved-search refused over XML-RPC (cache miss)"
RUST_LOG=bzr=debug run_bzr --api xmlrpc bug search --saved-search "$_SS_NAME"
if assert_exit_code 15 &&
    assert_stderr_contains 'unsupported_server_capability' &&
    assert_stderr_contains 'Bugzilla.extensions'; then
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

# The transport discriminators. Inline --server-url so neither depends on cache
# state or on the arms above. Each asserts three independent things: the verdict
# is *absent* (so the response was received AND parsed — a broken adapter flips
# this to undetermined), the probe went over the transport asked for, and it did
# NOT go over the other one. Wire evidence from the debug trace, not position.
test_begin "bug-search-saved-search-probes-over-xmlrpc" "--api xmlrpc probes extensions over XML-RPC, not REST"
RUST_LOG=bzr=debug run_bzr --server-url "$BZ_URL" --api xmlrpc bug search --saved-search "$_SS_NAME"
if assert_exit_code 15 &&
    assert_stderr_contains 'unsupported_server_capability' &&
    assert_stderr_contains 'does not implement' &&
    assert_stderr_contains 'Bugzilla.extensions' &&
    assert_stderr_not_contains "${BZ_URL}/rest/extensions"; then
    test_pass
fi

test_begin "bug-search-saved-search-probes-over-rest" "--api rest probes extensions over REST, not XML-RPC"
RUST_LOG=bzr=debug run_bzr --server-url "$BZ_URL" --api rest bug search --saved-search "$_SS_NAME"
if assert_exit_code 15 &&
    assert_stderr_contains 'does not implement' &&
    assert_stderr_contains "${BZ_URL}/rest/extensions" &&
    assert_stderr_not_contains 'Bugzilla.extensions'; then
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
