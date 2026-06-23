#!/usr/bin/env python3
"""TLS-terminating reverse proxy for bzr functional tests.

Fronts the HTTP-only Bugzilla functional container with HTTPS so the ad-hoc
``--server-tls-*`` trust flags can be exercised against a real Bugzilla server.
Terminates TLS with the supplied cert/key and forwards each request verbatim to
the plaintext HTTP backend.

Usage:
    tls-proxy.py <listen_port> <backend_host> <backend_port> <certfile> <keyfile>

Stdlib only (ssl, http.server, http.client) so it runs anywhere python3 does,
with no third-party packages.
"""

import http.client
import http.server
import ssl
import sys

# Hop-by-hop headers must not be forwarded across a proxy (RFC 7230 6.1).
# Content-Length is recomputed; Transfer-Encoding/Connection are connection-scoped.
_HOP_BY_HOP = frozenset(
    {
        "connection",
        "keep-alive",
        "proxy-authenticate",
        "proxy-authorization",
        "te",
        "trailers",
        "transfer-encoding",
        "upgrade",
        "host",
        "content-length",
    }
)


def _make_handler(backend_host, backend_port):
    class Handler(http.server.BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def _forward(self, method):
            length = int(self.headers.get("Content-Length", 0) or 0)
            body = self.rfile.read(length) if length else None
            headers = {
                k: v
                for k, v in self.headers.items()
                if k.lower() not in _HOP_BY_HOP
            }
            conn = http.client.HTTPConnection(backend_host, backend_port, timeout=30)
            try:
                conn.request(method, self.path, body=body, headers=headers)
                resp = conn.getresponse()
                data = resp.read()
            finally:
                conn.close()
            self.send_response(resp.status)
            for key, value in resp.getheaders():
                if key.lower() in _HOP_BY_HOP:
                    continue
                self.send_header(key, value)
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            if method != "HEAD":
                self.wfile.write(data)

        def do_GET(self):
            self._forward("GET")

        def do_HEAD(self):
            self._forward("HEAD")

        def do_POST(self):
            self._forward("POST")

        def do_PUT(self):
            self._forward("PUT")

        def log_message(self, *args):
            pass

    return Handler


def main():
    if len(sys.argv) != 6:
        sys.stderr.write(
            "usage: tls-proxy.py <listen_port> <backend_host> "
            "<backend_port> <certfile> <keyfile>\n"
        )
        return 2
    listen_port = int(sys.argv[1])
    backend_host = sys.argv[2]
    backend_port = int(sys.argv[3])
    certfile, keyfile = sys.argv[4], sys.argv[5]

    ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    ctx.load_cert_chain(certfile, keyfile)

    httpd = http.server.ThreadingHTTPServer(
        ("127.0.0.1", listen_port), _make_handler(backend_host, backend_port)
    )
    httpd.socket = ctx.wrap_socket(httpd.socket, server_side=True)
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        pass
    return 0


if __name__ == "__main__":
    sys.exit(main())
