#!/usr/bin/env python3
"""Fixed JSON adapter for python-bugzilla comparison probes."""

import datetime
import json
import os
import stat
import sys
from types import SimpleNamespace
import urllib.parse
from xmlrpc.client import DateTime
from pathlib import Path

from bugzilla import Bugzilla, BugzillaError


SERVER_URL = "http://127.0.0.1"
WORK_ROOT = Path("/work/compare")
TOKEN_FILE = WORK_ROOT / "python-bugzilla-token"
STALE_TOKEN_FILE = WORK_ROOT / "python-bugzilla-stale-token"


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


def _required_positive_id(request, name):
    value = request[name]
    if type(value) is not int or value <= 0:
        raise AdapterError(f"{name} must be a positive integer")
    return value


def _required_id_list(request, name):
    value = request[name]
    if (
        not isinstance(value, list)
        or not value
        or any(type(item) is not int or item <= 0 for item in value)
    ):
        raise AdapterError(f"{name} must be a non-empty positive integer array")
    return value.copy()


def _required_bool(request, name):
    value = request[name]
    if type(value) is not bool:
        raise AdapterError(f"{name} must be a boolean")
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


def _user_data(user):
    if user is None:
        raise AdapterError("python-bugzilla returned no user")
    user_id = getattr(user, "userid", None)
    email = getattr(user, "email", None)
    name = getattr(user, "name", None)
    if type(user_id) is not int or user_id <= 0 or not isinstance(email, str):
        raise AdapterError("python-bugzilla returned an invalid user")
    return {
        "id": user_id,
        "email": email,
        "name": name,
        "real_name": getattr(user, "real_name", None),
        "can_login": getattr(user, "can_login", False),
        "groups": sorted(getattr(user, "groupnames", [])),
    }


def _group_data(group):
    if group is None:
        raise AdapterError("python-bugzilla returned no group")
    group_id = getattr(group, "groupid", None)
    name = getattr(group, "name", None)
    members = getattr(group, "member_emails", [])
    if type(group_id) is not int or group_id <= 0 or not isinstance(name, str):
        raise AdapterError("python-bugzilla returned an invalid group")
    return {
        "id": group_id,
        "name": name,
        "description": getattr(group, "description", None),
        "is_active": getattr(group, "is_active", False),
        "members": sorted(members),
    }


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
        minor_update=True,
    )
    result = client.update_bugs([bug_id], update)
    comments = client.get_comments([bug_id])
    bug_comments = comments.get("bugs", {}).get(str(bug_id), {}).get("comments", [])
    comment_id = next(
        (entry.get("id") for entry in reversed(bug_comments) if entry.get("text") == comment),
        None,
    )
    if not isinstance(comment_id, int) or comment_id <= 0:
        raise AdapterError("updated comment was not found")
    try:
        client._backend._put(f"/bug/comment/{comment_id}/tags", {"add": comment_tags})
    except ValueError:
        # python-bugzilla 3.3.0 assumes REST mutations return an object, but
        # this endpoint returns its tag array after committing the change.
        pass
    tagged_comments = client.get_comments([bug_id])
    tagged = tagged_comments.get("bugs", {}).get(str(bug_id), {}).get("comments", [])
    if not any(
        entry.get("id") == comment_id and all(tag in entry.get("tags", []) for tag in comment_tags)
        for entry in tagged
    ):
        raise AdapterError("updated comment tags were not read back")
    return result


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


def _comment_add(client, request):
    _validate_keys(request, ("api_key", "bug_id", "text", "is_private"))
    update = client.build_update(
        comment=_required_text(request, "text"),
        comment_private=_required_bool(request, "is_private"),
    )
    return client.update_bugs([_required_id(request)], update)


def _comment_list(client, request):
    _validate_keys(request, ("api_key", "bug_id"))
    result = client.get_comments([_required_id(request)])
    if not isinstance(result, dict):
        raise AdapterError("python-bugzilla returned invalid comments")
    return result


def _attachment_path(request, name):
    path = _work_path(_required_text(request, name), name)
    try:
        metadata = path.lstat()
    except OSError as error:
        raise AdapterError(f"{name} file is unreadable") from error
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
        raise AdapterError(f"{name} must be a regular non-symlink file")
    if stat.S_IMODE(metadata.st_mode) != 0o600:
        raise AdapterError(f"{name} file mode must be 0600")
    return path


def _attachment_upload(client, request):
    _validate_keys(
        request,
        (
            "api_key",
            "bug_ids",
            "source",
            "summary",
            "file_name",
            "content_type",
            "comment",
            "is_private",
        ),
    )
    result = client.attachfile(
        _required_id_list(request, "bug_ids"),
        str(_attachment_path(request, "source")),
        _required_text(request, "summary"),
        file_name=_required_text(request, "file_name"),
        content_type=_required_text(request, "content_type"),
        comment=_required_text(request, "comment"),
        is_private=_required_bool(request, "is_private"),
    )
    attachment_ids = result if isinstance(result, list) else [result]
    if not attachment_ids or any(type(item) is not int or item <= 0 for item in attachment_ids):
        raise AdapterError("python-bugzilla returned invalid attachment IDs")
    return {"attachment_ids": attachment_ids}


def _attachment_list(client, request):
    _validate_keys(request, ("api_key", "bug_ids"))
    result = client.get_attachments(
        _required_id_list(request, "bug_ids"), None, exclude_fields=["data"]
    )
    if not isinstance(result, dict):
        raise AdapterError("python-bugzilla returned invalid attachments")
    return result


def _attachment_get(client, request):
    _validate_keys(request, ("api_key", "attachment_ids"))
    result = client.get_attachments(
        None, _required_id_list(request, "attachment_ids"), exclude_fields=["data"]
    )
    if not isinstance(result, dict):
        raise AdapterError("python-bugzilla returned invalid attachments")
    return result


def _write_private_bytes(path, data):
    flags = os.O_WRONLY | os.O_CREAT | os.O_TRUNC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(path, flags, 0o600)
    try:
        os.fchmod(descriptor, 0o600)
        with os.fdopen(descriptor, "wb") as destination:
            descriptor = -1
            destination.write(data)
    finally:
        if descriptor >= 0:
            os.close(descriptor)


def _attachment_download(client, request):
    _validate_keys(request, ("api_key", "attachment_id", "destination"))
    attachment_id = _required_positive_id(request, "attachment_id")
    destination = _work_path(_required_text(request, "destination"), "destination")
    stream = client.openattachment(attachment_id)
    data = stream.read()
    if not isinstance(data, bytes):
        raise AdapterError("python-bugzilla returned non-byte attachment data")
    _write_private_bytes(destination, data)
    return {"attachment_id": attachment_id, "bytes": len(data)}


def _attachment_cli_download_bug(client, request):
    import bugzilla._cli as bugzilla_cli

    _validate_keys(request, ("api_key", "bug_id", "destination", "ignore_obsolete"))
    bug_id = _required_id(request)
    destination = _work_path(_required_text(request, "destination"), "destination")
    ignore_obsolete = _required_bool(request, "ignore_obsolete")
    original_directory = Path.cwd()
    original_open = bugzilla_cli.open_without_clobber
    original_umask = os.umask(0o077)

    def safe_open(name, *args):
        if not isinstance(name, str) or not name or Path(name).name != name:
            raise AdapterError("python-bugzilla returned an unsafe attachment name")
        return original_open(name, *args)

    try:
        try:
            destination.mkdir(mode=0o700)
        except FileExistsError:
            if destination.is_symlink() or not destination.is_dir():
                raise AdapterError("destination must be a non-symlink directory") from None
        destination.chmod(0o700)
        before = {path.name for path in destination.iterdir()}
        os.chdir(destination)
        bugzilla_cli.open_without_clobber = safe_open
        bugzilla_cli._do_get_attach(
            client,
            SimpleNamespace(
                getall=[bug_id],
                get=None,
                ignore_obsolete=ignore_obsolete,
            ),
        )
    finally:
        bugzilla_cli.open_without_clobber = original_open
        os.chdir(original_directory)
        os.umask(original_umask)
    files = sorted(path.name for path in destination.iterdir() if path.name not in before)
    return {"bug_id": bug_id, "files": files}


def _attachment_flag(client, request):
    _validate_keys(
        request,
        ("api_key", "bug_id", "attachment_id", "flag_name", "status"),
        ("requestee",),
    )
    kwargs = {"status": _required_text(request, "status", 1)}
    if "requestee" in request:
        kwargs["requestee"] = _required_text(request, "requestee")
    return client.updateattachmentflags(
        _required_id(request),
        _required_positive_id(request, "attachment_id"),
        _required_text(request, "flag_name"),
        **kwargs,
    )


def _user_create(client, request):
    _validate_keys(request, ("api_key", "email", "password"), ("name",))
    user = client.createuser(
        _required_text(request, "email"),
        _required_text(request, "name") if "name" in request else "",
        _required_text(request, "password"),
    )
    return _user_data(user)


def _user_get(client, request):
    _validate_keys(request, ("api_key", "email"))
    return _user_data(client.getuser(_required_text(request, "email")))


def _user_search(client, request):
    _validate_keys(request, ("api_key", "pattern"))
    users = client.searchusers(_required_text(request, "pattern"))
    if not isinstance(users, list):
        raise AdapterError("python-bugzilla returned an invalid user list")
    return [_user_data(user) for user in users]


def _user_groups(client, request):
    _validate_keys(request, ("api_key", "email", "action", "groups"))
    action = _required_text(request, "action")
    if action not in {"add", "remove"}:
        raise AdapterError("action must be add or remove")
    return client.updateperms(
        _required_text(request, "email"),
        action,
        _required_string_list(request, "groups"),
    )


def _group_get(client, request):
    _validate_keys(request, ("api_key", "name", "membership"))
    group = client.getgroup(
        _required_text(request, "name"),
        membership=_required_bool(request, "membership"),
    )
    return _group_data(group)


def _group_list(client, request):
    _validate_keys(request, ("api_key", "names", "membership"))
    groups = client.getgroups(
        _required_string_list(request, "names"),
        membership=_required_bool(request, "membership"),
    )
    if not isinstance(groups, list):
        raise AdapterError("python-bugzilla returned an invalid group list")
    return [_group_data(group) for group in groups]


def _product_catalogue(client, request):
    _validate_keys(request, ("api_key", "catalogue"))
    catalogue = _required_text(request, "catalogue")
    if catalogue not in {"accessible", "enterable", "selectable"}:
        raise AdapterError("catalogue must be accessible, enterable, or selectable")
    result = client.product_get(ptype=catalogue)
    if not isinstance(result, list):
        raise AdapterError("python-bugzilla returned an invalid product list")
    return result


def _component_add(client, request):
    _validate_keys(request, ("api_key", "params"))
    return client.addcomponent(_required_mapping(request, "params", nonempty=True))


class _ComponentUpdateRecorder:
    def component_update(self, request):
        return {"request": request}


def _component_update_shape(_client, request):
    _validate_keys(request, ("api_key", "params"))
    client = Bugzilla(None, use_creds=False)
    client._backend = _ComponentUpdateRecorder()
    return client.editcomponent(_required_mapping(request, "params", nonempty=True))


def _required_url(request):
    value = _required_text(request, "url", 2048)
    parsed = urllib.parse.urlsplit(value)
    if parsed.scheme not in {"http", "https"} or not parsed.netloc or any(
        (parsed.username, parsed.password)
    ):
        raise AdapterError("url must be an HTTP(S) URL without embedded credentials")
    return value


def _auth_client(url, token_file=TOKEN_FILE):
    return Bugzilla(url, tokenfile=str(token_file), configpaths=[], force_rest=True)


def _login_operation(_client, request):
    _validate_keys(request, ("url", "username", "password", "restrict_login"))
    client = _auth_client(_required_url(request))
    result = client.login(
        _required_text(request, "username", 320),
        _required_text(request, "password", 1024),
        restrict_login=_required_bool(request, "restrict_login"),
    )
    if not result or not client._tokencache.get_value(client.url):
        raise AdapterError("python-bugzilla did not cache a login token")
    TOKEN_FILE.chmod(0o600)
    result = {"authenticated": True, "restricted": request["restrict_login"],
              "cache_written": True}
    return {"transport": _transport(client), "result": result}


def _cached_auth_operation(_client, request):
    _validate_keys(request, ("url", "username"))
    client = _auth_client(_required_url(request))
    if "Bugzilla_token" not in client._session.get_auth_params():
        raise AdapterError("python-bugzilla did not load a cached token")
    user = client.getuser(_required_text(request, "username", 320))
    if user is None:
        raise AdapterError("cached authentication did not identify the user")
    return {"transport": _transport(client),
            "result": {"authenticated": True, "cache_used": True}}


def _api_key_identity_operation(_client, request):
    _validate_keys(request, ("url", "api_key", "username"))
    url = _required_url(request)
    api_key = _required_text(request, "api_key", 4096)
    username = _required_text(request, "username", 320)
    client = Bugzilla(url, api_key=api_key, use_creds=False, force_rest=True)
    user = client.getuser(username)
    if user is None:
        raise AdapterError("API-key authentication did not identify a user")
    identity_matched = any(
        getattr(user, attribute, None) == username for attribute in ("email", "name")
    )
    return {"transport": _transport(client),
            "result": {"authenticated": True, "identity_matched": identity_matched}}


def _logout_operation(_client, request):
    _validate_keys(request, ("url", "username"))
    url = _required_url(request)
    client = _auth_client(url)
    token = client._session.get_auth_params().get("Bugzilla_token")
    if not token:
        raise AdapterError("python-bugzilla did not load a cached token")
    transport = _transport(client)
    stale_client = _auth_client(url, STALE_TOKEN_FILE)
    stale_client._tokencache.set_value(url, token)
    STALE_TOKEN_FILE.chmod(0o600)
    try:
        client.logout()
        try:
            stale_client.getuser(_required_text(request, "username", 320))
        except BugzillaError:
            pass
        else:
            raise AdapterError("python-bugzilla logout left the token valid")
    finally:
        STALE_TOKEN_FILE.unlink(missing_ok=True)
        client._tokencache.set_value(url, None)
    if client._tokencache.get_value(url) is not None:
        raise AdapterError("python-bugzilla did not clear the cached token")
    return {"transport": transport,
            "result": {"logged_out": True, "cache_cleared": True}}


class _CertificateProbeBackend:
    def __init__(self, _url, session):
        self.session = session

    @staticmethod
    def bugzilla_version():
        return {"version": "5.0"}


def _client_certificate_surface(_client, request):
    _validate_keys(request, ("certificate",))
    certificate = _attachment_path(request, "certificate")
    client = Bugzilla(None, use_creds=False, cert=str(certificate))
    client._get_backend_class = lambda url: (_CertificateProbeBackend, url)
    client.connect("https://127.0.0.1")
    session = getattr(getattr(client, "_session", None), "_session", None)
    return {
        "transport": None,
        "result": {"configured": getattr(session, "cert", None) == str(certificate)},
    }


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
    "comment_add": _comment_add,
    "comment_list": _comment_list,
    "attachment_upload": _attachment_upload,
    "attachment_list": _attachment_list,
    "attachment_get": _attachment_get,
    "attachment_download": _attachment_download,
    "attachment_cli_download_bug": _attachment_cli_download_bug,
    "attachment_flag": _attachment_flag,
    "user_create": _user_create,
    "user_get": _user_get,
    "user_search": _user_search,
    "user_groups": _user_groups,
    "group_get": _group_get,
    "group_list": _group_list,
    "product_catalogue": _product_catalogue,
    "component_add": _component_add,
    "component_update_shape": _component_update_shape,
    "login": _login_operation,
    "cached_auth": _cached_auth_operation,
    "api_key_identity": _api_key_identity_operation,
    "logout": _logout_operation,
    "client_certificate_surface": _client_certificate_surface,
}

LEGACY_OPERATIONS = {
    "create",
    "query",
    "update",
    "view",
    "history",
    "saved_search",
    "generic_fields",
    "update_options",
    "match_type",
    "bug_tags",
}
LOCAL_OPERATIONS = {"component_update_shape"}
SELF_MANAGED_OPERATIONS = {
    "login", "cached_auth", "api_key_identity", "logout", "client_certificate_surface"
}


def _work_path(value, label):
    path = Path(value)
    if not path.is_absolute():
        raise AdapterError(f"{label} path must be absolute")
    if path.is_symlink():
        raise AdapterError(f"{label} path must not be a symlink")
    try:
        parent = path.parent.resolve(strict=True)
    except OSError as error:
        raise AdapterError(f"{label} parent is unreadable") from error
    resolved = parent / path.name
    if resolved != WORK_ROOT and WORK_ROOT not in resolved.parents:
        raise AdapterError(f"{label} path is outside the exchange directory")
    return resolved


def _load_request(path):
    try:
        metadata = path.lstat()
        if not stat.S_ISREG(metadata.st_mode):
            raise AdapterError("input must be a regular non-symlink file")
        mode = stat.S_IMODE(metadata.st_mode)
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
    return request


def _transport(client):
    backend = getattr(client, "_backend", None)
    name = type(backend).__name__ if backend is not None else ""
    transports = {
        "_BackendREST": "REST",
        "_BackendXMLRPC": "XMLRPC",
    }
    try:
        return transports[name]
    except KeyError as error:
        if not name:
            raise AdapterError("python-bugzilla did not select a backend") from error
        raise AdapterError(f"unsupported python-bugzilla backend: {name}") from error


def _requested_transport(request, operation):
    if operation in SELF_MANAGED_OPERATIONS and "transport" in request:
        raise AdapterError("auth operation does not accept transport")
    requested = request.pop("transport", None)
    if operation in LOCAL_OPERATIONS:
        if requested is not None:
            raise AdapterError("local operation does not accept transport")
        return None
    if operation in LEGACY_OPERATIONS:
        if requested is not None:
            raise AdapterError("legacy operation does not accept transport")
        return None
    if requested is not None and requested not in {"REST", "XMLRPC"}:
        raise AdapterError("transport must be REST or XMLRPC")
    return requested


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
    input_label = "python-bugzilla-adapter.py"
    try:
        os.umask(0o077)
        if len(argv) != 4:
            raise AdapterError("usage: python-bugzilla-adapter.py OP INPUT OUTPUT")
        operation = argv[1]
        handler = OPERATIONS.get(operation)
        if handler is None:
            raise AdapterError("unsupported operation")
        input_path = _work_path(argv[2], "input")
        output_path = _work_path(argv[3], "output")
        if input_path == output_path:
            raise AdapterError("input and output paths must differ")
        request = _load_request(input_path)
        requested_transport = _requested_transport(request, operation)
        if operation in SELF_MANAGED_OPERATIONS:
            if input_path.parent != WORK_ROOT or output_path.parent != WORK_ROOT:
                raise AdapterError("auth operation paths must be direct exchange children")
            payload = handler(None, request)
            _write_output(output_path, payload)
            return 0
        if operation in LOCAL_OPERATIONS:
            result = handler(None, request)
            observed_transport = None
        else:
            api_key = request.get("api_key")
            if not isinstance(api_key, str) or not api_key:
                raise AdapterError("api_key must be a non-empty string")
            client = Bugzilla(
                SERVER_URL,
                api_key=api_key,
                use_creds=False,
                force_rest=(
                    requested_transport == "REST" or operation == "update_options"
                ),
                force_xmlrpc=(
                    requested_transport == "XMLRPC" or operation == "bug_tags"
                ),
            )
            result = handler(client, request)
            observed_transport = _transport(client)
        _write_output(
            output_path,
            {"transport": observed_transport, "result": result},
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
