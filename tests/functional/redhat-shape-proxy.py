#!/usr/bin/env python3
"""Bugzilla proxy that reproduces observed production response and policy variants."""

import http.client
import http.server
import json
import signal
import socket
import sys
import threading
import unittest
import urllib.error
import urllib.parse
import urllib.request

_HOP_BY_HOP = frozenset(
    {"connection", "keep-alive", "proxy-authenticate", "proxy-authorization",
     "te", "trailers", "transfer-encoding", "upgrade", "host", "content-length"}
)
_MAX_REQUEST_BODY = 1024 * 1024


def is_termless_bug_search(path):
    """Return whether a REST bug search has no Bugzilla search criterion."""
    parsed = urllib.parse.urlsplit(path)
    if parsed.path != "/rest/bug":
        return False
    ignored = {
        "bugzilla_api_key", "include_fields", "exclude_fields", "limit", "offset", "order"
    }
    return not any(
        name.casefold() not in ignored and value
        for name, value in urllib.parse.parse_qsl(parsed.query, keep_blank_values=True)
    )


def shape_bug_response(data):
    """Return JSON bytes with bug component/version values represented as arrays."""
    value = json.loads(data)
    bugs = value.get("bugs") if isinstance(value, dict) else None
    if isinstance(bugs, list):
        for bug in bugs:
            if not isinstance(bug, dict):
                continue
            for field in ("component", "version"):
                field_value = bug.get(field)
                if isinstance(field_value, str):
                    values = [] if field_value == "" else [field_value]
                    if values:
                        values.append(f"{field_value}-redhat-secondary")
                    bug[field] = values
    return json.dumps(value, separators=(",", ":")).encode()


def shape_product_ids_response(data):
    """Return JSON bytes with product IDs represented as decimal strings.

    Some Bugzilla stacks (e.g. bugzilla.kernel.org) serialize
    `get_{accessible,selectable,enterable}_products` ids as strings rather than
    numbers. This rewrites every `ids` element to its decimal string form so the
    client is exercised against the string wire shape the same endpoint serves
    in the wild.
    """
    value = json.loads(data)
    if isinstance(value, dict) and isinstance(value.get("ids"), list):
        value["ids"] = [str(item) for item in value["ids"]]
    return json.dumps(value, separators=(",", ":")).encode()


def shape_metadata_sort_keys_response(path, data):
    """Return production-shaped signed metadata ordering weights and count."""
    parsed_path = urllib.parse.urlsplit(path).path
    is_field = parsed_path.startswith("/rest/field/bug")
    is_product = parsed_path == "/rest/product" or parsed_path.startswith(
        "/rest/product/"
    )
    if not is_field and not is_product:
        return data, 0

    value = json.loads(data)
    candidates = []
    if is_field and isinstance(value, dict):
        fields = value.get("fields")
        if isinstance(fields, list):
            for field in fields:
                values = field.get("values") if isinstance(field, dict) else None
                if isinstance(values, list):
                    candidates.extend(values)
    elif is_product and isinstance(value, dict):
        products = value.get("products")
        if isinstance(products, list):
            for product in products:
                if not isinstance(product, dict):
                    continue
                for name in ("versions", "milestones"):
                    items = product.get(name)
                    if isinstance(items, list):
                        candidates.extend(items)

    changed = 0
    cycle = (-1, 0, 1)
    for candidate in candidates:
        if isinstance(candidate, dict) and "sort_key" in candidate:
            candidate["sort_key"] = cycle[changed % len(cycle)]
            changed += 1
    if changed == 0:
        return data, 0
    return json.dumps(value, separators=(",", ":")).encode(), changed


def make_handler(backend_port):
    class Handler(http.server.BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def do_GET(self):
            if self.path == "/_bzr_ready":
                self.send_response(204)
                self.send_header("Content-Length", "0")
                self.end_headers()
                return

            if is_termless_bug_search(self.path):
                body = json.dumps({"code": 1000, "error": True,
                                   "message": "You may not search without any search terms."},
                                  separators=(",", ":")).encode()
                self.send_response(400)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)
                return

            self._forward("GET")

        def do_POST(self):
            self._forward("POST")

        def _forward(self, method):
            headers = {key: value for key, value in self.headers.items()
                       if key.lower() not in _HOP_BY_HOP}
            raw_content_length = self.headers.get("Content-Length", "0")
            try:
                content_length = int(raw_content_length)
            except ValueError:
                self.send_error(400, "Invalid Content-Length")
                return
            if content_length < 0:
                self.send_error(400, "Invalid Content-Length")
                return
            if content_length > _MAX_REQUEST_BODY:
                self.send_error(413, "Request body exceeds 1 MiB limit")
                return
            request_body = self.rfile.read(content_length) if content_length else None
            conn = http.client.HTTPConnection("127.0.0.1", backend_port, timeout=30)
            try:
                conn.request(method, self.path, body=request_body, headers=headers)
                response = conn.getresponse()
                body = response.read()
                response_headers = response.getheaders()
                status = response.status
            except (OSError, http.client.HTTPException) as error:
                self.send_error(502, f"Bugzilla backend unavailable: {error}")
                return
            finally:
                conn.close()

            if 200 <= status < 300 and self.path.startswith("/rest/bug"):
                try:
                    body = shape_bug_response(body)
                except (UnicodeDecodeError, json.JSONDecodeError) as error:
                    self.send_error(502, f"Bugzilla returned malformed JSON: {error}")
                    return

            if 200 <= status < 300 and self.path.startswith(
                ("/rest/product_accessible", "/rest/product_selectable",
                 "/rest/product_enterable")
            ):
                try:
                    body = shape_product_ids_response(body)
                except (UnicodeDecodeError, json.JSONDecodeError) as error:
                    self.send_error(502, f"Bugzilla returned malformed JSON: {error}")
                    return

            if 200 <= status < 300:
                try:
                    body, changed = shape_metadata_sort_keys_response(self.path, body)
                except (UnicodeDecodeError, json.JSONDecodeError) as error:
                    self.send_error(502, f"Bugzilla returned malformed JSON: {error}")
                    return
                if changed:
                    route = "field" if urllib.parse.urlsplit(self.path).path.startswith(
                        "/rest/field/bug"
                    ) else "product"
                    sys.stderr.write(
                        f"metadata-sort-keys shaped route={route} count={changed}\n"
                    )
                    sys.stderr.flush()

            self.send_response(status)
            for key, value in response_headers:
                if key.lower() not in _HOP_BY_HOP:
                    self.send_header(key, value)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, *args):
            pass

    return Handler


class ShapeTests(unittest.TestCase):
    def test_identifies_termless_bug_search_without_matching_scoped_or_detail_reads(self):
        self.assertTrue(is_termless_bug_search("/rest/bug?limit=1&include_fields=id"))
        self.assertTrue(is_termless_bug_search("/rest/bug?product=&limit=1"))
        self.assertTrue(is_termless_bug_search(
            "/rest/bug?Bugzilla_api_key=secret&limit=1"
        ))
        self.assertFalse(is_termless_bug_search("/rest/bug?id=123&limit=1"))
        self.assertFalse(is_termless_bug_search("/rest/bug/123?include_fields=id"))

    def test_termless_search_returns_production_shaped_code_1000(self):
        server, thread = self._start_server(1)
        try:
            with self.assertRaises(urllib.error.HTTPError) as error:
                urllib.request.urlopen(
                    f"http://127.0.0.1:{server.server_port}/rest/bug?limit=1",
                    timeout=2,
                )
            self.assertEqual(error.exception.code, 400)
            self.assertEqual(json.load(error.exception)["code"], 1000)
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=2)

    def test_shapes_scalar_empty_and_multi_values(self):
        shaped = json.loads(shape_bug_response(json.dumps({"bugs": [
            {"component": "Backend", "version": "rawhide"},
            {"component": [], "version": ["40", "41"]},
            {"component": "", "version": ""},
        ]}).encode()))
        self.assertEqual(
            shaped["bugs"][0]["component"],
            ["Backend", "Backend-redhat-secondary"],
        )
        self.assertEqual(
            shaped["bugs"][0]["version"], ["rawhide", "rawhide-redhat-secondary"]
        )
        self.assertEqual(shaped["bugs"][1]["version"], ["40", "41"])
        self.assertEqual(shaped["bugs"][2]["component"], [])

    def test_leaves_non_bug_payload_untouched(self):
        payload = b'{"version":"5.2"}'
        self.assertEqual(json.loads(shape_bug_response(payload)), {"version": "5.2"})

    def test_rejects_malformed_json(self):
        with self.assertRaises(json.JSONDecodeError):
            shape_bug_response(b"not json")

    def test_products_shape_to_string_ids(self):
        shaped = json.loads(shape_product_ids_response(
            json.dumps({"ids": [12, 1, 21]}).encode()
        ))
        self.assertEqual(shaped, {"ids": ["12", "1", "21"]})

    def test_products_shape_preserves_existing_string_ids(self):
        payload = b'{"ids":["2","3","19"]}'
        self.assertEqual(
            json.loads(shape_product_ids_response(payload)), {"ids": ["2", "3", "19"]}
        )

    def test_products_shape_leaves_non_ids_payload_untouched(self):
        payload = b'{"products":[]}'
        self.assertEqual(
            json.loads(shape_product_ids_response(payload)), {"products": []}
        )

    def test_shapes_field_metadata_sort_keys_with_signed_cycle(self):
        payload = json.dumps({"fields": [{
            "id": 10,
            "name": "bug_status",
            "values": [
                {"id": 101, "name": "NEW", "sort_key": 100},
                {"id": 102, "name": "ASSIGNED", "sort_key": 200},
                {"id": 103, "name": "CLOSED", "sort_key": 300},
            ],
        }]}).encode()
        body, count = shape_metadata_sort_keys_response(
            "/rest/field/bug/status", payload
        )
        shaped = json.loads(body)
        self.assertEqual(count, 3)
        self.assertEqual(
            [item["sort_key"] for item in shaped["fields"][0]["values"]],
            [-1, 0, 1],
        )
        self.assertEqual(
            [item["id"] for item in shaped["fields"][0]["values"]],
            [101, 102, 103],
        )

    def test_shapes_product_metadata_sort_keys_with_signed_cycle(self):
        payload = json.dumps({"products": [{
            "id": 10,
            "versions": [
                {"id": 201, "sort_key": 100},
                {"id": 202, "sort_key": 200},
            ],
            "milestones": [{"id": 301, "sort_key": 300}],
        }]}).encode()
        body, count = shape_metadata_sort_keys_response("/rest/product", payload)
        shaped = json.loads(body)
        self.assertEqual(count, 3)
        self.assertEqual(
            [item["sort_key"] for item in shaped["products"][0]["versions"]],
            [-1, 0],
        )
        self.assertEqual(
            shaped["products"][0]["milestones"][0]["sort_key"], 1
        )
        self.assertEqual(shaped["products"][0]["id"], 10)

    def test_metadata_sort_key_shape_leaves_unrelated_payload_untouched(self):
        payload = b'{"products":[{"id":10,"sort_key":99}]}'
        for path in ["/rest/version", "/rest/product"]:
            body, count = shape_metadata_sort_keys_response(path, payload)
            self.assertEqual(body, payload)
            self.assertEqual(count, 0)

    def test_readiness_endpoint(self):
        server, thread = self._start_server(1)
        try:
            response = urllib.request.urlopen(
                f"http://127.0.0.1:{server.server_port}/_bzr_ready", timeout=2
            )
            self.assertEqual(response.status, 204)
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=2)

    def test_unavailable_backend_returns_502(self):
        blocker = socket.socket()
        blocker.bind(("127.0.0.1", 0))
        unavailable_port = blocker.getsockname()[1]
        blocker.close()
        server, thread = self._start_server(unavailable_port)
        try:
            with self.assertRaises(urllib.error.HTTPError) as error:
                urllib.request.urlopen(
                    f"http://127.0.0.1:{server.server_port}/rest/version", timeout=2
                )
            self.assertEqual(error.exception.code, 502)
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=2)

    def test_rejects_invalid_and_oversized_content_length(self):
        server, thread = self._start_server(1)
        try:
            for content_length, expected_status in (("invalid", 400), ("1048577", 413)):
                connection = http.client.HTTPConnection(
                    "127.0.0.1", server.server_port, timeout=2
                )
                connection.putrequest("POST", "/rest/bug")
                connection.putheader("Content-Length", content_length)
                connection.endheaders()
                response = connection.getresponse()
                self.assertEqual(response.status, expected_status)
                response.read()
                connection.close()
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=2)

    @staticmethod
    def _start_server(backend_port):
        server = http.server.ThreadingHTTPServer(
            ("127.0.0.1", 0), make_handler(backend_port)
        )
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        return server, thread


def main():
    if sys.argv[1:] == ["--self-test"]:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(ShapeTests)
        return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1
    if len(sys.argv) != 3:
        sys.stderr.write("usage: redhat-shape-proxy.py <listen_port> <backend_port>\n")
        return 2
    server = http.server.ThreadingHTTPServer(
        ("127.0.0.1", int(sys.argv[1])), make_handler(int(sys.argv[2]))
    )
    def stop_server(_signum, _frame):
        raise KeyboardInterrupt

    signal.signal(signal.SIGTERM, stop_server)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    return 0


if __name__ == "__main__":
    sys.exit(main())
