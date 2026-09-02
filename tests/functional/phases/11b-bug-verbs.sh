# 11b-bug-verbs
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# Reads: none (creates its own bugs). Creates: none shared.
# shellcheck shell=bash
#
# Covers the convenience verbs added since v0.4.4 that work against stock
# Bugzilla 5.x: `bug resolve` and `bug dup`. `bug close`/`bug reopen` are NOT
# covered here — they target the CLOSED/REOPENED statuses, which the default
# Bugzilla workflow used by these containers does not define (see verbs.rs and
# issue #349).

# ══════════════════════════════════════════════════════════════════════
# Phase 11b: Bug convenience verbs (resolve, dup)
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 11b: Bug verbs (resolve, dup) ─────────────────────"

_VERB_CREATE=(--product FuncTestProd --component Backend --op-sys Linux --platform PC --description d)

test_begin "bug-resolve-default-fixed" "bug resolve (default FIXED)"
VID=$(make_bug "${_VERB_CREATE[@]}" --summary "verb resolve default")
run_bzr bug resolve "$VID"
if assert_success; then
    run_bzr bug view "$VID"
    if assert_json '.status' "RESOLVED" && assert_json '.resolution' "FIXED"; then test_pass; fi
fi

test_begin "bug-resolve-as-wontfix" "bug resolve --as WONTFIX"
VID=$(make_bug "${_VERB_CREATE[@]}" --summary "verb resolve wontfix")
run_bzr bug resolve "$VID" --as WONTFIX
if assert_success; then
    run_bzr bug view "$VID"
    if assert_json '.status' "RESOLVED" && assert_json '.resolution' "WONTFIX"; then test_pass; fi
fi

test_begin "bug-resolve-comment-lands-an-atomic-comment" "bug resolve --comment lands an atomic comment"
VID=$(make_bug "${_VERB_CREATE[@]}" --summary "verb resolve comment")
run_bzr bug resolve "$VID" --as FIXED --comment "resolved via verb"
if assert_success; then
    run_bzr comment list "$VID"
    if assert_stdout_contains "resolved via verb"; then test_pass; fi
fi

test_begin "bug-dup-marks-source-a-duplicate-of-target" "bug dup marks source a duplicate of target"
SRC=$(make_bug "${_VERB_CREATE[@]}" --summary "verb dup source")
TGT=$(make_bug "${_VERB_CREATE[@]}" --summary "verb dup target")
run_bzr bug dup "$SRC" "$TGT"
if assert_success; then
    run_bzr bug view "$SRC"
    if assert_json '.status' "RESOLVED" && assert_json '.resolution' "DUPLICATE" &&
        assert_json '.dupe_of' "$TGT"; then test_pass; fi
fi

test_begin "bug-resolve-batch-partial-failure-exit-11-commits-valid-leg" "bug resolve batch partial failure (exit 11) commits valid leg"
VID=$(make_bug "${_VERB_CREATE[@]}" --summary "verb resolve batch")
run_bzr bug resolve "$VID" 999999 --as FIXED
if assert_exit_code 11; then
    run_bzr bug view "$VID"
    if assert_json '.status' "RESOLVED"; then test_pass; fi
fi

test_begin "bug-resolve-expect-unchanged-since-happy-path" "bug resolve --expect-unchanged-since happy path"
VID=$(make_bug "${_VERB_CREATE[@]}" --summary "verb resolve guarded")
run_bzr bug view "$VID"
if assert_success; then
    LCT=$(jq -r '.last_change_time' "$BZR_STDOUT" 2>/dev/null || true)
    run_bzr bug resolve "$VID" --expect-unchanged-since "$LCT"
    if assert_success; then
        run_bzr bug view "$VID"
        if assert_json '.status' "RESOLVED"; then test_pass; fi
    fi
fi

test_begin "bug-dup-expect-unchanged-since-detects-collision" "bug dup --expect-unchanged-since detects collision"
SRC=$(make_bug "${_VERB_CREATE[@]}" --summary "verb dup guarded source")
TGT=$(make_bug "${_VERB_CREATE[@]}" --summary "verb dup guarded target")
run_bzr bug view "$SRC"
if assert_success; then
    LCT=$(jq -r '.last_change_time' "$BZR_STDOUT" 2>/dev/null || true)
    run_bzr bug update "$SRC" --whiteboard "verb-guard-touch"
    if wait_for_changed "$SRC" "$LCT"; then
        run_bzr bug dup "$SRC" "$TGT" --expect-unchanged-since "$LCT"
        if assert_exit_code 14; then test_pass; fi
    else
        test_skip "last_change_time did not advance within retry budget"
    fi
fi

unset _VERB_CREATE VID SRC TGT LCT
echo ""
