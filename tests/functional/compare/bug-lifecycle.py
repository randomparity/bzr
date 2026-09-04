#!/usr/bin/env python3
"""Fixed JSON adapter for python-bugzilla lifecycle comparison probes."""

import datetime
import json
import os
import stat
import sys
from xmlrpc.client import DateTime
from pathlib import Path

from bugzilla import Bugzilla


SERVER_URL = "http://127.0.0.1"
WORK_ROOT = Path("/work")


class AdapterError(Exception):
    """A safe, request-shape error that can be shown without secret content."""


def _validate_keys(request, required, optional=()):
    required = set(required)
    allowed = required | set(optional)
    missing = sorted(required - request.keys())
    unexpected = sorted(request.keys() - allowed)
    if missing:
        raise AdapterError(f"missing request fields: {', '.join(missing)}")
    if unexpected:
        raise AdapterError(f"unexpected request fields: {', '.join(unexpected)}")


def _required_text(request, name, maximum=None):
    value = request[name]
    if not isinstance(value, str) or not value:
        raise AdapterError(f"{name} must be a non-empty string")
    if maximum is not None and len(value) > maximum:
        raise AdapterError(f"{name} must be at most {maximum} characters")
    return value


def _required_id(request):
    value = request["bug_id"]
    if type(value) is not int or value <= 0:
        raise AdapterError("bug_id must be a positive integer")
    return value


def _required_mapping(request, name, *, nonempty=False):
    value = request[name]
    if not isinstance(value, dict):
        raise AdapterError(f"{name} must be a JSON object")
    if nonempty and not value:
        raise AdapterError(f"{name} must not be empty")
    if any(not isinstance(key, str) or not key for key in value):
        raise AdapterError(f"{name} keys must be non-empty strings")
    return value.copy()


def _required_string_list(request, name):
    value = request[name]
    if (
        not isinstance(value, list)
        or not value
        or any(not isinstance(item, str) or not item for item in value)
    ):
        raise AdapterError(f"{name} must be a non-empty string array")
    return value


def _bug_data(bug):
    getter = getattr(bug, "get_raw_data", None)
    if not callable(getter):
        raise AdapterError("python-bugzilla returned an invalid bug result")
    data = getter()
    if not isinstance(data, dict):
        raise AdapterError("python-bugzilla returned non-object bug data")
    return data


def _bug_list(bugs):
    if not isinstance(bugs, list):
        raise AdapterError("python-bugzilla returned a non-list query result")
    return [_bug_data(bug) for bug in bugs]


def _create(client, request):
    _validate_keys(request, ("api_key", "params"))
    params = client.build_createbug(**_required_mapping(request, "params", nonempty=True))
    return _bug_data(client.createbug(params))


def _query(client, request):
    _validate_keys(request, ("api_key", "params"))
    query = client.build_query(**_required_mapping(request, "params", nonempty=True))
    return _bug_list(client.query(query))


def _update(client, request):
    _validate_keys(request, ("api_key", "bug_id", "params"))
    bug_id = _required_id(request)
    update = client.build_update(**_required_mapping(request, "params", nonempty=True))
    return client.update_bugs([bug_id], update)


def _view(client, request):
    _validate_keys(request, ("api_key", "bug_id"))
    return _bug_data(client.getbug(_required_id(request)))


def _history(client, request):
    _validate_keys(request, ("api_key", "bug_id"))
    return client.bugs_history_raw([_required_id(request)])


def _saved_search(client, request):
    _validate_keys(request, ("api_key", "name"))
    query = client.build_query(savedsearch=_required_text(request, "name", 64))
    return _bug_list(client.query(query))


def _merge_generic_fields(params, fields):
    overlap = sorted(params.keys() & fields.keys())
    if overlap:
        raise AdapterError(f"generic fields overlap parameters: {', '.join(overlap)}")
    params.update(fields)
    return params


def _generic_fields(client, request):
    _validate_keys(
        request,
        ("api_key", "action", "params", "fields"),
        ("bug_id",),
    )
    action = _required_text(request, "action")
    params = _required_mapping(request, "params")
    fields = _required_mapping(request, "fields", nonempty=True)
    if action == "create":
        if "bug_id" in request:
            raise AdapterError("generic create does not accept bug_id")
        built = client.build_createbug(**params)
        return _bug_data(client.createbug(_merge_generic_fields(built, fields)))
    if action == "update":
        if "bug_id" not in request:
            raise AdapterError("generic update requires bug_id")
        built = client.build_update(**params)
        return client.update_bugs(
            [_required_id(request)],
            _merge_generic_fields(built, fields),
        )
    raise AdapterError("action must be create or update")


def _update_options(client, request):
    _validate_keys(
        request,
        ("api_key", "bug_id", "comment", "comment_tags", "minor_update"),
    )
    bug_id = _required_id(request)
    comment = _required_text(request, "comment")
    comment_tags = _required_string_list(request, "comment_tags")
    if request["minor_update"] is not True:
        raise AdapterError("minor_update must be true")
    update = client.build_update(
        comment=comment,
        comment_tags=comment_tags,
        minor_update=True,
    )
    return client.update_bugs([bug_id], update)


def _match_type(client, request):
    _validate_keys(request, ("api_key", "value", "match_type"))
    match_type = _required_text(request, "match_type")
    if match_type != "equals":
        raise AdapterError("match_type must be equals")
    query = client.build_query(
        status_whiteboard=_required_text(request, "value"),
        status_whiteboard_type=match_type,
    )
    return _bug_list(client.query(query))


def _bug_tags(client, request):
    _validate_keys(request, ("api_key", "bug_id", "tag"))
    bug_id = _required_id(request)
    tag = _required_text(request, "tag")
    update = client.update_tags([bug_id], tags_add=[tag])
    query = client.build_query(tags=[tag])
    return {"update": update, "bugs": _bug_list(client.query(query))}


OPERATIONS = {
    "create": _create,
    "query": _query,
    "update": _update,
    "view": _view,
    "history": _history,
    "saved_search": _saved_search,
    "generic_fields": _generic_fields,
    "update_options": _update_options,
    "match_type": _match_type,
    "bug_tags": _bug_tags,
}


def _work_path(value, label):
    path = Path(value)
    if not path.is_absolute():
        raise AdapterError(f"{label} path must be absolute")
    resolved = path.resolve(strict=False)
    if resolved != WORK_ROOT and WORK_ROOT not in resolved.parents:
        raise AdapterError(f"{label} path must be under /work")
    return resolved


def _load_request(path):
    try:
        mode = stat.S_IMODE(path.stat().st_mode)
        if mode & 0o077:
            raise AdapterError("input file must not be accessible by group or others")
        with path.open(encoding="utf-8") as source:
            request = json.load(source)
    except AdapterError:
        raise
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise AdapterError("input file is unreadable or invalid JSON") from error
    if not isinstance(request, dict):
        raise AdapterError("input must be one JSON object")
    api_key = request.get("api_key")
    if not isinstance(api_key, str) or not api_key:
        raise AdapterError("api_key must be a non-empty string")
    return request


def _transport(client):
    backend = getattr(client, "_backend", None)
    name = type(backend).__name__ if backend is not None else ""
    if not name:
        raise AdapterError("python-bugzilla did not select a backend")
    return name


def _json_default(value):
    if isinstance(value, (datetime.date, datetime.datetime)):
        return value.isoformat()
    if isinstance(value, DateTime):
        return value.value
    raise TypeError(f"{type(value).__name__} is not JSON serializable")


def _write_output(path, payload):
    encoded = json.dumps(payload, default=_json_default, sort_keys=True) + "\n"
    flags = os.O_WRONLY | os.O_CREAT | os.O_TRUNC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(path, flags, 0o600)
    try:
        os.fchmod(descriptor, 0o600)
        with os.fdopen(descriptor, "w", encoding="utf-8") as destination:
            descriptor = -1
            destination.write(encoded)
    finally:
        if descriptor >= 0:
            os.close(descriptor)


def main(argv):
    input_label = "bug-lifecycle.py"
    try:
        if len(argv) != 4:
            raise AdapterError("usage: bug-lifecycle.py OP INPUT OUTPUT")
        operation = argv[1]
        input_label = argv[2]
        handler = OPERATIONS.get(operation)
        if handler is None:
            raise AdapterError("unsupported operation")
        input_path = _work_path(argv[2], "input")
        output_path = _work_path(argv[3], "output")
        if input_path == output_path:
            raise AdapterError("input and output paths must differ")
        request = _load_request(input_path)
        client = Bugzilla(
            SERVER_URL,
            api_key=request["api_key"],
            use_creds=False,
        )
        result = handler(client, request)
        _write_output(
            output_path,
            {"transport": _transport(client), "result": result},
        )
    except AdapterError as error:
        print(f"{input_label}: {error}", file=sys.stderr)
        return 1
    except Exception as error:  # noqa: BLE001 - do not expose upstream secret-bearing text
        print(
            f"{input_label}: operation failed ({type(error).__name__})",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
