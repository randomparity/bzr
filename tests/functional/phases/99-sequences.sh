# 18-sequences
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 16b: Complex multi-command sequences
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 16b: Complex sequences ──────────────────────────────"

# 104 — an update carrying no change fields must be rejected (exit 7) and
# must NOT issue an empty PUT that mutates last_change_time. Guards against
# the silent no-op update.
test_begin "104. bug update with no change fields is rejected (exit 7)"
run_bzr bug create --product FuncTestProd --component Backend \
    --summary "No-op update guard" --description "noop guard" \
    --priority Normal --severity normal --op-sys All --rep-platform All
if assert_success && assert_json_exists '.id'; then
    SEQ_NOOP=$(jq -r '.id' "$BZR_STDOUT")
    run_bzr bug update "$SEQ_NOOP"
    if assert_exit_code 7; then
        run_bzr bug view "$SEQ_NOOP"
        if assert_success && assert_json '.priority' "Normal"; then test_pass; fi
    fi
else test_skip "create failed"; fi

# 105 — full lifecycle with state verification at each transition, plus the
# atomic --comment landing on the resolve step.
test_begin "105. bug lifecycle state verification (new → confirmed → resolved)"
run_bzr bug create --product FuncTestProd --component Backend \
    --summary "Lifecycle bug" --description "lifecycle" \
    --priority Normal --severity normal --op-sys All --rep-platform All
if assert_success && assert_json_exists '.id'; then
    SEQ_LIFE=$(jq -r '.id' "$BZR_STDOUT")
    run_bzr bug update "$SEQ_LIFE" --status CONFIRMED
    if assert_success; then
        run_bzr bug view "$SEQ_LIFE"
        if assert_json '.status' "CONFIRMED"; then
            run_bzr bug update "$SEQ_LIFE" --status RESOLVED --resolution FIXED \
                --comment "Fixed: LIFECYCLE-MARKER-105"
            if assert_success; then
                run_bzr bug view "$SEQ_LIFE"
                if assert_json '.status' "RESOLVED" && assert_json '.resolution' "FIXED"; then
                    run_bzr comment list "$SEQ_LIFE"
                    if assert_success &&
                        [[ "$(jq '[.[] | select(.text | contains("LIFECYCLE-MARKER-105"))] | length' "$BZR_STDOUT")" -ge 1 ]]; then
                        test_pass
                    else
                        test_fail "resolve comment not found"
                    fi
                fi
            fi
        fi
    fi
else test_skip "create failed"; fi

# 106 — one mutated bug read back through every transport. Catches REST /
# hybrid / XML-RPC field divergence (e.g. XML-RPC parsing of populated and
# empty fields).
test_begin "106. cross-transport read parity (rest / hybrid / xmlrpc)"
run_bzr bug create --product FuncTestProd --component Backend \
    --summary "Parity bug" --description "parity" \
    --priority High --severity normal --op-sys All --rep-platform All
if assert_success && assert_json_exists '.id'; then
    SEQ_PAR=$(jq -r '.id' "$BZR_STDOUT")
    run_bzr bug update "$SEQ_PAR" --whiteboard "parity-marker-106"
    if assert_success; then
        run_bzr --api rest bug view "$SEQ_PAR"
        if assert_success &&
            assert_json '.whiteboard' "parity-marker-106" &&
            assert_json '.summary' "Parity bug"; then
            run_bzr --api hybrid bug view "$SEQ_PAR"
            if assert_success &&
                assert_json '.whiteboard' "parity-marker-106" &&
                assert_json '.summary' "Parity bug"; then
                run_bzr --api xmlrpc bug view "$SEQ_PAR"
                if assert_success &&
                    assert_json '.whiteboard' "parity-marker-106" &&
                    assert_json '.summary' "Parity bug"; then
                    test_pass
                fi
            fi
        fi
    fi
else test_skip "create failed"; fi

# 107 — a batch update mixing a valid id with a non-existent one must exit 11
# (partial failure), report each leg, AND still commit the valid leg.
test_begin "107. batch update partial failure (exit 11) commits valid leg"
run_bzr bug create --product FuncTestProd --component Backend \
    --summary "Batch valid leg" --description "batch" \
    --priority Normal --severity normal --op-sys All --rep-platform All
if assert_success && assert_json_exists '.id'; then
    SEQ_BVALID=$(jq -r '.id' "$BZR_STDOUT")
    run_bzr bug update "$SEQ_BVALID" 999999 --whiteboard "batch-partial-107"
    if assert_exit_code 11 &&
        assert_json '.succeeded[0]' "$SEQ_BVALID" &&
        assert_json '.failed[0].id' "999999"; then
        run_bzr bug view "$SEQ_BVALID"
        if assert_success && assert_json '.whiteboard' "batch-partial-107"; then test_pass; fi
    fi
else test_skip "create failed"; fi

# 108 — clone must carry the source description into the clone's comment #0
# (description), not silently drop it.
test_begin "108. clone preserves source description in comment #0"
run_bzr bug create --product FuncTestProd --component Backend \
    --summary "Clone description source" --description "CLONE-DESC-MARKER-108" \
    --priority Normal --severity normal --op-sys Linux --rep-platform PC
if assert_success && assert_json_exists '.id'; then
    SEQ_CSRC=$(jq -r '.id' "$BZR_STDOUT")
    run_bzr bug clone "$SEQ_CSRC" --op-sys Linux --rep-platform PC
    if assert_success && assert_json_exists '.id'; then
        SEQ_CDST=$(jq -r '.id' "$BZR_STDOUT")
        run_bzr comment list "$SEQ_CDST"
        if assert_success &&
            [[ "$(jq '[.[] | select(.count == 0 and (.text | contains("CLONE-DESC-MARKER-108")))] | length' "$BZR_STDOUT")" -ge 1 ]]; then
            test_pass
        else
            test_fail "clone comment #0 missing source description"
        fi
    fi
else test_skip "create failed"; fi

echo ""
