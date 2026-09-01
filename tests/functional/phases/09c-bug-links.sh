# 09c-bug-links
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# shellcheck shell=bash

# ══════════════════════════════════════════════════════════════════════
# Phase 9c: Bug Links (relationship graph)
# ══════════════════════════════════════════════════════════════════════
echo "── Phase 9c: Bug Links ───────────────────────────────────────"

# A three-node dependency chain LINK_A -> LINK_B -> LINK_C exercising depth
# bounding (depth 1 vs 2), early termination (a large --depth past the end of
# the chain still terminates), the reverse `blocks` direction, and the relation
# filter. Bugzilla itself rejects circular dependencies, so a true cycle cannot
# be built against a live server; the visited-set / cycle-safety behavior is
# covered by the wiremock unit test in src/commands/bug/links_tests.rs.
LINK_A=""
LINK_B=""
LINK_C=""

test_begin "bug-create-link-node-a" "bug create (link node A)"
run_bzr bug create --product FuncTestProd --component Backend \
    --summary "Link node A" --description "links graph node A" \
    --op-sys All --rep-platform All
if assert_success && assert_json_exists '.id'; then
    LINK_A=$(jq -r '.id' "$BZR_STDOUT")
    test_pass
fi

test_begin "bug-create-link-node-b" "bug create (link node B)"
run_bzr bug create --product FuncTestProd --component Backend \
    --summary "Link node B" --description "links graph node B" \
    --op-sys All --rep-platform All
if assert_success && assert_json_exists '.id'; then
    LINK_B=$(jq -r '.id' "$BZR_STDOUT")
    test_pass
fi

test_begin "bug-create-link-node-c" "bug create (link node C)"
run_bzr bug create --product FuncTestProd --component Backend \
    --summary "Link node C" --description "links graph node C" \
    --op-sys All --rep-platform All
if assert_success && assert_json_exists '.id'; then
    LINK_C=$(jq -r '.id' "$BZR_STDOUT")
    test_pass
fi

test_begin "wire-dependency-chain-a-b-c" "wire dependency chain A->B->C"
if [[ -n "$LINK_A" ]] && [[ -n "$LINK_B" ]] && [[ -n "$LINK_C" ]]; then
    run_bzr bug update "$LINK_A" --depends-on-add "$LINK_B"
    a=$BZR_EXIT
    run_bzr bug update "$LINK_B" --depends-on-add "$LINK_C"
    b=$BZR_EXIT
    if [[ $a -eq 0 ]] && [[ $b -eq 0 ]]; then test_pass; else
        test_fail "chain wiring exits: $a $b"
    fi
else test_skip "no link nodes"; fi

test_begin "bug-links-a-one-hop-single-depth-1-depends-on-record" "bug links A (one hop): single depth-1 depends_on record"
if [[ -n "$LINK_A" ]] && [[ -n "$LINK_B" ]]; then
    run_bzr bug links "$LINK_A"
    if assert_success &&
        assert_json 'length' "1" &&
        assert_json '.[0].id' "$LINK_B" &&
        assert_json '.[0].relation' "depends_on" &&
        assert_json '.[0].direction' "out" &&
        assert_json '.[0].depth' "1"; then
        test_pass
    fi
else test_skip "no link nodes"; fi

test_begin "bug-links-a-recursive-depth-1-still-just-one-hop" "bug links A --recursive --depth 1: still just one hop"
if [[ -n "$LINK_A" ]]; then
    run_bzr bug links "$LINK_A" --recursive --depth 1
    if assert_success && assert_json 'length' "1"; then test_pass; fi
else test_skip "no link nodes"; fi

test_begin "bug-links-a-recursive-depth-2-reaches-c-at-depth-2" "bug links A --recursive --depth 2: reaches C at depth 2"
if [[ -n "$LINK_A" ]] && [[ -n "$LINK_B" ]] && [[ -n "$LINK_C" ]]; then
    run_bzr bug links "$LINK_A" --recursive --depth 2
    if assert_success &&
        assert_json 'length' "2" &&
        assert_json '.[0].id' "$LINK_B" &&
        assert_json '.[0].depth' "1" &&
        assert_json '.[1].id' "$LINK_C" &&
        assert_json '.[1].depth' "2"; then
        test_pass
    fi
else test_skip "no link nodes"; fi

test_begin "bug-links-a-recursive-depth-10-terminates-past-end-of-chain" "bug links A --recursive --depth 10: terminates past end of chain"
if [[ -n "$LINK_A" ]]; then
    run_bzr bug links "$LINK_A" --recursive --depth 10
    # The chain ends at C, so a large depth still yields exactly B (depth 1) and
    # C (depth 2) — traversal stops when the frontier empties, and the root A is
    # never emitted.
    if assert_success &&
        assert_json 'length' "2" &&
        assert_json '[.[].id] | index('"$LINK_A"')' "null"; then
        test_pass
    fi
else test_skip "no link nodes"; fi

test_begin "bug-links-b-relation-blocks-reverse-edge-to-a-direction-in" "bug links B --relation blocks: reverse edge to A (direction in)"
if [[ -n "$LINK_A" ]] && [[ -n "$LINK_B" ]]; then
    # A depends_on B, so B.blocks contains A; the record's direction is "in".
    run_bzr bug links "$LINK_B" --relation blocks
    if assert_success &&
        assert_json 'length' "1" &&
        assert_json '.[0].id' "$LINK_A" &&
        assert_json '.[0].relation' "blocks" &&
        assert_json '.[0].direction' "in"; then
        test_pass
    fi
else test_skip "no link nodes"; fi

test_begin "bug-links-depth-without-recursive-is-a-usage-error" "bug links --depth without --recursive is a usage error"
if [[ -n "$LINK_A" ]]; then
    run_bzr_raw bug links "$LINK_A" --depth 2
    if assert_failure; then test_pass; fi
else test_skip "no link nodes"; fi

test_begin "bug-links-depth-0-rejected-out-of-range" "bug links --depth 0 rejected (out of range)"
if [[ -n "$LINK_A" ]]; then
    run_bzr_raw bug links "$LINK_A" --recursive --depth 0
    if assert_failure; then test_pass; fi
else test_skip "no link nodes"; fi

test_begin "credentialless-bug-links-a-recursive-depth-2-public-server" "credentialless bug links A --recursive --depth 2 (public server)"
if [[ -n "$LINK_A" ]] && [[ -n "$LINK_C" ]]; then
    run_bzr_raw --json --server public bug links "$LINK_A" --recursive --depth 2
    if assert_success &&
        assert_json 'length' "2" &&
        assert_json '.[1].id' "$LINK_C" &&
        assert_json '.[1].depth' "2"; then
        test_pass
    fi
else test_skip "no link nodes"; fi

# Red Hat Bugzilla returns `duplicates` as bug objects, unlike stock Bugzilla's
# numeric array. Host the captured minimal wire shape inside the same Bugzilla
# container so the real binary exercises vendor-shaped HTTP responses without
# depending on an external production service.
_RH_RUNTIME=$(container_runtime)
_RH_CONTAINER=$(bugzilla_container_name)
_RH_FIXTURE="$SCRIPT_DIR/fixtures/redhat-links.cgi"
_RH_REMOTE=/var/www/html/bugzilla/redhat-links.cgi
_RH_READY=0
if "$_RH_RUNTIME" cp "$_RH_FIXTURE" "$_RH_CONTAINER:$_RH_REMOTE" &&
    "$_RH_RUNTIME" exec "$_RH_CONTAINER" chmod 755 "$_RH_REMOTE"; then
    _RH_READY=1
fi

test_begin "red-hat-object-valued-duplicate-one-hop" "Red Hat object-valued duplicate: one hop"
if [[ $_RH_READY -eq 1 ]]; then
    run_bzr_raw --json --server-url "$BZ_URL/redhat-links.cgi" --api rest \
        bug links 998
    if assert_success &&
        assert_json 'length' "1" &&
        assert_json '.[0].id' "1117050" &&
        assert_json '.[0].relation' "duplicates" &&
        assert_json '.[0].depth' "1"; then
        test_pass
    fi
else test_fail "could not install Red Hat response fixture"; fi

test_begin "red-hat-object-valued-duplicate-recursive-depth-2" "Red Hat object-valued duplicate: recursive depth 2"
if [[ $_RH_READY -eq 1 ]]; then
    run_bzr_raw --json --server-url "$BZ_URL/redhat-links.cgi" --api rest \
        bug links 998 --recursive --depth 2
    if assert_success &&
        assert_json 'length' "2" &&
        assert_json '.[0].id' "1117050" &&
        assert_json '.[0].depth' "1" &&
        assert_json '.[1].id' "1200000" &&
        assert_json '.[1].depth' "2"; then
        test_pass
    fi
else test_fail "could not install Red Hat response fixture"; fi

if [[ $_RH_READY -eq 1 ]]; then
    "$_RH_RUNTIME" exec "$_RH_CONTAINER" rm -f "$_RH_REMOTE"
fi
unset _RH_RUNTIME _RH_CONTAINER _RH_FIXTURE _RH_REMOTE _RH_READY

echo ""
