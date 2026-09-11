# 16c-attachments-large
# Sourced by run-tests.sh; uses its private temporary directory and cleanup.
# shellcheck shell=bash

echo "── Phase 16c: Large attachment streaming ───────────────────"

# Seed through the API, then grow the stored bytes: download support must not
# depend on this fixture server's deliberately small upload-size policy.
prepare_large_attachments() {
	run_bzr bug create --product FuncTestProd --component Backend \
		--summary "Large attachment fixture" --description "Streaming downloads" \
		--op-sys All --platform All
	assert_success || return 1
	LARGE_BUG=$(jq -r '.id' "$BZR_STDOUT")
	run_bzr attachment upload "$LARGE_BUG" "$FUNC_ATTACH_FILE" --summary "Large public"
	assert_success || return 1
	LARGE_PUBLIC=$(jq -r '.id' "$BZR_STDOUT")
	run_bzr attachment upload "$LARGE_BUG" "$FUNC_ATTACH_FILE" --summary "Large private" --private
	assert_success || return 1
	LARGE_PRIVATE=$(jq -r '.id' "$BZR_STDOUT")
	[[ "$LARGE_PUBLIC" =~ ^[0-9]+$ && "$LARGE_PRIVATE" =~ ^[0-9]+$ ]] || return 1
	local sql="$FUNC_CONFIG_DIR/large-attachments.sql"
	# max_allowed_packet is inherited by new connections, hence two calls.
	printf 'SET GLOBAL max_allowed_packet=134217728;\n' >"$sql"
	run_bugzilla_sql_file "$sql" || return 1
	printf 'UPDATE attach_data SET thedata=REPEAT("A",49*1024*1024) WHERE id IN (%s,%s);\n' \
		"$LARGE_PUBLIC" "$LARGE_PRIVATE" >"$sql"
	run_bugzilla_sql_file "$sql" || return 1
	python3 - "$FUNC_CONFIG_DIR/large-expected" <<'PY'
import sys
with open(sys.argv[1], "wb") as stream:
    for _ in range(49):
        stream.write(b"A" * (1024 * 1024))
PY
}

# Compare bytes as well as exit status: an accidentally truncated successful
# response must fail the same test as an explicit transport error.
large_downloads_match() {
	local shape="$1" mode id destination
	for mode in rest hybrid xmlrpc; do
		destination="$FUNC_CONFIG_DIR/large-$mode-$shape"
		case "$shape" in
		file | stdout)
			for id in "$LARGE_PUBLIC" "$LARGE_PRIVATE"; do
				if [[ "$shape" == stdout ]]; then
					run_bzr_raw --api "$mode" attachment download "$id" --out -
					destination="$BZR_STDOUT_RAW"
				else
					run_bzr --api "$mode" attachment download "$id" --out "$destination"
				fi
				assert_success || return 1
				if ! cmp -s "$FUNC_CONFIG_DIR/large-expected" "$destination"; then
					test_fail "$mode $shape returned incorrect large attachment bytes"
					return 1
				fi
			done
			;;
		positional | bug)
			if [[ "$shape" == positional ]]; then
				run_bzr --api "$mode" attachment download "$LARGE_PUBLIC" "$LARGE_PRIVATE" --out-dir "$destination"
			else
				run_bzr --api "$mode" attachment download --bug "$LARGE_BUG" --out-dir "$destination"
			fi
			assert_success || return 1
			for id in "$LARGE_PUBLIC" "$LARGE_PRIVATE"; do
				if ! cmp -s "$FUNC_CONFIG_DIR/large-expected" "$destination/$LARGE_BUG/$id.attach.txt"; then
					test_fail "$mode $shape returned incorrect large attachment bytes"
					return 1
				fi
			done
			;;
		esac
	done
}

test_begin "large-attachment-fixture" "prepare public/private attachments above the former 48 MiB ceiling"
if prepare_large_attachments; then
	test_pass
else
	test_fail "could not prepare large attachment fixture"
	return 1
fi

test_begin "large-attachment-files" "large public/private file downloads in REST, Hybrid and XML-RPC"
if large_downloads_match file; then test_pass; fi

test_begin "large-attachment-stdout" "large public/private stdout downloads in REST, Hybrid and XML-RPC"
if large_downloads_match stdout; then test_pass; fi

test_begin "large-attachment-positional" "large public/private positional batch in all API modes"
if large_downloads_match positional; then test_pass; fi

test_begin "large-attachment-bug" "large public/private bug batch in all API modes"
if large_downloads_match bug; then test_pass; fi

test_begin "large-attachment-anonymous" "anonymous large public download preserves bytes"
large_anonymous_ok=true
for large_mode in rest hybrid xmlrpc; do
	run_bzr_raw --server-url "$BZ_URL" --api "$large_mode" attachment download "$LARGE_PUBLIC" --out -
	if ! assert_success; then
		large_anonymous_ok=false
		break
	fi
	if ! cmp -s "$FUNC_CONFIG_DIR/large-expected" "$BZR_STDOUT_RAW"; then
		test_fail "$large_mode anonymous download returned incorrect bytes"
		large_anonymous_ok=false
		break
	fi
done
if [[ "$large_anonymous_ok" == true ]]; then test_pass; fi

test_begin "large-attachment-anonymous-batch" "anonymous large bug batch includes public bytes and excludes private data"
large_anonymous_ok=true
for large_mode in rest hybrid xmlrpc; do
	large_destination="$FUNC_CONFIG_DIR/large-anonymous-$large_mode"
	run_bzr --server-url "$BZ_URL" --api "$large_mode" attachment download --bug "$LARGE_BUG" --out-dir "$large_destination"
	if ! assert_success; then
		large_anonymous_ok=false
		break
	fi
	if ! cmp -s "$FUNC_CONFIG_DIR/large-expected" "$large_destination/$LARGE_BUG/$LARGE_PUBLIC.attach.txt" ||
		[[ -e "$large_destination/$LARGE_BUG/$LARGE_PRIVATE.attach.txt" ]]; then
		test_fail "$large_mode anonymous batch returned incorrect bytes or private data"
		large_anonymous_ok=false
		break
	fi
done
if [[ "$large_anonymous_ok" == true ]]; then test_pass; fi
