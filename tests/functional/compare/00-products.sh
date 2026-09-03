#!/bin/bash
# First real bzr/python-bugzilla semantic comparison phase.
# Sourced by run-compare.sh; relies on its isolated configuration and sidecar.

test_begin "list-products" "product list"
run_bzr --server-url "$BZ_URL" product list
if [[ $BZR_EXIT -ne 0 ]]; then
    test_fail "bzr product list failed with exit $BZR_EXIT"
else
    cp "$BZR_STDOUT" "$COMPARE_EXCHANGE_DIR/bzr-products.json"
    cp "$BZR_STDOUT_RAW" "$COMPARE_EXCHANGE_DIR/bzr-products.raw"
    cp "$BZR_STDERR" "$COMPARE_EXCHANGE_DIR/bzr-products.stderr"
    printf '%s\n' "$BZR_EXIT" >"$COMPARE_EXCHANGE_DIR/bzr-products.exit"

    if ! jq -r '.[].name' "$COMPARE_EXCHANGE_DIR/bzr-products.json" |
        awk 'NF' | LC_ALL=C sort -u >"$COMPARE_EXCHANGE_DIR/bzr-product-names"; then
        test_fail "could not normalize bzr product names"
    else
        run_pybz --bugzilla http://127.0.0.1 info --products
        if [[ $BZR_EXIT -ne 0 ]]; then
            test_fail "python-bugzilla product list failed with exit $BZR_EXIT"
        else
            cp "$BZR_STDOUT" "$COMPARE_EXCHANGE_DIR/pybz-products.txt"
            cp "$BZR_STDERR" "$COMPARE_EXCHANGE_DIR/pybz-products.stderr"
            printf '%s\n' "$BZR_EXIT" >"$COMPARE_EXCHANGE_DIR/pybz-products.exit"

            if awk 'NF' "$COMPARE_EXCHANGE_DIR/pybz-products.txt" | LC_ALL=C sort -u \
                >"$COMPARE_EXCHANGE_DIR/pybz-product-names" &&
                diff -u "$COMPARE_EXCHANGE_DIR/bzr-product-names" \
                    "$COMPARE_EXCHANGE_DIR/pybz-product-names"; then
                test_pass
            else
                test_fail "normalized product-name lists differ"
            fi
        fi
    fi
fi
