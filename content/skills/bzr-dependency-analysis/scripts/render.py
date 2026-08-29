#!/usr/bin/env python3
"""Render a strict dependency analysis as safe Markdown or Mermaid text."""

import argparse
from datetime import datetime
import json
import os
from pathlib import Path
import re
import sys
import tempfile


SCHEMA = "bzr-dependency-analysis/v1"
MAX_NODES = 9_999
TOP_LEVEL_KEYS = {
    "analysis_timestamp", "bounds", "cap", "components", "edges", "findings",
    "layers", "limitations", "longest_chain", "nodes", "policy", "provenance",
    "roots", "schema", "status", "warnings",
}
NODE_KEYS = {
    "assigned_to", "boundary_reason", "depth", "error_type", "id", "last_change_time",
    "provenance", "requested", "requested_aliases", "resolution", "server", "stale",
    "state", "status", "summary",
}
IDENTITY_KEYS = {"id", "requested", "server"}
TIMESTAMP_PATTERN = re.compile(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z\Z")
COMPONENT_PATTERN = re.compile(r"c\d{4}\Z")
SCOPE_KINDS = {
    "alias", "bug-ids", "custom-search", "milestone", "product", "restriction",
    "saved-query", "version",
}
SCOPE_RANK = {
    "bug-ids": 0, "alias": 1, "saved-query": 2, "custom-search": 3,
    "product": 4, "milestone": 5, "version": 6, "restriction": 7,
}
PARAMETER_NAMES = {
    "assigned_to", "bug_status", "component", "creator", "keywords", "priority",
    "product", "qa_contact", "reporter", "resolution", "severity", "status",
    "target_milestone", "version",
}


class RenderInputError(ValueError):
    """The input does not match the versioned analysis contract."""


def exact_keys(value, expected, context):
    if not isinstance(value, dict):
        raise RenderInputError(f"{context} must be an object")
    if set(value) != set(expected):
        raise RenderInputError(f"{context} has invalid keys")


def string(value, context, *, optional=False):
    if optional and value is None:
        return
    if not isinstance(value, str) or not value:
        raise RenderInputError(f"{context} must be a non-empty string")


def integer(value, context, *, positive=False):
    if type(value) is not int or value < (1 if positive else 0):
        qualifier = "positive" if positive else "non-negative"
        raise RenderInputError(f"{context} must be a {qualifier} integer")


def boolean(value, context):
    if type(value) is not bool:
        raise RenderInputError(f"{context} must be a boolean")


def validate_cap_relationships(cap, limitations):
    graph_limited = "graph-node-cap" in limitations
    graph_cap_reached = cap["graph_cap_reached"]
    has_omissions = cap["omitted_discovered_identities"] > 0
    if graph_limited != graph_cap_reached or (has_omissions and not graph_cap_reached):
        raise RenderInputError("graph cap metadata is inconsistent")

    scope_limitations = set(limitations) & {
        "restriction-node-cap",
        "scope-node-cap",
    }
    if len(scope_limitations) > 1 or cap["scope_truncated"] != bool(scope_limitations):
        raise RenderInputError("scope cap metadata is inconsistent")


def sorted_strings(value, context, *, allowed=None, nonempty=False):
    if not isinstance(value, list) or any(
        not isinstance(item, str) or not item for item in value
    ):
        raise RenderInputError(f"{context} must be an array of non-empty strings")
    if nonempty and not value:
        raise RenderInputError(f"{context} must not be empty")
    if value != sorted(set(value)):
        raise RenderInputError(f"{context} must be sorted and unique")
    if allowed is not None and not set(value).issubset(allowed):
        raise RenderInputError(f"{context} contains an unsupported value")


def identity_tuple_key(value):
    return value[0], value[1] is None, -1 if value[1] is None else value[1], value[2] or ""


def validate_identity(value, context, *, root=False):
    exact_keys(value, IDENTITY_KEYS, context)
    string(value["server"], f"{context}.server")
    if value["id"] is None:
        string(value["requested"], f"{context}.requested")
        if value["requested"].isdecimal():
            raise RenderInputError(f"{context}.requested must be nonnumeric")
    else:
        integer(value["id"], f"{context}.id", positive=True)
        expected = str(value["id"]) if root else None
        if value["requested"] != expected:
            requirement = "match its numeric id" if root else "be null for a numeric id"
            raise RenderInputError(f"{context}.requested must {requirement}")
    return (
        value["server"], value["id"],
        value["requested"] if value["id"] is None else None,
    )


def validate_policy(document):
    policy = document["policy"]
    exact_keys(
        policy,
        {"direction", "duration", "resolved_mode", "resolved_statuses", "stale_after_days"},
        "policy",
    )
    if policy["direction"] not in {"blocks", "both", "depends_on"}:
        raise RenderInputError("policy.direction is unsupported")
    if policy["duration"] is not None:
        raise RenderInputError("policy.duration must be null in version 1")
    if policy["resolved_mode"] not in {"include-no-traverse", "include-traverse"}:
        raise RenderInputError("policy.resolved_mode is unsupported")
    sorted_strings(policy["resolved_statuses"], "policy.resolved_statuses", nonempty=True)
    integer(policy["stale_after_days"], "policy.stale_after_days", positive=True)


def validate_provenance(document):
    provenance = document["provenance"]
    if not isinstance(provenance, list):
        raise RenderInputError("provenance must be an array")
    keys = []
    for index, entry in enumerate(provenance):
        context = f"provenance[{index}]"
        exact_keys(entry, {"parameter_names", "scope_kind", "server", "source"}, context)
        string(entry["server"], f"{context}.server")
        if entry["scope_kind"] not in SCOPE_KINDS:
            raise RenderInputError(f"{context}.scope_kind is unsupported")
        sorted_strings(
            entry["parameter_names"], f"{context}.parameter_names",
            allowed=PARAMETER_NAMES,
        )
        string(entry["source"], f"{context}.source", optional=True)
        if entry["scope_kind"] == "saved-query" and entry["source"] is None:
            raise RenderInputError(f"{context}.source is required")
        if entry["scope_kind"] not in {"restriction", "saved-query"}:
            if entry["source"] is not None:
                raise RenderInputError(f"{context}.source must be null")
        keys.append((
            entry["server"], SCOPE_RANK[entry["scope_kind"]],
            entry["source"] is None, entry["source"] or "", tuple(entry["parameter_names"]),
        ))
    if keys != sorted(set(keys)):
        raise RenderInputError("provenance must be sorted and unique")


def validate_node(node, index):
    context = f"nodes[{index}]"
    exact_keys(node, NODE_KEYS, context)
    string(node["server"], f"{context}.server")
    string(node["requested"], f"{context}.requested")
    if node["id"] is None:
        if node["requested"].isdecimal():
            raise RenderInputError(f"{context}.requested must be nonnumeric")
    else:
        integer(node["id"], f"{context}.id", positive=True)
        if node["requested"] != str(node["id"]):
            raise RenderInputError(f"{context}.requested must match its numeric id")
    integer(node["depth"], f"{context}.depth")
    sorted_strings(node["requested_aliases"], f"{context}.requested_aliases")
    exact_keys(node["provenance"], {"command", "server"}, f"{context}.provenance")
    if node["provenance"] != {"command": "bug view", "server": node["server"]}:
        raise RenderInputError(f"{context}.provenance is invalid")
    validate_node_state(node, context)
    return (
        node["server"], node["id"],
        node["requested"] if node["id"] is None else None,
    )


def validate_node_state(node, context):
    state = node["state"]
    if state == "known":
        if node["id"] is None or node["boundary_reason"] is not None:
            raise RenderInputError(f"{context} has invalid known-node fields")
        if node["error_type"] is not None:
            raise RenderInputError(f"{context} has invalid known-node fields")
        for field in ("assigned_to", "last_change_time", "resolution"):
            if node[field] is not None and not isinstance(node[field], str):
                raise RenderInputError(f"{context}.{field} must be null or a string")
        if not isinstance(node["status"], str) or not isinstance(node["summary"], str):
            raise RenderInputError(f"{context} has invalid fetched fields")
        if type(node["stale"]) is not bool and node["stale"] != "unknown":
            raise RenderInputError(f"{context}.stale is invalid")
        return
    if state not in {"boundary", "unknown"}:
        raise RenderInputError(f"{context}.state is unsupported")
    for field in ("assigned_to", "last_change_time", "resolution", "status", "summary"):
        if node[field] is not None:
            raise RenderInputError(f"{context}.{field} must be null")
    if node["stale"] != "unknown":
        raise RenderInputError(f"{context}.stale must be unknown")
    if state == "unknown":
        valid = node["boundary_reason"] is None
        valid = valid and node["error_type"] in {"inaccessible", "not_found"}
    else:
        reasons = {"depth_limit", "fetch_interrupted", "pending_fetch", "scope_restriction"}
        valid = node["error_type"] is None and node["boundary_reason"] in reasons
    if not valid:
        raise RenderInputError(f"{context} has invalid {state}-node fields")


def validate_components(document, node_identities):
    components = document["components"]
    if not isinstance(components, list):
        raise RenderInputError("components must be an array")
    seen = []
    component_ids = []
    for index, component in enumerate(components, start=1):
        context = f"components[{index - 1}]"
        exact_keys(component, {"cyclic", "id", "nodes"}, context)
        boolean(component["cyclic"], f"{context}.cyclic")
        if component["id"] != f"c{index:04d}":
            raise RenderInputError(f"{context}.id is not canonical")
        if not COMPONENT_PATTERN.fullmatch(component["id"]):
            raise RenderInputError(f"{context}.id is not canonical")
        if not isinstance(component["nodes"], list) or not component["nodes"]:
            raise RenderInputError(f"{context}.nodes must not be empty")
        members = [
            validate_identity(member, f"{context}.nodes[{member_index}]")
            for member_index, member in enumerate(component["nodes"])
        ]
        if members != sorted(set(members), key=identity_tuple_key):
            raise RenderInputError(f"{context}.nodes must be sorted and unique")
        seen.extend(members)
        component_ids.append(component["id"])
    if set(seen) != node_identities or len(seen) != len(node_identities):
        raise RenderInputError("components must partition nodes")
    return component_ids


def validate_edges(document, numeric_nodes):
    edges = document["edges"]
    if not isinstance(edges, list):
        raise RenderInputError("edges must be an array")
    keys = []
    for index, edge in enumerate(edges):
        context = f"edges[{index}]"
        exact_keys(edge, {"observations", "predecessor", "successor"}, context)
        sorted_strings(
            edge["observations"], f"{context}.observations",
            allowed={"blocks", "depends_on"}, nonempty=True,
        )
        endpoints = []
        for role in ("predecessor", "successor"):
            endpoint = edge[role]
            exact_keys(endpoint, {"id", "server"}, f"{context}.{role}")
            string(endpoint["server"], f"{context}.{role}.server")
            integer(endpoint["id"], f"{context}.{role}.id", positive=True)
            identity = (endpoint["server"], endpoint["id"])
            if identity not in numeric_nodes:
                raise RenderInputError(f"{context}.{role} is absent from nodes")
            endpoints.append(identity)
        keys.append((*endpoints[0], *endpoints[1]))
    if keys != sorted(set(keys)):
        raise RenderInputError("edges must be sorted and unique")


def validate_identity_list(values, context, node_identities, *, roots=False):
    if not isinstance(values, list):
        raise RenderInputError(f"{context} must be an array")
    identities = [
        validate_identity(value, f"{context}[{index}]", root=roots)
        for index, value in enumerate(values)
    ]
    if any(identity not in node_identities for identity in identities):
        raise RenderInputError(f"{context} contains an identity absent from nodes")
    if identities != sorted(set(identities), key=identity_tuple_key):
        raise RenderInputError(f"{context} must be sorted and unique")


def validate_component_list(values, context, component_ids):
    if not isinstance(values, list) or any(value not in component_ids for value in values):
        raise RenderInputError(f"{context} contains an unknown component")
    if len(values) != len(set(values)):
        raise RenderInputError(f"{context} must be unique")


def validate_findings(document, node_identities, component_ids):
    findings = document["findings"]
    expected = {
        "bottlenecks", "execution_order", "stale_blockers", "structural_leaves",
        "structural_roots", "unassigned_blockers",
    }
    exact_keys(findings, expected, "findings")
    names = (
        "stale_blockers", "structural_leaves", "structural_roots",
        "unassigned_blockers",
    )
    for name in names:
        validate_identity_list(findings[name], f"findings.{name}", node_identities)
    if not isinstance(findings["bottlenecks"], list):
        raise RenderInputError("findings.bottlenecks must be an array")
    for index, value in enumerate(findings["bottlenecks"]):
        context = f"findings.bottlenecks[{index}]"
        exact_keys(value, {"fan_out", "node"}, context)
        integer(value["fan_out"], f"{context}.fan_out", positive=True)
        identity = validate_identity(value["node"], f"{context}.node")
        if identity not in node_identities:
            raise RenderInputError(f"{context}.node is absent from nodes")
    bottleneck_keys = [
        (-value["fan_out"], identity_tuple_key((
            value["node"]["server"], value["node"]["id"],
            value["node"]["requested"] if value["node"]["id"] is None else None,
        )))
        for value in findings["bottlenecks"]
    ]
    if bottleneck_keys != sorted(bottleneck_keys):
        raise RenderInputError("findings.bottlenecks must be sorted")
    order = findings["execution_order"]
    order_keys = {
        "assumptions", "component_order", "cycle_impediments", "incomplete_boundaries",
    }
    exact_keys(order, order_keys, "findings.execution_order")
    sorted_strings(order["assumptions"], "findings.execution_order.assumptions")
    validate_component_list(
        order["component_order"], "findings.execution_order.component_order", component_ids,
    )
    validate_component_list(
        order["cycle_impediments"], "findings.execution_order.cycle_impediments", component_ids,
    )
    validate_identity_list(
        order["incomplete_boundaries"],
        "findings.execution_order.incomplete_boundaries",
        node_identities,
    )


def validate_layers_and_chain(document, component_ids):
    layers = document["layers"]
    if not isinstance(layers, list):
        raise RenderInputError("layers must be an array")
    flattened = []
    for index, layer in enumerate(layers):
        validate_component_list(layer, f"layers[{index}]", component_ids)
        if layer != sorted(layer) or not layer:
            raise RenderInputError(f"layers[{index}] must be non-empty and sorted")
        flattened.extend(layer)
    if len(flattened) != len(component_ids) or set(flattened) != set(component_ids):
        raise RenderInputError("layers must partition components")
    if flattened != document["findings"]["execution_order"]["component_order"]:
        raise RenderInputError("component order must equal flattened layers")
    chain = document["longest_chain"]
    exact_keys(chain, {"kind", "length", "path"}, "longest_chain")
    if chain["kind"] != "edge_count":
        raise RenderInputError("longest_chain.kind must be edge_count")
    integer(chain["length"], "longest_chain.length")
    validate_component_list(chain["path"], "longest_chain.path", component_ids)
    if chain["length"] != max(0, len(chain["path"]) - 1):
        raise RenderInputError("longest_chain.length does not match its path")


def validate_warnings(document, node_identities):
    if not isinstance(document["warnings"], list):
        raise RenderInputError("warnings must be an array")
    codes = []
    for index, warning in enumerate(document["warnings"]):
        context = f"warnings[{index}]"
        exact_keys(warning, {"code", "nodes"}, context)
        string(warning["code"], f"{context}.code")
        validate_identity_list(warning["nodes"], f"{context}.nodes", node_identities)
        codes.append(warning["code"])
    if codes != sorted(set(codes)):
        raise RenderInputError("warnings must be sorted and unique by code")


def validate_analysis(document):
    exact_keys(document, TOP_LEVEL_KEYS, "analysis")
    if document["schema"] != SCHEMA:
        raise RenderInputError(f"schema must be {SCHEMA}")
    timestamp = document["analysis_timestamp"]
    if not isinstance(timestamp, str) or not TIMESTAMP_PATTERN.fullmatch(timestamp):
        raise RenderInputError("analysis_timestamp must be second-precision UTC RFC 3339")
    try:
        datetime.strptime(timestamp, "%Y-%m-%dT%H:%M:%SZ")
    except ValueError as error:
        raise RenderInputError(
            "analysis_timestamp must be second-precision UTC RFC 3339"
        ) from error
    exact_keys(document["bounds"], {"max_depth", "max_nodes"}, "bounds")
    integer(document["bounds"]["max_depth"], "bounds.max_depth", positive=True)
    integer(document["bounds"]["max_nodes"], "bounds.max_nodes", positive=True)
    if document["bounds"]["max_nodes"] > MAX_NODES:
        raise RenderInputError(f"bounds.max_nodes must be at most {MAX_NODES}")
    cap_keys = {"graph_cap_reached", "omitted_discovered_identities", "scope_truncated"}
    exact_keys(document["cap"], cap_keys, "cap")
    boolean(document["cap"]["graph_cap_reached"], "cap.graph_cap_reached")
    boolean(document["cap"]["scope_truncated"], "cap.scope_truncated")
    integer(
        document["cap"]["omitted_discovered_identities"],
        "cap.omitted_discovered_identities",
    )
    if document["status"] not in {"complete", "partial"}:
        raise RenderInputError("status is unsupported")
    sorted_strings(document["limitations"], "limitations")
    validate_cap_relationships(document["cap"], document["limitations"])
    if (document["status"] == "partial") != bool(document["limitations"]):
        raise RenderInputError("status and limitations disagree")
    validate_policy(document)
    validate_provenance(document)
    if not isinstance(document["nodes"], list):
        raise RenderInputError("nodes must be an array")
    identities = [validate_node(node, index) for index, node in enumerate(document["nodes"])]
    if identities != sorted(set(identities), key=identity_tuple_key):
        raise RenderInputError("nodes must be sorted and unique")
    if len(identities) > document["bounds"]["max_nodes"]:
        raise RenderInputError("nodes exceed bounds.max_nodes")
    node_identities = set(identities)
    numeric_nodes = {
        (server, bug_id) for server, bug_id, _ in identities if bug_id is not None
    }
    component_ids = validate_components(document, node_identities)
    validate_edges(document, numeric_nodes)
    validate_findings(document, node_identities, component_ids)
    validate_layers_and_chain(document, component_ids)
    validate_identity_list(document["roots"], "roots", node_identities, roots=True)
    validate_warnings(document, node_identities)
    return document


MARKDOWN_ENTITIES = {
    "&": "&amp;", "<": "&lt;", ">": "&gt;", "[": "&#91;", "]": "&#93;",
    "!": "&#33;", "`": "&#96;", "\\": "&#92;", "*": "&#42;", "_": "&#95;",
    "{": "&#123;", "}": "&#125;", "(": "&#40;", ")": "&#41;", "#": "&#35;",
    "+": "&#43;", "-": "&#45;", ".": "&#46;", "|": "&#124;", "\n": "&#10;",
    "\r": "&#13;",
}


def markdown_text(value):
    escaped = []
    for character in str(value):
        if character in MARKDOWN_ENTITIES:
            escaped.append(MARKDOWN_ENTITIES[character])
        elif ord(character) < 32 or character in {"\u2028", "\u2029"}:
            escaped.append(f"&#{ord(character)};")
        else:
            escaped.append(character)
    return "".join(escaped)


def identity_text(value):
    requested = value["requested"] if value["id"] is None else str(value["id"])
    return f"{value['server']}#{requested}"


def collection_commands(document):
    commands = {node["provenance"]["command"] for node in document["nodes"]}
    for entry in document["provenance"]:
        kind = entry["scope_kind"]
        if kind == "saved-query" or (kind == "restriction" and entry["source"]):
            commands.add("query run")
        elif kind == "custom-search":
            commands.add("bug search")
        elif kind in {"milestone", "product", "version", "restriction"}:
            commands.add("bug list")
        elif kind == "alias":
            commands.add("bug view")
    return sorted(commands)


def metadata_lines(document):
    states = [node["state"] for node in document["nodes"]]
    commands = collection_commands(document)
    statuses = ", ".join(document["policy"]["resolved_statuses"])
    limitations = ", ".join(document["limitations"]) or "none"
    return [
        f"Schema: {document['schema']}",
        f"Status: {document['status']}",
        f"Analysis timestamp: {document['analysis_timestamp']}",
        (
            f"Bounds: maximum depth {document['bounds']['max_depth']}; "
            f"maximum nodes {document['bounds']['max_nodes']}"
        ),
        (
            f"Resolved-node policy: {document['policy']['resolved_mode']}; "
            f"resolved statuses {statuses}"
        ),
        "Duration assumptions: none; weighted critical-path analysis is unsupported",
        (
            f"Evidence gaps: {states.count('unknown')} unknown nodes; "
            f"{states.count('boundary')} boundary nodes"
        ),
        f"Graph cap reached: {str(document['cap']['graph_cap_reached']).lower()}",
        (
            "Omitted discovered identities: "
            f"{document['cap']['omitted_discovered_identities']}"
        ),
        f"Scope truncated: {str(document['cap']['scope_truncated']).lower()}",
        f"Limitations: {limitations}",
        f"Collection commands: {', '.join(commands) if commands else 'none'}",
    ]


def provenance_text(entry):
    source = entry["source"] if entry["source"] is not None else "none"
    names = ", ".join(entry["parameter_names"]) if entry["parameter_names"] else "none"
    return (
        f"server {entry['server']}; scope {entry['scope_kind']}; "
        f"saved query {source}; parameter names {names}"
    )


def reference_list(values):
    return ", ".join(identity_text(value) for value in values) if values else "none"


def bottleneck_list(values):
    if not values:
        return "none"
    return ", ".join(
        f"{identity_text(value['node'])} (fan-out {value['fan_out']})"
        for value in values
    )


def component_list(values):
    return ", ".join(values) if values else "none"


def warning_list(values):
    if not values:
        return "none"
    return "; ".join(
        f"{warning['code']} ({reference_list(warning['nodes'])})"
        for warning in values
    )


def finding_rows(document):
    findings = document["findings"]
    order = findings["execution_order"]
    return [
        (
            "Longest dependency chain by edge count",
            str(document["longest_chain"]["length"]),
        ),
        (
            "Longest dependency chain components",
            component_list(document["longest_chain"]["path"]),
        ),
        ("Structural roots", reference_list(findings["structural_roots"])),
        ("Structural leaves", reference_list(findings["structural_leaves"])),
        ("Bottlenecks", bottleneck_list(findings["bottlenecks"])),
        ("Unassigned blockers", reference_list(findings["unassigned_blockers"])),
        ("Stale blockers", reference_list(findings["stale_blockers"])),
        ("Execution assumptions", component_list(order["assumptions"])),
        ("Execution component order", component_list(order["component_order"])),
        ("Cycle impediments", component_list(order["cycle_impediments"])),
        ("Incomplete boundaries", reference_list(order["incomplete_boundaries"])),
        ("Analysis warnings", warning_list(document["warnings"])),
    ]


def markdown_inventory(document):
    records = []
    for node in document["nodes"]:
        records.append(json.dumps({
            "boundary_reason": node["boundary_reason"],
            "error_type": node["error_type"],
            "identity": identity_text(node),
            "stale": node["stale"],
            "state": node["state"],
            "status": node["status"],
            "summary": node["summary"],
        }, ensure_ascii=False, sort_keys=True, separators=(",", ":")))
    body = "\n".join(records) if records else "(no nodes)"
    longest = max((len(run) for run in re.findall(r"`+", body)), default=0)
    fence = "`" * max(3, longest + 1)
    return f"{fence}text\n{body}\n{fence}"


def render_markdown(document):
    lines = ["# Bugzilla dependency analysis", ""]
    lines.extend(f"- {markdown_text(line)}" for line in metadata_lines(document))
    lines.extend(["", "## Scope provenance", ""])
    if document["provenance"]:
        lines.extend(
            f"- {markdown_text(provenance_text(entry))}"
            for entry in document["provenance"]
        )
    else:
        lines.append("- none")
    lines.extend(["", "## Structural findings", ""])
    lines.extend(
        f"- {label}: {markdown_text(value)}"
        for label, value in finding_rows(document)
    )
    lines.extend(["", "## Structural execution groups", ""])
    if document["layers"]:
        for index, layer in enumerate(document["layers"], start=1):
            lines.append(f"- Layer {index}: {markdown_text(', '.join(layer))}")
    else:
        lines.append("- none")
    lines.extend(["", "## Node inventory", "", markdown_inventory(document)])
    lines.extend(["", "## Limitations", ""])
    lines.extend(
        f"- {markdown_text(value)}" for value in (document["limitations"] or ["none"])
    )
    return "\n".join(lines) + "\n"


MERMAID_ENTITIES = {
    "&": "&amp;", "<": "&lt;", ">": "&gt;", "%": "&#37;", "[": "&#91;",
    "]": "&#93;", "{": "&#123;", "}": "&#125;", "(": "&#40;", ")": "&#41;",
    "`": "&#96;", "#": "&#35;", '"': "#quot;", "\\": "#92;",
    "\n": "#10;", "\r": "#13;", "\t": "#9;",
}


def mermaid_text(value):
    escaped = []
    for character in str(value):
        if ord(character) < 32 or character in {"\u2028", "\u2029"}:
            escaped.append(MERMAID_ENTITIES.get(character, f"#{ord(character)};"))
        else:
            escaped.append(MERMAID_ENTITIES.get(character, character))
    return "".join(escaped)


def mermaid_id(value):
    server = value["server"].encode("utf-8").hex()
    if value["id"] is not None:
        return f"n_{server}_{value['id']}"
    requested = value["requested"].encode("utf-8").hex()
    return f"u_{server}_{requested}"


def render_mermaid(document):
    membership = {
        (member["server"], member["id"], member["requested"]): component["id"]
        for component in document["components"]
        for member in component["nodes"]
    }
    cyclic = {
        component["id"]: component["cyclic"] for component in document["components"]
    }
    provenance = " | ".join(
        provenance_text(entry) for entry in document["provenance"]
    ) or "none"
    layers = " | ".join(",".join(layer) for layer in document["layers"]) or "none"
    label = "\n".join([
        *metadata_lines(document),
        f"Scope provenance: {provenance}",
        *(f"{name}: {value}" for name, value in finding_rows(document)),
        f"Structural execution layers: {layers}",
    ])
    lines = ["flowchart TD", f'    metadata["{mermaid_text(label)}"]']
    for node in document["nodes"]:
        identity = (
            node["server"], node["id"],
            node["requested"] if node["id"] is None else None,
        )
        component = membership[identity]
        detail = node["summary"]
        if detail is None:
            detail = node["error_type"] or node["boundary_reason"] or node["state"]
        node_label = (
            f"{identity_text(node)}: {detail} | state={node['state']} | "
            f"component={component} | cyclic={str(cyclic[component]).lower()}"
        )
        lines.append(f'    {mermaid_id(node)}["{mermaid_text(node_label)}"]')
    lookup = {
        (node["server"], node["id"]): node
        for node in document["nodes"] if node["id"] is not None
    }
    for edge in document["edges"]:
        before = edge["predecessor"]
        after = edge["successor"]
        predecessor = lookup[(before["server"], before["id"])]
        successor = lookup[(after["server"], after["id"])]
        observations = ", ".join(edge["observations"])
        lines.append(
            f"    {mermaid_id(predecessor)} -->|{observations}| {mermaid_id(successor)}"
        )
    return "\n".join(lines) + "\n"


def render(document, output_format):
    validate_analysis(document)
    if output_format == "markdown":
        return render_markdown(document)
    if output_format == "mermaid":
        return render_mermaid(document)
    raise RenderInputError("format is unsupported")


def atomic_write(path, content):
    output = Path(path)
    output.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary = tempfile.mkstemp(
        prefix=f".{output.name}.", dir=str(output.parent)
    )
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            handle.write(content)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, output)
    except BaseException:
        try:
            os.unlink(temporary)
        except FileNotFoundError:
            pass
        raise


def unique_object(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise RenderInputError(f"duplicate JSON key: {key}")
        value[key] = item
    return value


def load_analysis(path):
    try:
        with open(path, encoding="utf-8") as handle:
            return json.load(handle, object_pairs_hook=unique_object)
    except RenderInputError:
        raise
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise RenderInputError("input must be readable valid JSON") from error


def parse_arguments(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--input", required=True)
    parser.add_argument("--format", choices=("markdown", "mermaid"), required=True)
    parser.add_argument("--output", required=True)
    return parser.parse_args(argv)


def main(argv=None):
    args = parse_arguments(argv)
    try:
        content = render(load_analysis(args.input), args.format)
    except RenderInputError as error:
        sys.stderr.write(f"render input error: {error}\n")
        return 2
    try:
        atomic_write(args.output, content)
    except OSError:
        sys.stderr.write("render output error: unable to write output\n")
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
