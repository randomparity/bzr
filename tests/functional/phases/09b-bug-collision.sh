# 09b-bug-collision
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# Reads: none. Creates: its own bugs.
# shellcheck shell=bash
#
# Optimistic-concurrency guard `bug update --expect-unchanged-since` (exit 14
# MidAirCollision). The collision case uses wait_for_changed to make the
# second-granular last_change_time advance deterministically.

# ══════════════════════════════════════════════════════════════════════
# Phase 9b: Mid-air collision guard (--expect-unchanged-since)
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 9b: Mid-air collision guard ───────────────────────"

_CC=(--product FuncTestProd --component Backend --op-sys Linux --rep-platform PC --description d)

test_begin "147. bug update --expect-unchanged-since happy path"
CID=$(make_bug "${_CC[@]}" --summary "collision happy")
run_bzr bug view "$CID"
# Capture last_change_time only after confirming the view succeeded — otherwise
# BZR_STDOUT is empty and the jq capture would trip set -e and abort the run.
if assert_success; then
    LCT=$(jq -r '.last_change_time' "$BZR_STDOUT" 2>/dev/null || true)
    run_bzr bug update "$CID" --priority High --expect-unchanged-since "$LCT"
    if assert_success; then
        run_bzr bug view "$CID"
        if assert_json '.priority' "High"; then test_pass; fi
    fi
fi

test_begin "148. bug update --expect-unchanged-since detects collision (exit 14)"
CID=$(make_bug "${_CC[@]}" --summary "collision detect")
run_bzr bug view "$CID"
if assert_success; then
    LCT=$(jq -r '.last_change_time' "$BZR_STDOUT" 2>/dev/null || true)
    # Mutate, then wait until last_change_time strictly advances, so the stale
    # guard timestamp is genuinely older than the server's current value.
    run_bzr bug update "$CID" --whiteboard "collision-touch"
    if wait_for_changed "$CID" "$LCT"; then
        run_bzr bug update "$CID" --priority Low --expect-unchanged-since "$LCT"
        if assert_exit_code 14; then test_pass; fi
    else
        test_skip "last_change_time did not advance within retry budget"
    fi
fi

unset _CC CID LCT
echo ""
