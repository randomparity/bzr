test_begin "extensions" "RHBZ extensions are advertised"
if curl -fsS "$BZ_URL/rest/extensions" | jq -e '.extensions | has("ExternalBugs") and has("SubComponents") and has("RedHat")' >/dev/null; then
    test_pass
else
    test_fail "RHBZ did not advertise ExternalBugs, SubComponents, and RedHat"
fi
