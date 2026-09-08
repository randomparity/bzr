# 08h-terminal-escaping
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# Reads: none. Creates: its own bug and comment.
# shellcheck shell=bash
#
# Terminal-control escaping in table output (ADR 0065, issue #743). A wiremock
# fixture can only prove "given this shape, we escape it"; only a real server
# proves the shape a Bugzilla actually stores and returns still reaches the
# writers as a control character. Each case round-trips the payload through
# `--json` first and skips when the server normalised it away, so a container
# that sanitises input reports a named skip rather than a vacuous pass or a
# failure for behaviour bzr does not control.
#
# The fourth case pins the exclusion this change deliberately does not close:
# `--json` is a published schema surface, so the bidi override stays raw there.

# ══════════════════════════════════════════════════════════════════════
# Phase 8h: Terminal-control escaping
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 8h: Terminal-control escaping ─────────────────────"

# Bash 3.2 (the macOS system bash) has no $'\uXXXX', so the RIGHT-TO-LEFT
# OVERRIDE is spelled as its UTF-8 bytes. PATTERN_ESC is the BRE-safe form of
# PAYLOAD_ESC: the assertion helpers grep without -F, so the `[` is escaped.
# Every table assertion passes `--output table` explicitly: `run_bzr_raw`
# redirects stdout to a file, so bzr's TTY auto-detection would select JSON.
_TE_ARGS=(--product FuncTestProd --component Backend --op-sys Linux --platform PC
    --description "terminal escaping probe")
PAYLOAD_RLO=$'\xe2\x80\xae'
PAYLOAD_ESC=$'\x1b[2J'
PATTERN_ESC=$'\x1b''\[2J'

TE_BIDI_BUG=$(make_bug "${_TE_ARGS[@]}" --summary "escape probe ${PAYLOAD_RLO} tail")
TE_BIDI_STORED=0
if [[ -n "$TE_BIDI_BUG" ]]; then
    run_bzr bug view "$TE_BIDI_BUG"
    if jq -r '.summary' "$BZR_STDOUT" 2>/dev/null | grep -q -- "$PAYLOAD_RLO"; then
        TE_BIDI_STORED=1
    fi
fi

test_begin "escape-summary-bidi" "bug view escapes a bidi override in the summary"
if [[ -z "$TE_BIDI_BUG" ]]; then
    test_fail "bug create with a bidi override in the summary returned no bug ID"
elif [[ $TE_BIDI_STORED -eq 0 ]]; then
    test_skip "the server normalised the bidi override out of the summary"
else
    run_bzr_raw --output table bug view "$TE_BIDI_BUG"
    if assert_success &&
        assert_stdout_not_contains "$PAYLOAD_RLO" &&
        assert_stdout_contains 'u{202e}'; then
        test_pass
    fi
fi

test_begin "escape-json-passthrough" "--json leaves the bidi override to the JSON contract"
if [[ -z "$TE_BIDI_BUG" ]]; then
    test_fail "no fixture bug: the bidi summary create above returned no bug ID"
elif [[ $TE_BIDI_STORED -eq 0 ]]; then
    test_skip "the server normalised the bidi override out of the summary"
else
    run_bzr bug view "$TE_BIDI_BUG"
    if assert_success && assert_stdout_contains "$PAYLOAD_RLO"; then
        test_pass
    fi
fi

test_begin "escape-summary-esc" "bug view escapes a raw ESC in the summary"
TE_ESC_BUG=$(make_bug "${_TE_ARGS[@]}" --summary "escape probe ${PAYLOAD_ESC} tail")
if [[ -z "$TE_ESC_BUG" ]]; then
    test_fail "bug create with a raw ESC in the summary returned no bug ID"
else
    run_bzr bug view "$TE_ESC_BUG"
    # The round-trip probe reads the JSON spelling, not the raw byte: serde
    # escapes code points below 0x20, so a stored ESC appears there as
    # `\u001b`. Bugzilla 5.0.6 strips it from a summary and this case skips;
    # `escape-comment-body` carries the ESC contract, since comment bodies do
    # round-trip it.
    if ! jq -r 'tojson' "$BZR_STDOUT" 2>/dev/null | grep -q -- 'u001b'; then
        test_skip "the server normalised the ESC sequence out of the summary"
    else
        run_bzr_raw --output table bug view "$TE_ESC_BUG"
        if assert_success &&
            assert_stdout_not_contains "$PATTERN_ESC" &&
            assert_stdout_contains 'u{1b}'; then
            test_pass
        fi
    fi
fi

test_begin "escape-comment-body" "comment list escapes controls in a comment body"
if [[ -z "$TE_BIDI_BUG" ]]; then
    test_fail "no fixture bug: the bidi summary create above returned no bug ID"
else
    run_bzr comment add "$TE_BIDI_BUG" \
        --body "body probe ${PAYLOAD_RLO} and ${PAYLOAD_ESC} tail"
    if [[ $BZR_EXIT -ne 0 ]]; then
        test_fail "comment add with control characters failed (exit $BZR_EXIT)"
    else
        run_bzr comment list "$TE_BIDI_BUG"
        TE_BODY_ESC=0
        if grep -q -- 'u001b' "$BZR_STDOUT" 2>/dev/null; then
            TE_BODY_ESC=1
        fi
        if ! grep -q -- "$PAYLOAD_RLO" "$BZR_STDOUT" 2>/dev/null; then
            test_skip "the server normalised the bidi override out of the comment body"
        else
            run_bzr_raw --output table comment list "$TE_BIDI_BUG"
            if assert_success &&
                assert_stdout_not_contains "$PAYLOAD_RLO" &&
                assert_stdout_not_contains "$PATTERN_ESC" &&
                assert_stdout_contains 'u{202e}'; then
                # Bugzilla 5.0.6 round-trips a raw ESC in a comment body even
                # though it strips one from a summary, so this is where the Cc
                # half of the predicate is proven end to end. Conditional so a
                # server that sanitises bodies reports the bidi half rather
                # than failing on behaviour bzr does not control.
                if [[ $TE_BODY_ESC -eq 0 ]] || assert_stdout_contains 'u{1b}'; then
                    test_pass
                fi
            fi
        fi
    fi
fi

unset TE_BIDI_BUG TE_BIDI_STORED TE_ESC_BUG TE_BODY_ESC
unset PAYLOAD_RLO PAYLOAD_ESC PATTERN_ESC
unset _TE_ARGS
