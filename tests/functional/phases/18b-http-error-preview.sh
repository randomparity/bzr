# 18b-http-error-preview
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# Reads: none. Creates: one loopback HTTP fixture process.
# shellcheck shell=bash

# ═════════════════════════════════════════════════════════════
# Phase 18b: Bounded HTTP error output (issue #512)
# ═════════════════════════════════════════════════════════════
echo "── Phase 18b: Bounded HTTP error output ────────────"

test_begin "oversized-http-error-body-is-utf-8-safe-and-bounded" "oversized HTTP error body is UTF-8-safe and bounded (#512)"
if ! command -v python3 >/dev/null 2>&1; then
	test_fail "python3 is required for the HTTP error fixture"
	echo ""
	return 0
fi

_http_error_port_file=$(mktemp /tmp/bzr-func-http-error-port.XXXXXX)
python3 -c '
import http.server
import sys

body = ("a" * 511 + "é trailing").encode()

class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(502)
        self.send_header("Content-Type", "text/plain; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, _format, *args):
        pass

server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
with open(sys.argv[1], "w", encoding="utf-8") as port_file:
    port_file.write(str(server.server_address[1]))
server.serve_forever()
' "$_http_error_port_file" &
_http_error_pid=$!

for _attempt in {1..50}; do
	if [[ -s "$_http_error_port_file" ]]; then
		break
	fi
	sleep 0.1
done

if [[ ! -s "$_http_error_port_file" ]]; then
	test_fail "HTTP error fixture did not become ready"
else
	_http_error_port=$(<"$_http_error_port_file")
	run_bzr --server-url "http://127.0.0.1:${_http_error_port}" --api rest server info
	if assert_exit_code 5 &&
		assert_stderr_json '.error.type' "http" &&
		assert_stderr_json '.error.exit_code' "5" &&
		assert_stderr_json '.error.message | length' "522" &&
		assert_stderr_json '.error.message | endswith("…")' "true"; then
		test_pass
	fi
fi

if kill -0 "$_http_error_pid" 2>/dev/null; then
	kill "$_http_error_pid"
	wait "$_http_error_pid" 2>/dev/null || :
fi
rm -f "$_http_error_port_file"
echo ""
