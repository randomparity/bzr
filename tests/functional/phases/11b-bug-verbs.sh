# 11b-bug-verbs
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# Reads: none (creates its own bugs). Creates: none shared.
# shellcheck shell=bash
#
# Covers the convenience verbs added since v0.4.4 that work against stock
# Bugzilla 5.x: `bug resolve` and `bug dup`. `bug close`/`bug reopen` are NOT
# covered here — they target the CLOSED/REOPENED statuses, which the default
# Bugzilla workflow used by these containers does not define (see verbs.rs).

# ══════════════════════════════════════════════════════════════════════
# Phase 11b: Bug convenience verbs (resolve, dup)
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 11b: Bug verbs (resolve, dup) ─────────────────────"

_VERB_CREATE=(--product FuncTestProd --component Backend --op-sys Linux --rep-platform PC --description d)

test_begin "130. bug resolve (default FIXED)"
VID=$(make_bug "${_VERB_CREATE[@]}" --summary "verb resolve default")
run_bzr bug resolve "$VID"
if assert_success; then
    run_bzr bug view "$VID"
    if assert_json '.status' "RESOLVED" && assert_json '.resolution' "FIXED"; then test_pass; fi
fi

test_begin "131. bug resolve --as WONTFIX"
VID=$(make_bug "${_VERB_CREATE[@]}" --summary "verb resolve wontfix")
run_bzr bug resolve "$VID" --as WONTFIX
if assert_success; then
    run_bzr bug view "$VID"
    if assert_json '.status' "RESOLVED" && assert_json '.resolution' "WONTFIX"; then test_pass; fi
fi

test_begin "132. bug resolve --comment lands an atomic comment"
VID=$(make_bug "${_VERB_CREATE[@]}" --summary "verb resolve comment")
run_bzr bug resolve "$VID" --as FIXED --comment "resolved via verb"
if assert_success; then
    run_bzr comment list "$VID"
    if assert_stdout_contains "resolved via verb"; then test_pass; fi
fi

test_begin "133. bug dup marks source a duplicate of target"
SRC=$(make_bug "${_VERB_CREATE[@]}" --summary "verb dup source")
TGT=$(make_bug "${_VERB_CREATE[@]}" --summary "verb dup target")
run_bzr bug dup "$SRC" "$TGT"
if assert_success; then
    run_bzr bug view "$SRC"
    if assert_json '.status' "RESOLVED" && assert_json '.resolution' "DUPLICATE" &&
        assert_json '.dupe_of' "$TGT"; then test_pass; fi
fi

test_begin "134. bug resolve batch partial failure (exit 11) commits valid leg"
VID=$(make_bug "${_VERB_CREATE[@]}" --summary "verb resolve batch")
run_bzr bug resolve "$VID" 999999 --as FIXED
if assert_exit_code 11; then
    run_bzr bug view "$VID"
    if assert_json '.status' "RESOLVED"; then test_pass; fi
fi

unset _VERB_CREATE VID SRC TGT
echo ""
