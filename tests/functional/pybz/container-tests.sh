#!/bin/bash
# Focused fixtures for the python-bugzilla comparison sidecar.
set -euo pipefail

PYBZ_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=tests/functional/lib.sh
source "$PYBZ_DIR/../lib.sh"

assert_equals() {
    local expected="$1"
    local actual="$2"
    local label="$3"

    if [[ $actual != "$expected" ]]; then
        printf 'expected %s to be %s, got %s\n' "$label" "$expected" "$actual" >&2
        return 1
    fi
    return 0
}

run_expected_gap_fixture() {
    local summary
    local result_output
    summary=$(mktemp)
    result_output=$(mktemp)
    trap 'rm -f "$summary" "$result_output"' RETURN
    export GITHUB_STEP_SUMMARY="$summary"
    TEST_ID_PREFIX=compare
    BZ_VERSION=bz50

    CURRENT_TEST_GROUP="20-pybz"
    {
        test_begin "expected-gap" "expected client gap"
        test_fail "known comparison difference"
        expect_gap 666
    } >"$result_output"
    assert_equals \
        '  TEST  [compare/20-pybz/expected-gap] expected client gap ... GAP (#666)' \
        "$(<"$result_output")" "expected-gap terminal output"
    assert_equals 0 "$PASS_COUNT" "pass count"
    assert_equals 0 "$FAIL_COUNT" "fail count"
    assert_equals 0 "$SKIP_COUNT" "skip count"
    assert_equals 1 "$GAP_COUNT" "gap count"
    if ! test_summary; then
        printf 'expected-gap-only summary failed\n' >&2
        return 1
    fi
    assert_equals $'## bzr/python-bugzilla comparison summary\n\n| Bugzilla | Passed | Failed | Skipped | Expected gaps |\n| --- | ---: | ---: | ---: | ---: |\n| bz50 | 0 | 0 | 0 | 1 |' \
        "$(<"$summary")" "comparison GitHub summary"

    if expect_gap 667; then
        printf 'expected gap was accepted twice\n' >&2
        return 1
    fi
    if expect_gap not-a-decimal-issue; then
        printf 'non-decimal issue was accepted\n' >&2
        return 1
    fi

    PASS_COUNT=0
    FAIL_COUNT=0
    SKIP_COUNT=0
    GAP_COUNT=0
    CURRENT_TEST_GROUP="20-pybz"
    {
        test_begin "stale-gap" "stale expected client gap"
        test_pass
        expect_gap 666
    } >"$result_output"
    assert_equals \
        '  TEST  [compare/20-pybz/stale-gap] stale expected client gap ... FAIL  (expected gap issue #666 appears resolved)' \
        "$(<"$result_output")" "stale-gap terminal output"
    assert_equals 0 "$PASS_COUNT" "stale pass count"
    assert_equals 1 "$FAIL_COUNT" "stale fail count"
    assert_equals 0 "$SKIP_COUNT" "stale skip count"
    assert_equals 0 "$GAP_COUNT" "stale gap count"
    if test_summary; then
        printf 'stale expected gap summary unexpectedly passed\n' >&2
        return 1
    fi
    return 0
}

run_summary_fixture() {
    local summary
    local ordinary_output
    summary=$(mktemp)
    trap 'rm -f "$summary"' RETURN
    export GITHUB_STEP_SUMMARY="$summary"

    PASS_COUNT=1
    FAIL_COUNT=0
    SKIP_COUNT=2
    GAP_COUNT=3
    TEST_ID_PREFIX=''
    BZ_VERSION=bz50
    ordinary_output=$(test_summary)
    assert_equals $'\n════════════════════════════════════════════════════════════\n  PASSED: 1  FAILED: 0  SKIPPED: 2\n  TOTAL:  3\n════════════════════════════════════════════════════════════' \
        "$ordinary_output" "ordinary terminal summary"
    assert_equals '' "$(<"$summary")" "ordinary GitHub summary"

    : >"$summary"
    TEST_ID_PREFIX=compare
    PASS_COUNT=1
    FAIL_COUNT=0
    SKIP_COUNT=2
    GAP_COUNT=3
    for BZ_VERSION in bz50 bz52 bz53; do
        test_summary >/dev/null
    done
    assert_equals $'## bzr/python-bugzilla comparison summary\n\n| Bugzilla | Passed | Failed | Skipped | Expected gaps |\n| --- | ---: | ---: | ---: | ---: |\n| bz50 | 1 | 0 | 2 | 3 |\n\n## bzr/python-bugzilla comparison summary\n\n| Bugzilla | Passed | Failed | Skipped | Expected gaps |\n| --- | ---: | ---: | ---: | ---: |\n| bz52 | 1 | 0 | 2 | 3 |\n\n## bzr/python-bugzilla comparison summary\n\n| Bugzilla | Passed | Failed | Skipped | Expected gaps |\n| --- | ---: | ---: | ---: | ---: |\n| bz53 | 1 | 0 | 2 | 3 |' \
        "$(<"$summary")" "multi-version comparison GitHub summary"
    return 0
}

run_product_normalization_fixture() (
    local fixture_output
    COMPARE_EXCHANGE_DIR=$(mktemp -d)
    fixture_output=$(mktemp)
    trap 'rm -rf "$COMPARE_EXCHANGE_DIR"; rm -f "$fixture_output"' EXIT

    PASS_COUNT=0
    FAIL_COUNT=0
    SKIP_COUNT=0
    GAP_COUNT=0
    TEST_ID_PREFIX=compare
    CURRENT_TEST_GROUP=00-products
    BZ_URL=http://127.0.0.1

    run_bzr() {
        printf '%s\n' \
            '[{"name":"Beta"},{"name":""},{"name":"Alpha"},{"name":"Alpha"}]' \
            >"$BZR_STDOUT"
        cp "$BZR_STDOUT" "$BZR_STDOUT_RAW"
        : >"$BZR_STDERR"
        BZR_EXIT=0
    }
    run_pybz() {
        printf '\nAlpha\nBeta\nBeta\n' >"$BZR_STDOUT"
        cp "$BZR_STDOUT" "$BZR_STDOUT_RAW"
        : >"$BZR_STDERR"
        BZR_EXIT=0
    }

    # shellcheck source=tests/functional/compare/00-products.sh
    source "$PYBZ_DIR/../compare/00-products.sh" >"$fixture_output"
    assert_equals 1 "$PASS_COUNT" "normalized product pass count"
    assert_equals 0 "$FAIL_COUNT" "normalized product fail count"
    assert_equals $'Alpha\nBeta' "$(<"$COMPARE_EXCHANGE_DIR/bzr-product-names")" \
        "normalized bzr product names"
    assert_equals $'Alpha\nBeta' "$(<"$COMPARE_EXCHANGE_DIR/pybz-product-names")" \
        "normalized python-bugzilla product names"
)

cleanup_container_fixture() {
    local runtime="$1"
    local donor="$2"
    local config_dir="$3"

    pybz_sidecar_stop "$runtime"
    if "$runtime" container inspect "$donor" >/dev/null 2>&1; then
        "$runtime" rm -f "$donor" >/dev/null
    fi
    rm -rf "$config_dir"
    return 0
}

run_container_fixture() {
    local runtime
    local checkout_id
    local fixture_image
    local donor
    local config_dir
    local package_version
    local cli_version
    local sidecar
    local sidecar_id
    local collision_error
    local collision_status
    local replacement_id

    runtime=$(container_runtime) || {
        printf 'no container runtime available\n' >&2
        return 1
    }
    checkout_id=$(bugzilla_checkout_id)
    fixture_image="localhost/bzr-pybz-fixture-${checkout_id}:3.3.0"
    donor="bzr-pybz-fixture-${checkout_id}"
    config_dir=$(mktemp -d)
    export FUNC_CONFIG_DIR="$config_dir"
    BZ_VERSION="bz50"
    trap 'cleanup_container_fixture "$runtime" "$donor" "$config_dir"' RETURN

    "$runtime" build -t "$fixture_image" -f "$PYBZ_DIR/Containerfile" "$PYBZ_DIR"
    package_version=$("$runtime" run --rm "$fixture_image" python -c \
        'from importlib.metadata import version; print(version("python-bugzilla"))')
    assert_equals 3.3.0 "$package_version" "python-bugzilla version"
    cli_version=$("$runtime" run --rm "$fixture_image" bugzilla --version)
    if [[ $cli_version != *3.3.0* ]]; then
        printf 'bugzilla CLI did not report version 3.3.0\n' >&2
        return 1
    fi

    if "$runtime" container inspect "$donor" >/dev/null 2>&1; then
        "$runtime" rm -f "$donor" >/dev/null
    fi
    "$runtime" run -d --name "$donor" "$fixture_image" >/dev/null
    pybz_sidecar_start "$runtime" "$donor"

    sidecar=$(pybz_sidecar_name)
    sidecar_id=$("$runtime" container inspect --format '{{.Id}}' "$sidecar")
    collision_error="$config_dir/running-sidecar.stderr"
    PYBZ_RUNTIME=''
    set +e
    pybz_sidecar_start "$runtime" "$donor" 2>"$collision_error"
    collision_status=$?
    set -e
    assert_equals 1 "$collision_status" "running sidecar collision status"
    assert_equals "$sidecar_id" \
        "$("$runtime" container inspect --format '{{.Id}}' "$sidecar")" \
        "running sidecar identity"
    assert_equals '' "$PYBZ_RUNTIME" "running sidecar ownership"
    if ! grep -Fq "sidecar is already running: $sidecar" "$collision_error"; then
        printf 'running sidecar collision omitted its actionable diagnostic\n' >&2
        return 1
    fi
    "$runtime" stop "$sidecar" >/dev/null
    pybz_sidecar_start "$runtime" "$donor"
    replacement_id=$("$runtime" container inspect --format '{{.Id}}' "$sidecar")
    if [[ $replacement_id == "$sidecar_id" ]]; then
        printf 'stopped sidecar was not replaced\n' >&2
        return 1
    fi
    assert_equals true \
        "$("$runtime" container inspect --format '{{.State.Running}}' "$sidecar")" \
        "replacement sidecar running state"

    run_pybz --version
    assert_success
    run_pybz --definitely-invalid-option
    assert_failure

    "$runtime" exec "$sidecar" sh -c "printf '%s' exchange-proof > /work/proof"
    assert_equals exchange-proof "$(<"$config_dir/proof")" "bind-mount bytes"
    return 0
}

run_expected_gap_fixture
run_summary_fixture
run_product_normalization_fixture
run_container_fixture
