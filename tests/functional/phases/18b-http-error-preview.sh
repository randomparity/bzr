# 18b-http-error-preview
# Sourced by run-tests.sh in order; assumes lib.sh helpers and the
# orchestrator preamble (constants, shared globals, cleanup trap).
# Reads: none. Creates: two loopback HTTP fixture processes.
# shellcheck shell=bash

# ═════════════════════════════════════════════════════════════
# Phase 18b: Bounded HTTP error output (issues #512, #740)
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

test_begin "oversized-response-body-is-refused" "oversized response body is refused (#740)"
if ! command -v python3 >/dev/null 2>&1; then
	test_fail "python3 is required for the oversized-response fixture"
	echo ""
	return 0
fi

# The fixture streams a fixed 96 MiB budget — 1.5x bzr's 64 MiB response-body
# limit — and then closes. The budget is load-bearing: run_bzr is wrapped in no
# timeout, so an unbounded writer against a client whose bound regressed would
# buffer until the host ran out of memory instead of failing the assertion.
_oversize_port_file=$(mktemp /tmp/bzr-func-oversize-port.XXXXXX)
python3 -c '
import http.server
import sys
import threading

version = b"{\"version\":\"5.0.4\"}"
filler = b"a" * 65536
budget = 96 * 1024 * 1024

class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def do_GET(self):
        if self.path.startswith("/rest/version"):
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(version)))
            self.end_headers()
            self.wfile.write(version)
            return
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(budget))
        self.end_headers()
        sent = 0
        try:
            while sent < budget:
                self.wfile.write(filler)
                sent += len(filler)
        except (BrokenPipeError, ConnectionResetError):
            pass

    def log_message(self, _format, *args):
        pass

class Server(http.server.ThreadingHTTPServer):
    # bzr resetting the connection the moment it refuses the body is the
    # expected outcome here, not an error worth a traceback in the phase log.
    def handle_error(self, request, client_address):
        pass

server = Server(("127.0.0.1", 0), Handler)
# The harness kills this fixture explicitly, but its cleanup trap does not know
# the PID. Without a watchdog an orphan would keep serving 96 MiB per GET for as
# long as the host stayed up; 120s is far longer than the single request this
# case makes.
threading.Timer(120, server.shutdown).start()
with open(sys.argv[1], "w", encoding="utf-8") as port_file:
    port_file.write(str(server.server_address[1]))
server.serve_forever()
' "$_oversize_port_file" &
_oversize_pid=$!

for _attempt in {1..50}; do
	if [[ -s "$_oversize_port_file" ]]; then
		break
	fi
	sleep 0.1
done

if [[ ! -s "$_oversize_port_file" ]]; then
	test_fail "oversized-response fixture did not become ready"
else
	_oversize_port=$(<"$_oversize_port_file")
	run_bzr --server-url "http://127.0.0.1:${_oversize_port}" --api rest server info
	if assert_exit_code 16 &&
		assert_stderr_json '.error.type' "response_too_large" &&
		assert_stderr_json '.error.exit_code' "16" &&
		assert_stderr_json '.error.limit_bytes' "67108864"; then
		test_pass
	fi
fi

if kill -0 "$_oversize_pid" 2>/dev/null; then
	kill "$_oversize_pid"
	wait "$_oversize_pid" 2>/dev/null || :
fi
rm -f "$_oversize_port_file"
echo ""

# bzr schema <name> writes the schema verbatim with no envelope, so the jq path
# starts at the schema root rather than at .data.
test_begin "schema-error-admits-exit-16" "published error schema admits exit 16 (#740)"
run_bzr_raw schema error
if assert_success &&
	assert_raw_json '.properties.error.properties.exit_code.maximum' "16"; then
	test_pass
fi
echo ""
