# 08i-batch-error-escaping
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# Reads: none. Creates: its own bug.
# shellcheck shell=bash
#
# Terminal-control escaping in the batch command layer's per-item error line
# (ADR 0070, issue #759). The per-item error is a BzrError display that can
# carry server-supplied text, so a hostile one must reach stderr escaped. A
# wiremock fixture can only prove "given this error string, we escape it"; only
# a real server proves the error a Bugzilla actually returns still reaches the
# stderr line as a control character. The first case is server-independent (a
# real partial failure prints the well-formed stderr line and exits 11); the
# hostile case round-trips the payload through --json first and skips when the
# server does not echo it in the per-item error, so a container that sanitises
# the input reports a named skip rather than a vacuous pass.

# ══════════════════════════════════════════════════════════════════════
# Phase 8i: Batch-error terminal escaping
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 8i: Batch-error terminal escaping ─────────────────"

# Bash 3.2 (the macOS system bash) has no $'\uXXXX'. PAYLOAD_ESC is the
# clear-screen ESC sequence; PATTERN_ESC is its BRE-safe form for the
# grep-based assertion helpers (they run without -F, so the `[` is escaped).
_BE_ARGS=(--product FuncTestProd --component Backend --op-sys Linux --platform PC
    --description "batch error escaping probe")
PAYLOAD_ESC=$'\x1b[2J'
PATTERN_ESC=$'\x1b''\[2J'

BE_A=$(make_bug "${_BE_ARGS[@]}" --summary "batch error probe A")

test_begin "batch-error-well-formed" "a real batch partial failure prints the per-item stderr line and exits 11"
if [[ -z "$BE_A" ]]; then
    test_fail "could not create the probe bug"
else
    # Mixing the valid bug with a non-existent id fails exactly one leg (the
    # non-existent bug), so the batch reports it on stderr and exits 11.
    run_bzr_raw --output table bug update "$BE_A" 999999 \
        --whiteboard "batch-escape-08i"
    if assert_exit_code 11 && assert_stderr_contains "Failed to update bug #999999:"; then
        test_pass
    fi
fi

test_begin "batch-error-hostile-escaped" "a hostile per-item error reaches stderr escaped"
if [[ -z "$BE_A" ]]; then
    test_fail "could not create the probe bug"
else
    # Round-trip probe: does the server echo the hostile resolution in the
    # per-item error? The --json batch result carries the raw server text;
    # serde escapes code points below 0x20, so a stored ESC appears as \u001b.
    run_bzr bug update "$BE_A" 999999 \
        --status RESOLVED --resolution "$PAYLOAD_ESC"
    if ! grep -q -- 'u001b' "$BZR_STDOUT" 2>/dev/null; then
        test_skip "the server did not echo the ESC in the per-item error; the unit tests carry the escape contract"
    else
        run_bzr_raw --output table bug update "$BE_A" 999999 \
            --status RESOLVED --resolution "$PAYLOAD_ESC"
        if assert_stderr_not_contains "$PATTERN_ESC" && assert_stderr_contains 'u{1b}'; then
            test_pass
        fi
    fi
fi

unset BE_A PAYLOAD_ESC PATTERN_ESC
unset _BE_ARGS
