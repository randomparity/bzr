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

assert_summary_counter() {
    local summary="$1"
    local label="$2"
    local count="$3"

    if ! grep -F "| $label | $count |" "$summary" >/dev/null; then
        printf 'missing %s counter in GitHub summary\n' "$label" >&2
        return 1
    fi
    return 0
}

run_expected_gap_fixture() {
    local summary
    summary=$(mktemp)
    trap 'rm -f "$summary"' RETURN
    export GITHUB_STEP_SUMMARY="$summary"

    CURRENT_TEST_GROUP="20-pybz"
    test_begin "expected-gap" "expected client gap"
    test_fail "known comparison difference"
    expect_gap 666
    assert_equals 0 "$PASS_COUNT" "pass count"
    assert_equals 0 "$FAIL_COUNT" "fail count"
    assert_equals 0 "$SKIP_COUNT" "skip count"
    assert_equals 1 "$GAP_COUNT" "gap count"
    if ! test_summary; then
        printf 'expected-gap-only summary failed\n' >&2
        return 1
    fi
    assert_summary_counter "$summary" "Passed" 0
    assert_summary_counter "$summary" "Failed" 0
    assert_summary_counter "$summary" "Skipped" 0
    assert_summary_counter "$summary" "Gaps" 1

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
    test_begin "stale-gap" "stale expected client gap"
    test_pass
    expect_gap 666
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

    run_pybz --version
    assert_success
    run_pybz --definitely-invalid-option
    assert_failure

    sidecar=$(pybz_sidecar_name)
    "$runtime" exec "$sidecar" sh -c "printf '%s' exchange-proof > /work/proof"
    assert_equals exchange-proof "$(<"$config_dir/proof")" "bind-mount bytes"
    return 0
}

run_expected_gap_fixture
run_container_fixture
