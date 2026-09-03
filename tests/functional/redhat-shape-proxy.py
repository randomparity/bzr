#!/usr/bin/env python3
"""Proxy observed Bugzilla response variants for functional conformance tests.

Add a rewrite to ``REWRITE_HOOKS`` as a ``(matcher, transformer)`` pair. A
matcher receives ``method, path, enabled_modes``; its transformer receives the
same request metadata and response bytes and returns ``(bytes, evidence)``.
Evidence is emitted after all matching hooks, preserving registry order.
"""

import http.client
import http.server
import json
import os
import re
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


def shape_user_group_response(method, path, data):
    """Return explicit user/group production shapes, route, and changed count."""
    route_path = urllib.parse.urlsplit(path).path
    route = None
    if method == "GET" and route_path == "/rest/whoami":
        route = "whoami"
    elif method == "GET" and (
        route_path == "/rest/user" or route_path.startswith("/rest/user/")
    ):
        route = "user-read"
    elif method == "GET" and (
        route_path == "/rest/group" or route_path.startswith("/rest/group/")
    ):
        route = "group-read"
    elif method == "POST" and route_path == "/rest/user":
        route = "user-create"
    elif method == "POST" and route_path == "/rest/group":
        route = "group-create"
    else:
        return data, None, 0

    value = json.loads(data)
    changed = 0

    def stringify_id(item):
        nonlocal changed
        if (
            isinstance(item, dict)
            and isinstance(item.get("id"), int)
            and not isinstance(item.get("id"), bool)
        ):
            item["id"] = str(item["id"])
            changed += 1

    if route in ("whoami", "user-create", "group-create"):
        stringify_id(value)
    elif route == "user-read":
        users = value.get("users") if isinstance(value, dict) else None
        if isinstance(users, list):
            for user in users:
                stringify_id(user)
                if not isinstance(user, dict):
                    continue
                can_login = user.get("can_login")
                if isinstance(can_login, bool):
                    user["can_login"] = int(can_login)
                    changed += 1
                groups = user.get("groups")
                if isinstance(groups, list):
                    for group in groups:
                        stringify_id(group)
    elif route == "group-read":
        groups = value.get("groups") if isinstance(value, dict) else None
        if isinstance(groups, list):
            for group in groups:
                stringify_id(group)
                if not isinstance(group, dict):
                    continue
                is_active = group.get("is_active")
                if isinstance(is_active, bool):
                    group["is_active"] = int(is_active)
                    changed += 1
                membership = group.get("membership")
                if isinstance(membership, list):
                    for member in membership:
                        stringify_id(member)

    if changed == 0:
        return data, route, 0
    return json.dumps(value, separators=(",", ":")).encode(), route, changed


def shape_server_capabilities_response(path, data, *, enabled):
    """Return opt-in production shapes used by the capability functional proof."""
    if not enabled:
        return data, {}

    route = urllib.parse.urlsplit(path).path
    if route not in {
        "/rest/version",
        "/rest/parameters",
        "/rest/field/bug/bug%5Fstatus",
        "/rest/field/bug",
    }:
        return data, {}

    value = json.loads(data)
    if not isinstance(value, dict):
        return data, {}

    if route == "/rest/version":
        value["version"] = "5.2+"
        evidence = {"version": 1}
    elif route == "/rest/parameters":
        parameters = value.get("parameters")
        if not isinstance(parameters, dict) or "maxattachmentsize" not in parameters:
            return data, {}
        parameters["maxattachmentsize"] = str(parameters["maxattachmentsize"])
        evidence = {"parameters": 1}
    elif route == "/rest/field/bug/bug%5Fstatus":
        fields = value.get("fields")
        if not isinstance(fields, list):
            return data, {}
        status_field = next(
            (field for field in fields if isinstance(field, dict)), None
        )
        values = status_field.get("values") if status_field is not None else None
        if not isinstance(values, list):
            return data, {}
        values.insert(0, {"name": "", "can_change_to": []})
        evidence = {"status": 1}
    else:
        fields = value.get("fields")
        if not isinstance(fields, list):
            return data, {}
        fields.append({
            "name": "cf_bzr_proxy_probe",
            "type": "2",
            "is_custom": True,
            "values": [],
        })
        evidence = {"field-type": 1}

    return json.dumps(value, separators=(",", ":")).encode(), evidence


def shape_attachment_comment_response(method, path, data):
    """Return opt-in attachment/comment shapes and credential-free evidence."""
    parsed = urllib.parse.urlsplit(path)
    route = parsed.path
    is_upload = method == "POST" and re.fullmatch(
        r"/rest/bug/[0-9]+/attachment", route
    )
    is_by_id = method == "GET" and re.fullmatch(
        r"/rest/bug/attachment/[0-9]+", route
    )
    is_list = method == "GET" and re.fullmatch(
        r"/rest/bug/[0-9]+/attachment", route
    )
    is_comments = method == "GET" and re.fullmatch(
        r"/rest/bug/[0-9]+/comment", route
    )
    if not any((is_upload, is_by_id, is_list, is_comments)):
        return data, {}

    value = json.loads(data)
    evidence = {}

    if is_upload:
        ids = value.get("ids") if isinstance(value, dict) else None
        if isinstance(ids, list):
            for index, attachment_id in enumerate(ids):
                if isinstance(attachment_id, int) and not isinstance(
                    attachment_id, bool
                ):
                    ids[index] = str(attachment_id)
            observed = sum(isinstance(attachment_id, str) for attachment_id in ids)
            if observed:
                evidence["attachment-upload"] = observed

    elif is_by_id:
        attachments = value.get("attachments") if isinstance(value, dict) else None
        if isinstance(attachments, dict):
            value["attachments"] = list(attachments.values())
            attachments = value["attachments"]
        if isinstance(attachments, list):
            exclusion = urllib.parse.parse_qs(parsed.query).get("exclude_fields")
            evidence[
                "attachment-by-id-metadata"
                if exclusion == ["data"]
                else "attachment-by-id-body"
            ] = len(attachments)

    elif is_list:
        exclusion = urllib.parse.parse_qs(parsed.query).get("exclude_fields")
        if exclusion == ["data"]:
            attachment_lists = []
            bugs = value.get("bugs") if isinstance(value, dict) else None
            if isinstance(bugs, dict):
                attachment_lists.extend(
                    attachments for attachments in bugs.values()
                    if isinstance(attachments, list)
                )
            attachments = value.get("attachments") if isinstance(value, dict) else None
            if isinstance(attachments, list):
                attachment_lists.append(attachments)
            for attachments in attachment_lists:
                for attachment in attachments:
                    if isinstance(attachment, dict):
                        attachment.pop("data", None)
            evidence["attachment-list-excludes-body"] = 1

    else:
        comments = []
        if isinstance(value, dict) and isinstance(value.get("comments"), list):
            comments.extend(value["comments"])
        bugs = value.get("bugs") if isinstance(value, dict) else None
        if isinstance(bugs, dict):
            for bug in bugs.values():
                if isinstance(bug, dict) and isinstance(bug.get("comments"), list):
                    comments.extend(bug["comments"])
        observed = 0
        for comment in comments:
            if not isinstance(comment, dict):
                continue
            privacy = comment.get("is_private")
            if isinstance(privacy, bool):
                privacy = int(privacy)
                comment["is_private"] = privacy
            if type(privacy) is int:
                observed += 1
        if observed:
            evidence["comment-privacy"] = observed

    if not evidence:
        return data, {}
    return json.dumps(value, separators=(",", ":")).encode(), evidence


def _hook_matches(prefixes=(), *, method=None, mode=None):
    def matches(request_method, path, enabled_modes):
        route = urllib.parse.urlsplit(path).path
        return (
            (method is None or request_method == method)
            and (not prefixes or route.startswith(prefixes))
            and (mode is None or mode in enabled_modes)
        )

    return matches


def _run_bug_hook(_method, _path, body):
    return shape_bug_response(body), []


def _run_product_ids_hook(_method, _path, body):
    return shape_product_ids_response(body), []


def _run_server_hook(_method, path, body):
    shaped, evidence = shape_server_capabilities_response(
        path, body, enabled=True
    )
    return shaped, [
        ("server-capability", route, count) for route, count in evidence.items()
    ]


def _run_attachment_hook(method, path, body):
    shaped, evidence = shape_attachment_comment_response(method, path, body)
    return shaped, [
        ("attachment-comment", route, count) for route, count in evidence.items()
    ]


def _attachment_hook_matches(method, path, enabled_modes):
    if "attachment-comment" not in enabled_modes:
        return False
    route = urllib.parse.urlsplit(path).path
    return (
        (method == "POST" and re.fullmatch(
            r"/rest/bug/[0-9]+/attachment", route
        ))
        or (method == "GET" and re.fullmatch(
            r"/rest/bug/attachment/[0-9]+", route
        ))
        or (method == "GET" and re.fullmatch(
            r"/rest/bug/[0-9]+/attachment", route
        ))
        or (method == "GET" and re.fullmatch(
            r"/rest/bug/[0-9]+/comment", route
        ))
    )


def _run_metadata_hook(_method, path, body):
    shaped, changed = shape_metadata_sort_keys_response(path, body)
    if not changed:
        return shaped, []
    route = (
        "field"
        if urllib.parse.urlsplit(path).path.startswith("/rest/field/bug")
        else "product"
    )
    return shaped, [("metadata-sort-keys", route, changed)]


def _run_user_group_hook(method, path, body):
    shaped, route, changed = shape_user_group_response(method, path, body)
    return shaped, [("user-group-shaped", route, changed)] if changed else []


# Each entry has one matcher and one transformer. Transformers return
# (body, evidence), so adding a production shape never edits the forwarding loop.
REWRITE_HOOKS = (
    (_hook_matches(("/rest/bug",)), _run_bug_hook),
    (
        _hook_matches((
            "/rest/product_accessible",
            "/rest/product_selectable",
            "/rest/product_enterable",
        )),
        _run_product_ids_hook,
    ),
    (_hook_matches(mode="server-capabilities"), _run_server_hook),
    (_attachment_hook_matches, _run_attachment_hook),
    (_hook_matches(), _run_metadata_hook),
    (_hook_matches(), _run_user_group_hook),
)


def apply_rewrite_hooks(method, path, body, enabled_modes):
    """Apply every matching hook in registry order and return body plus evidence."""
    evidence = []
    for matches, transform in REWRITE_HOOKS:
        if matches(method, path, enabled_modes):
            body, hook_evidence = transform(method, path, body)
            evidence.extend(hook_evidence)
    return body, evidence


def emit_rewrite_evidence(evidence):
    for marker, route, count in evidence:
        if marker == "metadata-sort-keys":
            sys.stderr.write(f"metadata-sort-keys shaped route={route} count={count}\n")
        elif marker == "user-group-shaped":
            sys.stderr.write(f"user-group-shaped route={route} count={count}\n")
        else:
            sys.stderr.write(f"{marker} shaped route={route} count={count}\n")
    if evidence:
        sys.stderr.flush()


def make_handler(backend_port):
    server_capability_mode = os.environ.get("BZR_FUNC_REDHAT_MODE") == (
        "server-capabilities"
    )
    attachment_comment_mode = os.environ.get("BZR_FUNC_REDHAT_MODE") == (
        "attachment-comment"
    )

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

        def do_PUT(self):
            if attachment_comment_mode:
                self._forward("PUT")
            else:
                self.send_error(501, "Unsupported method ('PUT')")

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

            if 200 <= status < 300:
                try:
                    body, evidence = apply_rewrite_hooks(
                        method,
                        self.path,
                        body,
                        {mode for mode, enabled in (
                            ("server-capabilities", server_capability_mode),
                            ("attachment-comment", attachment_comment_mode),
                        ) if enabled},
                    )
                except (UnicodeDecodeError, json.JSONDecodeError) as error:
                    self.send_error(502, f"Bugzilla returned malformed JSON: {error}")
                    return
                emit_rewrite_evidence(evidence)

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
    def test_rewrite_hook_registry_has_uniform_entries(self):
        self.assertGreaterEqual(len(REWRITE_HOOKS), 6)
        for matches, transform in REWRITE_HOOKS:
            self.assertTrue(callable(matches))
            self.assertTrue(callable(transform))

    def test_dispatcher_applies_matching_hooks_in_registry_order(self):
        payload = json.dumps({
            "bugs": [{"component": "Backend", "version": "rawhide"}]
        }).encode()
        body, evidence = apply_rewrite_hooks("GET", "/rest/bug", payload, set())
        self.assertEqual(json.loads(body)["bugs"][0]["component"], [
            "Backend", "Backend-redhat-secondary"
        ])
        self.assertEqual(evidence, [])

    def test_dispatcher_keeps_mode_hooks_disabled_by_default(self):
        payload = b'{"version":"5.0.6"}'
        body, evidence = apply_rewrite_hooks(
            "GET", "/rest/version", payload, set()
        )
        self.assertEqual(body, payload)
        self.assertEqual(evidence, [])

    def test_shapes_attachment_upload_ids_as_strings(self):
        body, evidence = shape_attachment_comment_response(
            "POST", "/rest/bug/42/attachment", b'{"ids":[123]}'
        )
        self.assertEqual(json.loads(body), {"ids": ["123"]})
        self.assertEqual(evidence, {"attachment-upload": 1})

    def test_observes_native_attachment_upload_string_ids(self):
        body, evidence = shape_attachment_comment_response(
            "POST", "/rest/bug/42/attachment", b'{"ids":["123"]}'
        )
        self.assertEqual(json.loads(body), {"ids": ["123"]})
        self.assertEqual(evidence, {"attachment-upload": 1})

    def test_shapes_attachment_by_id_envelopes_as_flat_arrays(self):
        payload = b'{"attachments":{"123":{"id":123,"data":"Ynl0ZXM="}}}'
        for path, expected_evidence in [
            ("/rest/bug/attachment/123", {"attachment-by-id-body": 1}),
            (
                "/rest/bug/attachment/123?exclude_fields=data",
                {"attachment-by-id-metadata": 1},
            ),
        ]:
            body, evidence = shape_attachment_comment_response("GET", path, payload)
            self.assertEqual(
                json.loads(body),
                {"attachments": [{"id": 123, "data": "Ynl0ZXM="}]},
            )
            self.assertEqual(evidence, expected_evidence)

    def test_observes_native_attachment_by_id_flat_arrays(self):
        payload = b'{"attachments":[{"id":123,"data":"Ynl0ZXM="}]}'
        body, evidence = shape_attachment_comment_response(
            "GET", "/rest/bug/attachment/123", payload
        )
        self.assertEqual(json.loads(body), json.loads(payload))
        self.assertEqual(evidence, {"attachment-by-id-body": 1})

    def test_shapes_comment_privacy_as_binary_integers(self):
        payload = json.dumps({"bugs": {"42": {"comments": [
            {"id": 1, "is_private": False},
            {"id": 2, "is_private": True},
            {"id": 3, "is_private": None},
        ]}}}).encode()
        body, evidence = shape_attachment_comment_response(
            "GET", "/rest/bug/42/comment", payload
        )
        comments = json.loads(body)["bugs"]["42"]["comments"]
        self.assertEqual(
            [comment["is_private"] for comment in comments], [0, 1, None]
        )
        self.assertEqual(evidence, {"comment-privacy": 2})

    def test_observes_native_comment_privacy_integers(self):
        payload = b'{"comments":[{"id":1,"is_private":1}]}'
        body, evidence = shape_attachment_comment_response(
            "GET", "/rest/bug/42/comment", payload
        )
        self.assertEqual(json.loads(body), json.loads(payload))
        self.assertEqual(evidence, {"comment-privacy": 1})

    def test_observes_exact_attachment_list_body_exclusion_without_query_values(self):
        payload = b'{"bugs":{"42":[{"id":123,"data":"Ynl0ZXM="}]}}'
        body, evidence = shape_attachment_comment_response(
            "GET",
            "/rest/bug/42/attachment?exclude_fields=data&Bugzilla_api_key=secret",
            payload,
        )
        self.assertEqual(json.loads(body), {"bugs": {"42": [{"id": 123}]}})
        self.assertEqual(evidence, {"attachment-list-excludes-body": 1})

        _, evidence = shape_attachment_comment_response(
            "GET", "/rest/bug/42/attachment?exclude_fields=id,data", payload
        )
        self.assertEqual(evidence, {})

    def test_put_is_rejected_outside_attachment_comment_mode(self):
        previous_mode = os.environ.pop("BZR_FUNC_REDHAT_MODE", None)
        blocker = socket.socket()
        blocker.bind(("127.0.0.1", 0))
        unavailable_port = blocker.getsockname()[1]
        blocker.close()
        server, thread = self._start_server(unavailable_port)
        try:
            request = urllib.request.Request(
                f"http://127.0.0.1:{server.server_port}/rest/bug/attachment/123",
                data=b"{}",
                method="PUT",
            )
            with self.assertRaises(urllib.error.HTTPError) as error:
                urllib.request.urlopen(request, timeout=2)
            self.assertEqual(error.exception.code, 501)
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=2)
            if previous_mode is not None:
                os.environ["BZR_FUNC_REDHAT_MODE"] = previous_mode

    def test_put_is_forwarded_in_attachment_comment_mode(self):
        previous_mode = os.environ.get("BZR_FUNC_REDHAT_MODE")
        os.environ["BZR_FUNC_REDHAT_MODE"] = "attachment-comment"
        blocker = socket.socket()
        blocker.bind(("127.0.0.1", 0))
        unavailable_port = blocker.getsockname()[1]
        blocker.close()
        server, thread = self._start_server(unavailable_port)
        try:
            request = urllib.request.Request(
                f"http://127.0.0.1:{server.server_port}/rest/bug/attachment/123",
                data=b"{}",
                method="PUT",
            )
            with self.assertRaises(urllib.error.HTTPError) as error:
                urllib.request.urlopen(request, timeout=2)
            self.assertEqual(error.exception.code, 502)
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=2)
            if previous_mode is None:
                os.environ.pop("BZR_FUNC_REDHAT_MODE", None)
            else:
                os.environ["BZR_FUNC_REDHAT_MODE"] = previous_mode

    def test_attachment_comment_shape_leaves_unrelated_routes_untouched(self):
        payload = b"not json"
        for method, path in [
            ("GET", "/rest/bug/42"),
            ("POST", "/rest/bug/42/comment"),
            ("GET", "/rest/product"),
        ]:
            body, evidence = shape_attachment_comment_response(method, path, payload)
            self.assertEqual(body, payload)
            self.assertEqual(evidence, {})

    def test_attachment_comment_shape_rejects_malformed_matching_json(self):
        for method, path in [
            ("POST", "/rest/bug/42/attachment"),
            ("GET", "/rest/bug/attachment/123"),
            ("GET", "/rest/bug/42/comment"),
        ]:
            with self.assertRaises(json.JSONDecodeError):
                shape_attachment_comment_response(method, path, b"not json")

    def test_server_capability_shapes_are_opt_in(self):
        payload = b'{"version":"5.0.6"}'
        body, evidence = shape_server_capabilities_response(
            "/rest/version", payload, enabled=False
        )
        self.assertEqual(body, payload)
        self.assertEqual(evidence, {})

    def test_shapes_server_capability_routes(self):
        cases = [
            (
                "/rest/version",
                {"version": "5.0.6"},
                {"version": "5.2+"},
                {"version": 1},
            ),
            (
                "/rest/parameters",
                {"parameters": {"maxattachmentsize": 1000}},
                {"parameters": {"maxattachmentsize": "1000"}},
                {"parameters": 1},
            ),
            (
                "/rest/field/bug/bug%5Fstatus",
                {"fields": [{"name": "bug_status", "values": [{"name": "NEW"}]}]},
                {"fields": [{"name": "bug_status", "values": [
                    {"name": "", "can_change_to": []}, {"name": "NEW"}
                ]}]},
                {"status": 1},
            ),
        ]
        for path, payload, expected, evidence in cases:
            body, actual_evidence = shape_server_capabilities_response(
                path, json.dumps(payload).encode(), enabled=True
            )
            self.assertEqual(json.loads(body), expected)
            self.assertEqual(actual_evidence, evidence)

        body, evidence = shape_server_capabilities_response(
            "/rest/field/bug", b'{"fields":[]}', enabled=True
        )
        self.assertEqual(evidence, {"field-type": 1})
        self.assertEqual(json.loads(body)["fields"], [{
            "name": "cf_bzr_proxy_probe",
            "type": "2",
            "is_custom": True,
            "values": [],
        }])

    def test_server_capability_shape_leaves_unrelated_payload_untouched(self):
        payload = b'{"products":[]}'
        body, evidence = shape_server_capabilities_response(
            "/rest/product", payload, enabled=True
        )
        self.assertEqual(body, payload)
        self.assertEqual(evidence, {})

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

    def test_shapes_whoami_and_create_ids(self):
        for method, path, route in [
            ("GET", "/rest/whoami?Bugzilla_api_key=secret", "whoami"),
            ("POST", "/rest/user", "user-create"),
            ("POST", "/rest/group", "group-create"),
        ]:
            body, actual_route, count = shape_user_group_response(
                method, path, b'{"id":42}'
            )
            self.assertEqual(actual_route, route)
            self.assertEqual(count, 1)
            self.assertEqual(json.loads(body)["id"], "42")

    def test_shapes_user_read_fields_and_preserves_absent_optional_fields(self):
        payload = json.dumps({"users": [
            {"id": 7, "can_login": True, "groups": [{"id": 3}]},
            {"id": 8, "can_login": None, "groups": []},
            {"id": 9},
        ]}).encode()
        body, route, count = shape_user_group_response(
            "GET", "/rest/user?groups=admin", payload
        )
        shaped = json.loads(body)
        self.assertEqual(route, "user-read")
        self.assertEqual(count, 5)
        self.assertEqual(shaped["users"][0], {
            "id": "7", "can_login": 1, "groups": [{"id": "3"}]
        })
        self.assertIsNone(shaped["users"][1]["can_login"])
        self.assertNotIn("can_login", shaped["users"][2])

    def test_shapes_group_read_fields(self):
        payload = json.dumps({"groups": [{
            "id": 4,
            "is_active": False,
            "membership": [{"id": 12}],
        }]}).encode()
        body, route, count = shape_user_group_response(
            "GET", "/rest/group?names=admin", payload
        )
        self.assertEqual(route, "group-read")
        self.assertEqual(count, 3)
        self.assertEqual(json.loads(body)["groups"][0], {
            "id": "4", "is_active": 0, "membership": [{"id": "12"}]
        })

    def test_user_group_shape_leaves_unrelated_routes_untouched(self):
        payload = b'{"id":42}'
        for method, path in [
            ("GET", "/rest/bug/42"),
            ("PUT", "/rest/user/alice"),
            ("POST", "/rest/group/4"),
        ]:
            body, route, count = shape_user_group_response(method, path, payload)
            self.assertEqual(body, payload)
            self.assertIsNone(route)
            self.assertEqual(count, 0)

    def test_user_group_shape_does_not_coerce_boolean_id(self):
        payload = b'{"id":true}'
        body, route, count = shape_user_group_response(
            "GET", "/rest/whoami", payload
        )
        self.assertEqual(route, "whoami")
        self.assertEqual(count, 0)
        self.assertIs(json.loads(body)["id"], True)

    def test_user_group_shape_rejects_malformed_json_on_matching_route(self):
        with self.assertRaises(json.JSONDecodeError):
            shape_user_group_response("GET", "/rest/user", b"not json")

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
