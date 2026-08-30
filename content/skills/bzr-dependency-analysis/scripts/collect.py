#!/usr/bin/env python3
"""Collect deterministic, bounded Bugzilla dependency evidence through bzr."""

import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
from urllib.parse import parse_qsl, urlparse


SCHEMA = "bzr-dependency-collection/v1"
BZR_SCHEMA_VERSION = "1.0.0"
MAX_NODES = 9_999
MAX_RELATIONSHIPS = 9_999
TIMESTAMP_PATTERN = re.compile(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z\Z")
DETAIL_FIELDS = [
    "id",
    "summary",
    "status",
    "resolution",
    "assigned_to",
    "last_change_time",
    "blocks",
    "depends_on",
]
SCOPE_RANK = {
    "bug-ids": 0,
    "alias": 1,
    "saved-query": 2,
    "custom-search": 3,
    "product": 4,
    "milestone": 5,
    "version": 6,
    "restriction": 7,
}
CUSTOM_PARAMETER_ALLOWLIST = {
    "assigned_to",
    "bug_status",
    "component",
    "creator",
    "keywords",
    "priority",
    "product",
    "qa_contact",
    "reporter",
    "resolution",
    "severity",
    "status",
    "target_milestone",
    "version",
}
CREDENTIAL_PARAMETER_NAMES = {"bugzilla_api_key", "token", "api_key"}
ERROR_STRING_KEYS = {
    "type",
    "message",
    "field",
    "value",
    "last_change_time",
    "if_match_token",
    "resource",
    "identifier",
    "server",
    "expected",
    "actual",
}
ERROR_INTEGER_KEYS = {
    "exit_code",
    "bug_id",
    "status",
    "api_code",
    "succeeded",
    "failed",
}
ERROR_KEYS = ERROR_STRING_KEYS | ERROR_INTEGER_KEYS
FATAL_LIMITATIONS = {
    "api": "collection-api",
    "http": "collection-http",
    "auth": "collection-auth",
    "tls": "collection-tls",
}


class PolicyError(ValueError):
    """The local policy does not match the collection input contract."""


class FatalCollection(RuntimeError):
    """A run-wide failure that must produce a sanitized partial document."""

    def __init__(self, limitation, error_type):
        super().__init__(limitation)
        self.limitation = limitation
        self.error_type = error_type


def unique_object(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise PolicyError(f"duplicate JSON key: {key}")
        value[key] = item
    return value


def exact_keys(value, expected, context):
    if not isinstance(value, dict):
        raise PolicyError(f"{context} must be an object")
    actual = set(value)
    if actual != set(expected):
        raise PolicyError(f"{context} has invalid keys")


def nonempty_string(value, context):
    if not isinstance(value, str) or not value:
        raise PolicyError(f"{context} must be a non-empty string")
    return value


def positive_integer(value, context):
    if type(value) is not int or value <= 0:
        raise PolicyError(f"{context} must be a positive integer")
    return value


def validate_server(scope, servers, context):
    server = nonempty_string(scope["server"], f"{context}.server")
    if server not in servers:
        raise PolicyError(f"{context}.server is not declared")
    return server


def validate_scope(scope, servers, index):
    context = f"scopes[{index}]"
    if not isinstance(scope, dict) or "kind" not in scope:
        raise PolicyError(f"{context} must have a kind")
    kind = scope["kind"]
    if kind == "bug-ids":
        exact_keys(scope, {"kind", "server", "ids"}, context)
        if not isinstance(scope["ids"], list) or not scope["ids"]:
            raise PolicyError(f"{context}.ids must be a non-empty array")
        ids = [positive_integer(value, f"{context}.ids") for value in scope["ids"]]
        normalized = {"kind": kind, "server": validate_server(scope, servers, context), "ids": ids}
    elif kind == "alias":
        exact_keys(scope, {"kind", "server", "alias"}, context)
        alias = nonempty_string(scope["alias"], f"{context}.alias")
        if alias.isdecimal():
            raise PolicyError(f"{context}.alias must be nonnumeric")
        normalized = {"kind": kind, "server": validate_server(scope, servers, context), "alias": alias}
    elif kind == "saved-query":
        exact_keys(scope, {"kind", "server", "name"}, context)
        normalized = {
            "kind": kind,
            "server": validate_server(scope, servers, context),
            "name": nonempty_string(scope["name"], f"{context}.name"),
        }
    elif kind == "custom-search":
        exact_keys(scope, {"kind", "server", "url", "parameter_names"}, context)
        url = validate_custom_url(scope["url"], context)
        names = validate_parameter_names(scope["parameter_names"], context)
        derived_names = sorted(
            {
                name
                for name, _ in parse_qsl(urlparse(url).query, keep_blank_values=True)
                if name in CUSTOM_PARAMETER_ALLOWLIST
            }
        )
        if names != derived_names:
            raise PolicyError(f"{context}.parameter_names must match the URL's allowlisted names")
        normalized = {
            "kind": kind,
            "server": validate_server(scope, servers, context),
            "url": url,
            "parameter_names": names,
        }
    elif kind in {"product", "milestone", "version"}:
        exact_keys(scope, {"kind", "server", "value"}, context)
        normalized = {
            "kind": kind,
            "server": validate_server(scope, servers, context),
            "value": nonempty_string(scope["value"], f"{context}.value"),
        }
    else:
        raise PolicyError(f"{context}.kind is unsupported")
    return normalized


def validate_custom_url(value, context):
    url = nonempty_string(value, f"{context}.url")
    parsed = urlparse(url)
    if parsed.scheme not in {"http", "https"} or not parsed.netloc:
        raise PolicyError(f"{context}.url must be an absolute HTTP(S) URL")
    query_names = {
        name.casefold()
        for name, _ in parse_qsl(parsed.query, keep_blank_values=True)
    }
    if (
        parsed.username is not None
        or parsed.password is not None
        or query_names & CREDENTIAL_PARAMETER_NAMES
    ):
        raise PolicyError(f"{context}.url must not include credentials")
    return url


def validate_parameter_names(value, context):
    if not isinstance(value, list):
        raise PolicyError(f"{context}.parameter_names must be an array")
    names = []
    for item in value:
        name = nonempty_string(item, f"{context}.parameter_names")
        if name not in CUSTOM_PARAMETER_ALLOWLIST:
            raise PolicyError(f"{context}.parameter_names contains an unsupported name")
        names.append(name)
    return sorted(set(names))


def validate_restriction(value, servers):
    if value is None:
        return None
    if not isinstance(value, dict) or "kind" not in value:
        raise PolicyError("restriction must be null or an object with a kind")
    kind = value["kind"]
    if kind == "saved-query":
        exact_keys(value, {"kind", "server", "name"}, "restriction")
        return {
            "kind": kind,
            "server": validate_server(value, servers, "restriction"),
            "name": nonempty_string(value["name"], "restriction.name"),
        }
    if kind in {"product", "milestone", "version"}:
        exact_keys(value, {"kind", "server", "value"}, "restriction")
        return {
            "kind": kind,
            "server": validate_server(value, servers, "restriction"),
            "value": nonempty_string(value["value"], "restriction.value"),
        }
    raise PolicyError("restriction.kind is unsupported")


def validate_policy(value):
    required_keys = {
        "bounds",
        "bzr",
        "direction",
        "resolved_mode",
        "resolved_statuses",
        "restriction",
        "scopes",
        "servers",
        "stale_after_days",
    }
    if not isinstance(value, dict) or not required_keys.issubset(value):
        raise PolicyError("policy has invalid keys")
    if not set(value).issubset(required_keys | {"unassigned_assignees"}):
        raise PolicyError("policy has invalid keys")
    if not isinstance(value["bounds"], dict) or not set(value["bounds"]).issubset(
        {"max_depth", "max_nodes", "max_relationships"}
    ) or not {"max_depth", "max_nodes"}.issubset(value["bounds"]):
        raise PolicyError("bounds has invalid keys")
    max_nodes = positive_integer(value["bounds"]["max_nodes"], "bounds.max_nodes")
    if max_nodes > MAX_NODES:
        raise PolicyError(f"bounds.max_nodes must be at most {MAX_NODES}")
    bounds = {
        "max_depth": positive_integer(value["bounds"]["max_depth"], "bounds.max_depth"),
        "max_nodes": max_nodes,
        "max_relationships": positive_integer(
            value["bounds"].get("max_relationships", max_nodes),
            "bounds.max_relationships",
        ),
    }
    if bounds["max_relationships"] > MAX_RELATIONSHIPS:
        raise PolicyError(
            f"bounds.max_relationships must be at most {MAX_RELATIONSHIPS}"
        )
    if not isinstance(value["servers"], list) or not value["servers"]:
        raise PolicyError("servers must be a non-empty array")
    servers = [nonempty_string(server, "servers") for server in value["servers"]]
    if len(servers) != len(set(servers)):
        raise PolicyError("servers must be unique")
    if not isinstance(value["scopes"], list) or not value["scopes"]:
        raise PolicyError("scopes must be a non-empty array")
    server_set = set(servers)
    scopes = [
        validate_scope(scope, server_set, index)
        for index, scope in enumerate(value["scopes"])
    ]
    alias_servers = [scope["server"] for scope in scopes if scope["kind"] == "alias"]
    if len(alias_servers) != len(set(alias_servers)):
        raise PolicyError("at most one alias scope is allowed per server")
    restriction = validate_restriction(value["restriction"], server_set)
    referenced_servers = {scope["server"] for scope in scopes}
    if restriction is not None:
        referenced_servers.add(restriction["server"])
    if server_set != referenced_servers:
        raise PolicyError("servers must match scope and restriction servers")
    direction = value["direction"]
    if direction not in {"depends_on", "blocks", "both"}:
        raise PolicyError("direction is unsupported")
    resolved_mode = value["resolved_mode"]
    if resolved_mode not in {"include-traverse", "include-no-traverse"}:
        raise PolicyError("resolved_mode is unsupported")
    statuses = value["resolved_statuses"]
    if not isinstance(statuses, list) or not statuses:
        raise PolicyError("resolved_statuses must be a non-empty array")
    statuses = sorted(set(nonempty_string(status, "resolved_statuses") for status in statuses))
    unassigned_assignees = validate_unassigned_assignees(
        value.get("unassigned_assignees", {}),
        server_set,
    )
    return {
        "bounds": bounds,
        "bzr": nonempty_string(value["bzr"], "bzr"),
        "direction": direction,
        "resolved_mode": resolved_mode,
        "resolved_statuses": statuses,
        "restriction": restriction,
        "scopes": scopes,
        "servers": sorted(servers),
        "stale_after_days": positive_integer(value["stale_after_days"], "stale_after_days"),
        "unassigned_assignees": unassigned_assignees,
    }


def validate_unassigned_assignees(value, servers):
    if not isinstance(value, dict) or not set(value).issubset(servers):
        raise PolicyError("unassigned_assignees must map declared servers to login arrays")
    normalized = {}
    for server in sorted(value):
        logins = value[server]
        if not isinstance(logins, list) or any(
            not isinstance(login, str) or not login for login in logins
        ):
            raise PolicyError("unassigned_assignees values must be login arrays")
        if logins != sorted(set(logins)):
            raise PolicyError("unassigned_assignees login arrays must be sorted and unique")
        normalized[server] = logins
    return normalized


def analysis_timestamp(value):
    if value is None:
        return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    if not TIMESTAMP_PATTERN.fullmatch(value):
        raise PolicyError("analysis timestamp must use YYYY-MM-DDTHH:MM:SSZ")
    try:
        datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ")
    except ValueError as error:
        raise PolicyError("analysis timestamp is not a valid UTC instant") from error
    return value


class CommandRunner:
    def __init__(self, executable):
        self.executable = executable
        self.preflighted_servers = set()

    def preflight(self, server):
        data, resource_error = self.run_json(preflight_argv(server))
        if resource_error is not None:
            raise FatalCollection("collection-unclassified", resource_error)
        validate_id_page(data)
        self.preflighted_servers.add(server)

    def run_json(self, argv, resource_server=None):
        try:
            child_environment = os.environ.copy()
            child_environment["RUST_LOG"] = "off"
            result = subprocess.run(
                [self.executable, *argv],
                capture_output=True,
                text=True,
                encoding="utf-8",
                check=False,
                env=child_environment,
            )
        except (OSError, UnicodeError):
            raise FatalCollection("collection-transport", "transport")
        if result.returncode == 0:
            envelope = parse_json_object(result.stdout, "collection-malformed-output")
            if set(envelope) != {"schema_version", "data"}:
                raise FatalCollection("collection-malformed-output", "malformed-output")
            validate_envelope_version(envelope)
            return envelope["data"], None
        envelope = parse_json_object(result.stderr, "collection-malformed-output")
        if set(envelope) != {"schema_version", "error"}:
            raise FatalCollection("collection-malformed-output", "malformed-output")
        validate_envelope_version(envelope)
        error = validate_error_envelope(envelope["error"], result.returncode)
        if resource_server is not None:
            if error["type"] == "not_found" and error.get("resource") == "bug":
                return None, "not_found"
            if error["type"] == "api" and error.get("api_code") in {100, 101}:
                return None, "not_found"
            if (
                error["type"] == "api"
                and error.get("api_code") == 102
                and resource_server in self.preflighted_servers
            ):
                return None, "inaccessible"
        error_type = error["type"]
        limitation = FATAL_LIMITATIONS.get(error_type, "collection-unclassified")
        raise FatalCollection(limitation, error_type)


def parse_json_object(text, limitation):
    try:
        value = json.loads(text)
    except (json.JSONDecodeError, TypeError):
        raise FatalCollection(limitation, "malformed-output")
    if not isinstance(value, dict):
        raise FatalCollection(limitation, "malformed-output")
    return value


def validate_envelope_version(envelope):
    if envelope["schema_version"] != BZR_SCHEMA_VERSION:
        raise FatalCollection("collection-schema-version", "schema-version")


def validate_error_envelope(value, returncode):
    if not isinstance(value, dict) or not {"type", "message", "exit_code"}.issubset(value):
        raise FatalCollection("collection-malformed-output", "malformed-output")
    if not set(value).issubset(ERROR_KEYS):
        raise FatalCollection("collection-malformed-output", "malformed-output")
    for key, item in value.items():
        if key in ERROR_STRING_KEYS and not isinstance(item, str):
            raise FatalCollection("collection-malformed-output", "malformed-output")
        if key in ERROR_INTEGER_KEYS and type(item) is not int:
            raise FatalCollection("collection-malformed-output", "malformed-output")
    if not 1 <= value["exit_code"] <= 14 or value["exit_code"] != returncode:
        raise FatalCollection("collection-malformed-output", "malformed-output")
    return value


class Collector:
    def __init__(self, policy, timestamp, runner):
        self.policy = policy
        self.timestamp = timestamp
        self.runner = runner
        self.max_nodes = policy["bounds"]["max_nodes"]
        self.max_relationships = policy["bounds"]["max_relationships"]
        self.nodes = {}
        self.details = {}
        self.fetched = set()
        self.roots = set()
        self.staged = set()
        self.reciprocal_staged = set()
        self.rejected = set()
        self.limitations = set()
        self.scope_truncated = False
        self.graph_cap_reached = False
        self.relationship_cap_reached = False
        self.relationships_processed = 0
        self.omitted_relationships_lower_bound = 0
        self.membership = None
        self.provenance = self.build_provenance()

    def collect(self):
        try:
            for server in self.policy["servers"]:
                self.runner.preflight(server)
            if not self.collect_restriction():
                return self.document(), 0
            candidates = self.enumerate_scopes()
            self.admit_roots(candidates)
            self.fetch_breadth_first()
        except FatalCollection as error:
            self.limitations.add(error.limitation)
            self.interrupt_pending_fetches()
            return self.document(), 1
        return self.document(), 0

    def build_provenance(self):
        entries = [scope_provenance(scope) for scope in self.policy["scopes"]]
        restriction = self.policy["restriction"]
        if restriction is not None:
            entries.append(restriction_provenance(restriction))
        unique = {json.dumps(entry, sort_keys=True, separators=(",", ":")): entry for entry in entries}
        return sorted(unique.values(), key=provenance_key)

    def collect_restriction(self):
        restriction = self.policy["restriction"]
        if restriction is None or restriction["kind"] != "saved-query":
            return True
        ids, overflow = self.enumerate_list_scope(restriction)
        if overflow:
            self.scope_truncated = True
            self.limitations.add("restriction-node-cap")
            return False
        self.membership = {(restriction["server"], bug_id) for bug_id in ids}
        return True

    def enumerate_scopes(self):
        candidates = {server: set() for server in self.policy["servers"]}
        scopes = sorted(self.policy["scopes"], key=scope_sort_key)
        for scope in scopes:
            if scope["kind"] == "bug-ids":
                candidates[scope["server"]].update(scope["ids"])
            elif scope["kind"] != "alias":
                ids, overflow = self.enumerate_list_scope(scope)
                candidates[scope["server"]].update(ids)
                if overflow:
                    self.record_scope_truncation()
        return candidates

    def enumerate_list_scope(self, scope):
        collected = []
        offset = 0
        ceiling = self.max_nodes + 1
        while len(collected) < ceiling:
            limit = ceiling - len(collected)
            argv = list_scope_argv(scope, limit, offset)
            data, resource_error = self.runner.run_json(argv)
            if resource_error is not None:
                raise FatalCollection("collection-unclassified", resource_error)
            page = validate_id_page(data)
            if len(page) > limit:
                raise FatalCollection("collection-malformed-output", "malformed-output")
            collected.extend(page)
            if not page or len(collected) == ceiling:
                break
            offset += len(page)
        unique = sorted(set(collected))
        overflow = len(collected) == ceiling or len(unique) > self.max_nodes
        return unique[: self.max_nodes], overflow

    def admit_roots(self, candidates):
        aliases = {
            scope["server"]: scope
            for scope in self.policy["scopes"]
            if scope["kind"] == "alias"
        }
        for server in self.policy["servers"]:
            if server in aliases:
                self.resolve_alias_root(aliases[server])
            for bug_id in sorted(candidates[server]):
                key = (server, bug_id)
                if key in self.nodes:
                    self.roots.add((server, bug_id, str(bug_id)))
                    continue
                if len(self.nodes) >= self.max_nodes:
                    self.record_scope_truncation()
                    continue
                reason = self.membership_boundary(key)
                self.nodes[key] = boundary_node(server, bug_id, str(bug_id), 0, reason)
                self.roots.add((server, bug_id, str(bug_id)))

    def resolve_alias_root(self, scope):
        server = scope["server"]
        alias = scope["alias"]
        if len(self.nodes) >= self.max_nodes:
            self.record_scope_truncation()
            return
        alias_key = (server, alias)
        self.nodes[alias_key] = pending_node(server, None, alias, 0)
        self.roots.add((server, None, alias))
        data, resource_error = self.runner.run_json(
            view_argv(server, alias, self.restriction_field()),
            resource_server=server,
        )
        if resource_error is not None:
            self.nodes[alias_key] = unknown_node(server, None, alias, 0, resource_error)
            self.fetched.add(alias_key)
            return
        detail = validate_bug(data, self.restriction_field())
        numeric_key = (server, detail["id"])
        del self.nodes[alias_key]
        self.roots.remove((server, None, alias))
        self.roots.add((server, detail["id"], str(detail["id"])))
        self.fetched.add(numeric_key)
        reason = self.membership_boundary(numeric_key)
        if reason != "pending_fetch":
            self.nodes[numeric_key] = boundary_node(
                server, detail["id"], str(detail["id"]), 0, reason, [alias]
            )
            return
        self.nodes[numeric_key] = known_node(server, detail, 0, [alias])
        if not self.apply_field_restriction(numeric_key, detail):
            return
        self.stage_detail(
            server,
            detail,
            selected_eligible=self.selected_adjacency_eligible(detail),
        )

    def fetch_breadth_first(self):
        frontier = sorted(
            (key for key in self.nodes if isinstance(key[1], int) and self.nodes[key]["depth"] == 0),
            key=numeric_identity_key,
        )
        depth = 0
        while frontier:
            for index, key in enumerate(frontier):
                if self.nodes[key]["boundary_reason"] == "pending_fetch" and key not in self.fetched:
                    self.fetch_numeric(key)
                if (
                    not self.relationship_cap_reached
                    and self.relationships_processed == self.max_relationships
                    and index + 1 < len(frontier)
                ):
                    self.record_relationship_cap()
            candidates = self.discovery_candidates(frontier)
            next_frontier = self.admit_discoveries(candidates, depth + 1)
            if (
                self.relationships_processed == self.max_relationships
                and next_frontier
            ):
                self.record_relationship_cap()
            if self.graph_cap_reached or self.relationship_cap_reached:
                break
            frontier = next_frontier
            depth += 1

    def fetch_numeric(self, key):
        server, bug_id = key
        data, resource_error = self.runner.run_json(
            view_argv(server, str(bug_id), self.restriction_field()),
            resource_server=server,
        )
        if resource_error is not None:
            self.nodes[key] = unknown_node(server, bug_id, str(bug_id), self.nodes[key]["depth"], resource_error)
            self.fetched.add(key)
            return
        detail = validate_bug(data, self.restriction_field())
        if detail["id"] != bug_id:
            raise FatalCollection("collection-malformed-output", "malformed-output")
        aliases = self.nodes[key]["requested_aliases"]
        self.nodes[key] = known_node(server, detail, self.nodes[key]["depth"], aliases)
        self.fetched.add(key)
        if not self.apply_field_restriction(key, detail):
            return
        self.stage_detail(
            server,
            detail,
            selected_eligible=self.selected_adjacency_eligible(detail),
        )

    def selected_adjacency_eligible(self, detail):
        return not (
            self.policy["resolved_mode"] == "include-no-traverse"
            and detail["status"] in self.policy["resolved_statuses"]
        )

    def discovery_candidates(self, frontier):
        selected = selected_fields(self.policy["direction"])
        candidates = set()
        for key in frontier:
            node = self.nodes[key]
            if node["state"] != "known" or key not in self.details:
                continue
            if (
                self.policy["resolved_mode"] == "include-no-traverse"
                and node["status"] in self.policy["resolved_statuses"]
            ):
                continue
            for field in selected:
                for bug_id in self.details[key][field]:
                    candidates.add((key[0], bug_id))
        return sorted(candidates, key=numeric_identity_key)

    def admit_discoveries(self, candidates, depth):
        next_frontier = []
        for key in candidates:
            if key in self.nodes:
                continue
            if len(self.nodes) >= self.max_nodes:
                self.rejected.add(key)
                self.record_graph_cap()
                continue
            reason = "depth_limit" if depth > self.policy["bounds"]["max_depth"] else self.membership_boundary(key)
            self.nodes[key] = boundary_node(key[0], key[1], str(key[1]), depth, reason)
            if reason == "pending_fetch":
                next_frontier.append(key)
            if len(self.nodes) == self.max_nodes:
                self.record_graph_cap()
        return sorted(next_frontier, key=numeric_identity_key)

    def stage_detail(self, server, detail, *, selected_eligible):
        key = (server, detail["id"])
        retained = {"blocks": [], "depends_on": []}
        relationships = {
            field: canonical_relationship_ids(detail[field])
            for field in ("blocks", "depends_on")
        }
        selected = selected_fields(self.policy["direction"])
        if selected_eligible:
            for field_index, field in enumerate(selected):
                field_relationships = relationships[field]
                index = 0
                while (
                    index < len(field_relationships)
                    and self.relationships_processed < self.max_relationships
                ):
                    target = field_relationships[index]
                    self.relationships_processed += 1
                    retained[field].append(target)
                    self.staged.add((field, server, detail["id"], server, target))
                    index += 1
                if index < len(field_relationships):
                    self.omitted_relationships_lower_bound += len(field_relationships) - index
                    self.record_relationship_cap()
                if self.relationship_cap_reached:
                    for later_field in selected[field_index + 1:]:
                        self.omitted_relationships_lower_bound += len(
                            relationships[later_field]
                        )
                    break
        if self.policy["direction"] != "both":
            reciprocal_field = "blocks" if selected[0] == "depends_on" else "depends_on"
            available = self.max_relationships - len(self.reciprocal_staged)
            for target in relationships[reciprocal_field][:available]:
                self.reciprocal_staged.add(
                    (reciprocal_field, server, detail["id"], server, target)
                )
        self.details[key] = {
            field: sorted(set(values)) for field, values in retained.items()
        }

    def apply_field_restriction(self, key, detail):
        restriction = self.policy["restriction"]
        if restriction is None or restriction["kind"] == "saved-query" or restriction["server"] != key[0]:
            return True
        field = restriction_field_name(restriction["kind"])
        if detail[field] != restriction["value"]:
            aliases = self.nodes[key]["requested_aliases"]
            self.nodes[key] = boundary_node(key[0], key[1], str(key[1]), self.nodes[key]["depth"], "scope_restriction", aliases)
            return False
        return True

    def membership_boundary(self, key):
        restriction = self.policy["restriction"]
        if self.membership is not None and restriction["server"] == key[0] and key not in self.membership:
            return "scope_restriction"
        return "pending_fetch"

    def restriction_field(self):
        restriction = self.policy["restriction"]
        if restriction is None or restriction["kind"] == "saved-query":
            return None
        return restriction_field_name(restriction["kind"])

    def record_scope_truncation(self):
        self.scope_truncated = True
        self.limitations.add("scope-node-cap")

    def record_graph_cap(self):
        self.graph_cap_reached = True
        self.limitations.add("graph-node-cap")

    def record_relationship_cap(self):
        self.relationship_cap_reached = True
        self.limitations.add("relationship_cap")

    def interrupt_pending_fetches(self):
        for key, node in list(self.nodes.items()):
            if node["state"] == "boundary" and node["boundary_reason"] == "pending_fetch":
                self.nodes[key] = boundary_node(
                    node["server"], node["id"], node["requested"], node["depth"],
                    "fetch_interrupted", node["requested_aliases"],
                )

    def normalized_observations(self):
        endpoints = {key for key in self.nodes if isinstance(key[1], int)}
        selected = [
            item
            for item in self.staged
            if (item[1], item[2]) in endpoints and (item[3], item[4]) in endpoints
        ]
        canonical = {canonical_edge(item) for item in selected}
        reciprocal = [
            item
            for item in self.reciprocal_staged
            if (item[1], item[2]) in endpoints
            and (item[3], item[4]) in endpoints
            and canonical_edge(item) in canonical
        ]
        retained = [*selected, *reciprocal]
        retained.sort(key=lambda item: (item[1], item[2], item[3], item[4], item[0]))
        return [
            {
                "field": field,
                "source": {"id": source_id, "server": source_server},
                "target": {"id": target_id, "server": target_server},
            }
            for field, source_server, source_id, target_server, target_id in retained
        ]

    def document(self):
        nodes = sorted(self.nodes.values(), key=node_sort_key)
        roots = [
            {"server": server, "id": bug_id, "requested": requested}
            for server, bug_id, requested in sorted(self.roots, key=root_sort_key)
        ]
        status = "partial" if self.limitations else "complete"
        return {
            "analysis_timestamp": self.timestamp,
            "bounds": self.policy["bounds"],
            "cap": {
                "graph_cap_reached": self.graph_cap_reached,
                "omitted_discovered_identities": len(self.rejected),
                "omitted_relationships_lower_bound": self.omitted_relationships_lower_bound,
                "relationship_cap_reached": self.relationship_cap_reached,
                "scope_truncated": self.scope_truncated,
            },
            "limitations": sorted(self.limitations),
            "nodes": nodes,
            "observations": self.normalized_observations(),
            "provenance": self.provenance,
            "policy": {
                "direction": self.policy["direction"],
                "duration": None,
                "resolved_mode": self.policy["resolved_mode"],
                "resolved_statuses": self.policy["resolved_statuses"],
                "stale_after_days": self.policy["stale_after_days"],
                "unassigned_assignees": self.policy["unassigned_assignees"],
            },
            "roots": roots,
            "schema": SCHEMA,
            "status": status,
        }


def selected_fields(direction):
    if direction == "both":
        return ("blocks", "depends_on")
    return (direction,)


def canonical_relationship_ids(values):
    if any(type(value) is not int or value <= 0 for value in values):
        raise FatalCollection("collection-malformed-output", "malformed-output")
    return sorted(set(values))


def canonical_edge(observation):
    field, source_server, source_id, target_server, target_id = observation
    if field == "depends_on":
        return target_server, target_id, source_server, source_id
    return source_server, source_id, target_server, target_id


def pending_node(server, bug_id, requested, depth, aliases=None):
    return boundary_node(server, bug_id, requested, depth, "pending_fetch", aliases)


def boundary_node(server, bug_id, requested, depth, reason, aliases=None):
    return {
        "assigned_to": None,
        "boundary_reason": reason,
        "depth": depth,
        "error_type": None,
        "id": bug_id,
        "last_change_time": None,
        "provenance": {"command": "bug view", "server": server},
        "requested": requested,
        "requested_aliases": sorted(aliases or []),
        "resolution": None,
        "server": server,
        "state": "boundary",
        "status": None,
        "summary": None,
    }


def unknown_node(server, bug_id, requested, depth, error_type):
    node = boundary_node(server, bug_id, requested, depth, "pending_fetch")
    node["boundary_reason"] = None
    node["error_type"] = error_type
    node["state"] = "unknown"
    return node


def known_node(server, detail, depth, aliases):
    return {
        "assigned_to": detail["assigned_to"],
        "boundary_reason": None,
        "depth": depth,
        "error_type": None,
        "id": detail["id"],
        "last_change_time": detail["last_change_time"],
        "provenance": {"command": "bug view", "server": server},
        "requested": str(detail["id"]),
        "requested_aliases": sorted(set(aliases)),
        "resolution": detail["resolution"],
        "server": server,
        "state": "known",
        "status": detail["status"],
        "summary": detail["summary"],
    }


def validate_bug(value, extra_field):
    expected = set(DETAIL_FIELDS)
    if extra_field is not None:
        expected.add(extra_field)
    if not isinstance(value, dict) or set(value) != expected:
        raise FatalCollection("collection-malformed-output", "malformed-output")
    if type(value["id"]) is not int or value["id"] <= 0:
        raise FatalCollection("collection-malformed-output", "malformed-output")
    if not isinstance(value["summary"], str) or not isinstance(value["status"], str):
        raise FatalCollection("collection-malformed-output", "malformed-output")
    for field in ("resolution", "assigned_to", "last_change_time"):
        if value[field] is not None and not isinstance(value[field], str):
            raise FatalCollection("collection-malformed-output", "malformed-output")
    for field in ("blocks", "depends_on"):
        if not isinstance(value[field], list):
            raise FatalCollection("collection-malformed-output", "malformed-output")
    if extra_field is not None and not isinstance(value[extra_field], str):
        raise FatalCollection("collection-malformed-output", "malformed-output")
    return value


def validate_id_page(value):
    if not isinstance(value, list):
        raise FatalCollection("collection-malformed-output", "malformed-output")
    ids = []
    for item in value:
        if not isinstance(item, dict) or set(item) != {"id"}:
            raise FatalCollection("collection-malformed-output", "malformed-output")
        if type(item["id"]) is not int or item["id"] <= 0:
            raise FatalCollection("collection-malformed-output", "malformed-output")
        ids.append(item["id"])
    return ids


def view_argv(server, requested, extra_field):
    fields = DETAIL_FIELDS + ([extra_field] if extra_field is not None else [])
    return [
        "--server",
        server,
        "--json",
        "bug",
        "view",
        str(requested),
        "--fields",
        ",".join(fields),
    ]


def preflight_argv(server):
    return [
        "--server",
        server,
        "--json",
        "bug",
        "list",
        "--limit",
        "1",
        "--offset",
        "0",
        "--fields",
        "id",
        "--sort",
        "bug_id",
        "--order",
        "asc",
    ]


def list_scope_argv(scope, limit, offset):
    prefix = ["--server", scope["server"], "--json"]
    kind = scope["kind"]
    if kind == "saved-query":
        command = ["query", "run", scope["name"]]
    elif kind == "custom-search":
        command = ["bug", "search", "--from-url", scope["url"]]
    else:
        flag = {"product": "--product", "milestone": "--target-milestone", "version": "--version"}[kind]
        command = ["bug", "list", flag, scope["value"]]
    return [
        *prefix,
        *command,
        "--limit",
        str(limit),
        "--offset",
        str(offset),
        "--fields",
        "id",
        "--sort",
        "bug_id",
        "--order",
        "asc",
    ]


def restriction_field_name(kind):
    return {"product": "product", "milestone": "target_milestone", "version": "version"}[kind]


def scope_provenance(scope):
    kind = scope["kind"]
    source = scope["name"] if kind == "saved-query" else None
    if kind == "custom-search":
        names = scope["parameter_names"]
    elif kind in {"product", "milestone", "version"}:
        names = [restriction_field_name(kind)]
    else:
        names = []
    return {
        "parameter_names": sorted(set(names)),
        "scope_kind": kind,
        "source": source,
        "server": scope["server"],
    }


def restriction_provenance(restriction):
    source = restriction["name"] if restriction["kind"] == "saved-query" else None
    names = [] if restriction["kind"] == "saved-query" else [restriction_field_name(restriction["kind"])]
    return {
        "parameter_names": names,
        "scope_kind": "restriction",
        "source": source,
        "server": restriction["server"],
    }


def provenance_key(entry):
    source = entry["source"]
    return (
        entry["server"],
        SCOPE_RANK[entry["scope_kind"]],
        source is None,
        "" if source is None else source,
        tuple(entry["parameter_names"]),
    )


def scope_sort_key(scope):
    source = scope.get("name") or scope.get("value") or scope.get("alias") or scope.get("url") or ""
    return scope["server"], SCOPE_RANK[scope["kind"]], source


def numeric_identity_key(key):
    return key[0], key[1]


def node_sort_key(node):
    return (
        node["server"],
        node["depth"],
        node["id"] is None,
        -1 if node["id"] is None else node["id"],
        node["requested"],
    )


def root_sort_key(root):
    server, bug_id, requested = root
    return server, bug_id is None, -1 if bug_id is None else bug_id, requested


def atomic_write(path, document):
    output = Path(path)
    output.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary = tempfile.mkstemp(prefix=f".{output.name}.", dir=str(output.parent))
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            json.dump(document, handle, indent=2, sort_keys=True, ensure_ascii=False)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, output)
    except BaseException:
        try:
            os.unlink(temporary)
        except FileNotFoundError:
            pass
        raise


def parse_arguments(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--policy", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--runner")
    parser.add_argument("--analysis-timestamp")
    return parser.parse_args(argv)


def load_policy(path):
    try:
        with open(path, encoding="utf-8") as handle:
            return validate_policy(json.load(handle, object_pairs_hook=unique_object))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise PolicyError("policy must be readable valid JSON") from error


def main(argv=None):
    args = parse_arguments(argv)
    try:
        timestamp = analysis_timestamp(args.analysis_timestamp)
        policy = load_policy(args.policy)
    except PolicyError as error:
        sys.stderr.write(f"policy error: {error}\n")
        return 2
    runner = CommandRunner(args.runner or policy["bzr"])
    document, exit_status = Collector(policy, timestamp, runner).collect()
    try:
        atomic_write(args.output, document)
    except OSError:
        sys.stderr.write("collection output error: unable to write output\n")
        return 2
    if exit_status:
        error_type = next(
            (code.removeprefix("collection-") for code in document["limitations"] if code.startswith("collection-")),
            "unclassified",
        )
        sys.stderr.write(f"collection failed: {error_type}\n")
    return exit_status


if __name__ == "__main__":
    sys.exit(main())
