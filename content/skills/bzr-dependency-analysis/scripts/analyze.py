#!/usr/bin/env python3
"""Analyze a deterministic Bugzilla dependency collection."""

import argparse
from datetime import datetime, timezone
import heapq
import json
import os
from pathlib import Path
import re
import sys
import tempfile


COLLECTION_SCHEMA = "bzr-dependency-collection/v1"
ANALYSIS_SCHEMA = "bzr-dependency-analysis/v1"
MAX_COMPONENTS = 9_999
TIMESTAMP_PATTERN = re.compile(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z\Z")
TOP_LEVEL_KEYS = {
    "analysis_timestamp",
    "bounds",
    "cap",
    "limitations",
    "nodes",
    "observations",
    "policy",
    "provenance",
    "roots",
    "schema",
    "status",
}
NODE_KEYS = {
    "assigned_to",
    "boundary_reason",
    "depth",
    "error_type",
    "id",
    "last_change_time",
    "provenance",
    "requested",
    "requested_aliases",
    "resolution",
    "server",
    "state",
    "status",
    "summary",
}
BOUNDARY_REASONS = {
    "pending_fetch",
    "depth_limit",
    "scope_restriction",
    "fetch_interrupted",
}
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
PROVENANCE_PARAMETER_NAMES = {
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
COPIED_KEYS = (
    "analysis_timestamp",
    "bounds",
    "cap",
    "limitations",
    "policy",
    "provenance",
    "roots",
    "status",
)


class AnalysisInputError(ValueError):
    """The collection does not match the versioned analyzer contract."""


def exact_keys(value, expected, context):
    if not isinstance(value, dict):
        raise AnalysisInputError(f"{context} must be an object")
    if set(value) != set(expected):
        raise AnalysisInputError(f"{context} has invalid keys")


def nonempty_string(value, context):
    if not isinstance(value, str) or not value:
        raise AnalysisInputError(f"{context} must be a non-empty string")
    return value


def positive_integer(value, context):
    if type(value) is not int or value <= 0:
        raise AnalysisInputError(f"{context} must be a positive integer")
    return value


def nonnegative_integer(value, context):
    if type(value) is not int or value < 0:
        raise AnalysisInputError(f"{context} must be a non-negative integer")
    return value


def optional_string(value, context):
    if value is not None and not isinstance(value, str):
        raise AnalysisInputError(f"{context} must be null or a string")


def sorted_unique_strings(value, context, *, allow_empty=True):
    if not isinstance(value, list) or any(
        not isinstance(item, str) or not item for item in value
    ):
        raise AnalysisInputError(f"{context} must be an array of non-empty strings")
    if not allow_empty and not value:
        raise AnalysisInputError(f"{context} must not be empty")
    if value != sorted(set(value)):
        raise AnalysisInputError(f"{context} must be sorted and unique")


def parse_analysis_timestamp(value):
    if not isinstance(value, str) or not TIMESTAMP_PATTERN.fullmatch(value):
        raise AnalysisInputError("analysis_timestamp must be second-precision UTC RFC 3339")
    try:
        return datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=timezone.utc)
    except ValueError as error:
        raise AnalysisInputError(
            "analysis_timestamp must be second-precision UTC RFC 3339"
        ) from error


def validate_policy(policy):
    exact_keys(
        policy,
        {"direction", "duration", "resolved_mode", "resolved_statuses", "stale_after_days"},
        "policy",
    )
    if policy["direction"] not in {"depends_on", "blocks", "both"}:
        raise AnalysisInputError("policy.direction is unsupported")
    if policy["duration"] is not None:
        raise AnalysisInputError("policy.duration must be null in version 1")
    if policy["resolved_mode"] not in {"include-traverse", "include-no-traverse"}:
        raise AnalysisInputError("policy.resolved_mode is unsupported")
    sorted_unique_strings(
        policy["resolved_statuses"], "policy.resolved_statuses", allow_empty=False
    )
    positive_integer(policy["stale_after_days"], "policy.stale_after_days")


def identity_key(node):
    return (
        node["server"],
        node["id"] is None,
        -1 if node["id"] is None else node["id"],
        node["requested"] if node["id"] is None else "",
    )


def node_identity(node):
    return (
        node["server"],
        node["id"],
        node["requested"] if node["id"] is None else None,
    )


def identity_reference(node):
    return {
        "id": node["id"],
        "requested": node["requested"] if node["id"] is None else None,
        "server": node["server"],
    }


def validate_node(node, index):
    context = f"nodes[{index}]"
    exact_keys(node, NODE_KEYS, context)
    server = nonempty_string(node["server"], f"{context}.server")
    requested = nonempty_string(node["requested"], f"{context}.requested")
    if node["id"] is not None:
        positive_integer(node["id"], f"{context}.id")
        if requested != str(node["id"]):
            raise AnalysisInputError(f"{context}.requested must match its numeric id")
    elif requested.isdecimal():
        raise AnalysisInputError(f"{context}.id may be null only for a nonnumeric request")
    nonnegative_integer(node["depth"], f"{context}.depth")
    sorted_unique_strings(node["requested_aliases"], f"{context}.requested_aliases")
    exact_keys(node["provenance"], {"command", "server"}, f"{context}.provenance")
    if node["provenance"] != {"command": "bug view", "server": server}:
        raise AnalysisInputError(f"{context}.provenance is invalid")
    state = node["state"]
    if state == "known":
        validate_known_node(node, context)
    elif state == "unknown":
        validate_incomplete_node(node, context, unknown=True)
    elif state == "boundary":
        validate_incomplete_node(node, context, unknown=False)
    else:
        raise AnalysisInputError(f"{context}.state is unsupported")


def validate_known_node(node, context):
    if node["id"] is None:
        raise AnalysisInputError(f"{context}.id is required for a known node")
    if node["boundary_reason"] is not None or node["error_type"] is not None:
        raise AnalysisInputError(f"{context} has invalid known-node error fields")
    for key in ("assigned_to", "resolution", "last_change_time"):
        optional_string(node[key], f"{context}.{key}")
    if not isinstance(node["summary"], str) or not isinstance(node["status"], str):
        raise AnalysisInputError(f"{context} has invalid known-node fetched fields")


def validate_incomplete_node(node, context, *, unknown):
    for key in ("assigned_to", "last_change_time", "resolution", "status", "summary"):
        if node[key] is not None:
            raise AnalysisInputError(f"{context}.{key} must be null")
    if unknown:
        if node["boundary_reason"] is not None:
            raise AnalysisInputError(f"{context}.boundary_reason must be null")
        if node["error_type"] not in {"not_found", "inaccessible"}:
            raise AnalysisInputError(f"{context}.error_type is unsupported")
    else:
        if node["error_type"] is not None:
            raise AnalysisInputError(f"{context}.error_type must be null")
        if node["boundary_reason"] not in BOUNDARY_REASONS:
            raise AnalysisInputError(f"{context}.boundary_reason is unsupported")


def collection_node_key(node):
    return (
        node["server"],
        node["depth"],
        node["id"] is None,
        -1 if node["id"] is None else node["id"],
        node["requested"],
    )


def validate_endpoint(value, context):
    exact_keys(value, {"id", "server"}, context)
    return (
        nonempty_string(value["server"], f"{context}.server"),
        positive_integer(value["id"], f"{context}.id"),
    )


def validate_observations(observations, numeric_nodes):
    if not isinstance(observations, list):
        raise AnalysisInputError("observations must be an array")
    keys = []
    for index, observation in enumerate(observations):
        context = f"observations[{index}]"
        exact_keys(observation, {"field", "source", "target"}, context)
        if observation["field"] not in {"blocks", "depends_on"}:
            raise AnalysisInputError(f"{context}.field is unsupported")
        source = validate_endpoint(observation["source"], f"{context}.source")
        target = validate_endpoint(observation["target"], f"{context}.target")
        if source not in numeric_nodes or target not in numeric_nodes:
            raise AnalysisInputError(f"{context} has an endpoint absent from nodes")
        keys.append((*source, *target, observation["field"]))
    if keys != sorted(set(keys)):
        raise AnalysisInputError("observations must be sorted and unique")


def validate_roots(roots, node_lookup):
    if not isinstance(roots, list):
        raise AnalysisInputError("roots must be an array")
    keys = []
    for index, root in enumerate(roots):
        context = f"roots[{index}]"
        exact_keys(root, {"id", "requested", "server"}, context)
        server = nonempty_string(root["server"], f"{context}.server")
        requested = nonempty_string(root["requested"], f"{context}.requested")
        if root["id"] is None:
            if requested.isdecimal():
                raise AnalysisInputError(f"{context}.requested must be nonnumeric")
            identity = (server, None, requested)
        else:
            positive_integer(root["id"], f"{context}.id")
            if requested != str(root["id"]):
                raise AnalysisInputError(f"{context}.requested must match its numeric id")
            identity = (server, root["id"], None)
        if identity not in node_lookup:
            raise AnalysisInputError(f"{context} does not link to a node")
        keys.append((
            server,
            root["id"] is None,
            -1 if root["id"] is None else root["id"],
            requested,
        ))
    if keys != sorted(set(keys)):
        raise AnalysisInputError("roots must be sorted and unique")


def provenance_key(entry):
    source = entry["source"]
    return (
        entry["server"],
        SCOPE_RANK[entry["scope_kind"]],
        source is None,
        "" if source is None else source,
        tuple(entry["parameter_names"]),
    )


def validate_provenance(provenance):
    if not isinstance(provenance, list):
        raise AnalysisInputError("provenance must be an array")
    for index, entry in enumerate(provenance):
        context = f"provenance[{index}]"
        exact_keys(entry, {"parameter_names", "scope_kind", "source", "server"}, context)
        nonempty_string(entry["server"], f"{context}.server")
        if entry["scope_kind"] not in SCOPE_RANK:
            raise AnalysisInputError(f"{context}.scope_kind is unsupported")
        sorted_unique_strings(entry["parameter_names"], f"{context}.parameter_names")
        if not set(entry["parameter_names"]).issubset(PROVENANCE_PARAMETER_NAMES):
            raise AnalysisInputError(f"{context}.parameter_names contains an unsupported name")
        optional_string(entry["source"], f"{context}.source")
        if entry["source"] is not None:
            nonempty_string(entry["source"], f"{context}.source")
        if entry["scope_kind"] == "saved-query" and not entry["source"]:
            raise AnalysisInputError(f"{context}.source is required")
        if (
            entry["scope_kind"] not in {"saved-query", "restriction"}
            and entry["source"] is not None
        ):
            raise AnalysisInputError(f"{context}.source must be null")
    keys = [provenance_key(entry) for entry in provenance]
    if keys != sorted(set(keys)):
        raise AnalysisInputError("provenance must be sorted and unique")


def validate_collection(document, allow_partial):
    exact_keys(document, TOP_LEVEL_KEYS, "collection")
    if document["schema"] != COLLECTION_SCHEMA:
        raise AnalysisInputError(f"schema must be {COLLECTION_SCHEMA}")
    parse_analysis_timestamp(document["analysis_timestamp"])
    exact_keys(document["bounds"], {"max_depth", "max_nodes"}, "bounds")
    positive_integer(document["bounds"]["max_depth"], "bounds.max_depth")
    max_nodes = positive_integer(document["bounds"]["max_nodes"], "bounds.max_nodes")
    if max_nodes > MAX_COMPONENTS:
        raise AnalysisInputError(
            f"bounds.max_nodes must be at most {MAX_COMPONENTS}"
        )
    validate_cap(document["cap"])
    sorted_unique_strings(document["limitations"], "limitations")
    validate_cap_relationships(document["cap"], document["limitations"])
    validate_policy(document["policy"])
    validate_provenance(document["provenance"])
    if document["status"] not in {"complete", "partial"}:
        raise AnalysisInputError("status is unsupported")
    if (document["status"] == "partial") != bool(document["limitations"]):
        raise AnalysisInputError("status must agree with limitations")
    if document["status"] == "partial" and not allow_partial:
        raise AnalysisInputError("partial input requires --allow-partial")
    nodes, node_lookup, numeric_nodes = validate_nodes(document["nodes"], max_nodes)
    validate_observations(document["observations"], numeric_nodes)
    validate_roots(document["roots"], node_lookup)
    return nodes


def validate_cap(cap):
    exact_keys(
        cap,
        {"graph_cap_reached", "omitted_discovered_identities", "scope_truncated"},
        "cap",
    )
    if (
        type(cap["graph_cap_reached"]) is not bool
        or type(cap["scope_truncated"]) is not bool
    ):
        raise AnalysisInputError("cap flags must be booleans")
    nonnegative_integer(
        cap["omitted_discovered_identities"], "cap.omitted_discovered_identities"
    )


def validate_cap_relationships(cap, limitations):
    graph_limited = "graph-node-cap" in limitations
    graph_cap_reached = cap["graph_cap_reached"]
    has_omissions = cap["omitted_discovered_identities"] > 0
    if graph_limited != graph_cap_reached or (has_omissions and not graph_cap_reached):
        raise AnalysisInputError("graph cap metadata is inconsistent")

    scope_limitations = set(limitations) & {
        "restriction-node-cap",
        "scope-node-cap",
    }
    if len(scope_limitations) > 1 or cap["scope_truncated"] != bool(scope_limitations):
        raise AnalysisInputError("scope cap metadata is inconsistent")


def validate_nodes(nodes, max_nodes):
    if not isinstance(nodes, list):
        raise AnalysisInputError("nodes must be an array")
    if len(nodes) > max_nodes:
        raise AnalysisInputError("nodes exceeds bounds.max_nodes")
    for index, node in enumerate(nodes):
        validate_node(node, index)
    if [collection_node_key(node) for node in nodes] != sorted(
        collection_node_key(node) for node in nodes
    ):
        raise AnalysisInputError("nodes must use canonical collection order")
    lookup = {node_identity(node): node for node in nodes}
    if len(lookup) != len(nodes):
        raise AnalysisInputError("nodes contains duplicate identities")
    numeric = {(node["server"], node["id"]) for node in nodes if node["id"] is not None}
    return nodes, lookup, numeric


def canonical_edges(observations):
    grouped = {}
    for observation in observations:
        source = (observation["source"]["server"], observation["source"]["id"])
        target = (observation["target"]["server"], observation["target"]["id"])
        predecessor, successor = (
            (target, source) if observation["field"] == "depends_on" else (source, target)
        )
        grouped.setdefault((predecessor, successor), set()).add(observation["field"])
    result = []
    for (predecessor, successor), fields in sorted(grouped.items()):
        result.append({
            "observations": sorted(fields),
            "predecessor": {"id": predecessor[1], "server": predecessor[0]},
            "successor": {"id": successor[1], "server": successor[0]},
        })
    return result


def graph_adjacency(nodes, edges):
    numeric_lookup = {
        (node["server"], node["id"]): node_identity(node)
        for node in nodes
        if node["id"] is not None
    }
    adjacency = {node_identity(node): set() for node in nodes}
    for edge_value in edges:
        predecessor = edge_value["predecessor"]
        successor = edge_value["successor"]
        source_key = numeric_lookup[(predecessor["server"], predecessor["id"])]
        target_key = numeric_lookup[(successor["server"], successor["id"])]
        adjacency[source_key].add(target_key)
    return adjacency


def strongly_connected(node_keys, adjacency, sort_keys):
    indices = {}
    lowlinks = {}
    stack = []
    on_stack = set()
    components = []
    next_index = 0
    for start in node_keys:
        if start in indices:
            continue
        frames = []
        indices[start] = lowlinks[start] = next_index
        next_index += 1
        stack.append(start)
        on_stack.add(start)
        frames.append([start, sorted(adjacency[start], key=sort_keys.__getitem__), 0, None])
        while frames:
            node, successors, position, parent = frames[-1]
            if position < len(successors):
                successor = successors[position]
                frames[-1][2] += 1
                if successor not in indices:
                    indices[successor] = lowlinks[successor] = next_index
                    next_index += 1
                    stack.append(successor)
                    on_stack.add(successor)
                    frames.append([
                        successor,
                        sorted(adjacency[successor], key=sort_keys.__getitem__),
                        0,
                        node,
                    ])
                elif successor in on_stack:
                    lowlinks[node] = min(lowlinks[node], indices[successor])
                continue
            frames.pop()
            if parent is not None:
                lowlinks[parent] = min(lowlinks[parent], lowlinks[node])
            if lowlinks[node] == indices[node]:
                members = []
                while True:
                    member = stack.pop()
                    on_stack.remove(member)
                    members.append(member)
                    if member == node:
                        break
                components.append(sorted(members, key=sort_keys.__getitem__))
    components.sort(key=lambda members: sort_keys[members[0]])
    return components


def build_components(nodes, adjacency):
    lookup = {node_identity(node): node for node in nodes}
    sort_keys = {key: identity_key(node) for key, node in lookup.items()}
    raw_components = strongly_connected(
        sorted(lookup, key=sort_keys.__getitem__), adjacency, sort_keys
    )
    if len(raw_components) > MAX_COMPONENTS:
        raise AnalysisInputError("component count exceeds four-digit component namespace")
    self_loops = {key for key, successors in adjacency.items() if key in successors}
    components = []
    membership = {}
    for index, members in enumerate(raw_components, start=1):
        component_id = f"c{index:04d}"
        for member in members:
            membership[member] = component_id
        components.append({
            "cyclic": len(members) > 1 or members[0] in self_loops,
            "id": component_id,
            "nodes": [identity_reference(lookup[member]) for member in members],
        })
    return components, membership


def condensation_graph(components, membership, adjacency):
    graph = {component["id"]: set() for component in components}
    for source, successors in adjacency.items():
        source_component = membership[source]
        for successor in successors:
            target_component = membership[successor]
            if source_component != target_component:
                graph[source_component].add(target_component)
    return graph


def topological_layers(graph):
    indegree = {component: 0 for component in graph}
    for successors in graph.values():
        for successor in successors:
            indegree[successor] += 1
    layers = []
    ready = [component for component, degree in indegree.items() if degree == 0]
    heapq.heapify(ready)
    processed = 0
    while ready:
        layer = []
        following = []
        while ready:
            component = heapq.heappop(ready)
            layer.append(component)
            for successor in graph[component]:
                indegree[successor] -= 1
                if indegree[successor] == 0:
                    heapq.heappush(following, successor)
        layers.append(layer)
        processed += len(layer)
        ready = following
    if processed != len(graph):
        raise AnalysisInputError("component condensation graph is cyclic")
    return layers


def longest_chain(graph, component_order):
    if not component_order:
        return {"kind": "edge_count", "length": 0, "path": []}
    lengths = {component: 0 for component in component_order}
    incoming = {component: [] for component in component_order}
    for component in component_order:
        for successor in graph[component]:
            incoming[successor].append(component)
            lengths[successor] = max(lengths[successor], lengths[component] + 1)

    by_length = {}
    for component, length in lengths.items():
        by_length.setdefault(length, []).append(component)
    predecessors = {}
    ranks = {}
    for length in range(max(by_length) + 1):
        components = by_length.get(length, [])
        if length == 0:
            ordered = sorted(components)
        else:
            for component in components:
                candidates = (
                    predecessor
                    for predecessor in incoming[component]
                    if lengths[predecessor] == length - 1
                )
                predecessors[component] = min(candidates, key=ranks.__getitem__)
            ordered = sorted(
                components,
                key=lambda component: (ranks[predecessors[component]], component),
            )
        for rank, component in enumerate(ordered):
            ranks[component] = rank

    length = max(lengths.values())
    component = min(by_length[length], key=ranks.__getitem__)
    path = [component]
    while component in predecessors:
        component = predecessors[component]
        path.append(component)
    path.reverse()
    return {"kind": "edge_count", "length": length, "path": path}


def parse_node_timestamp(value):
    if not isinstance(value, str):
        return None
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return None
    if parsed.tzinfo is None:
        return None
    return parsed.astimezone(timezone.utc)


def apply_staleness(nodes, policy, analysis_time):
    resolved_statuses = set(policy["resolved_statuses"])
    threshold_days = policy["stale_after_days"]
    warnings = {}
    output = []
    for node in sorted(nodes, key=identity_key):
        copied = dict(node)
        if node["state"] != "known":
            copied["stale"] = "unknown"
        elif node["status"] in resolved_statuses:
            copied["stale"] = False
        else:
            changed = parse_node_timestamp(node["last_change_time"])
            if changed is None:
                copied["stale"] = "unknown"
                warnings.setdefault("stale-timestamp-unknown", []).append(
                    identity_reference(node)
                )
            elif changed > analysis_time:
                copied["stale"] = "unknown"
                warnings.setdefault("stale-timestamp-future", []).append(
                    identity_reference(node)
                )
            else:
                age_days = (analysis_time - changed).days
                copied["stale"] = age_days >= threshold_days
        output.append(copied)
    warning_list = [
        {"code": code, "nodes": references}
        for code, references in sorted(warnings.items())
    ]
    return output, warning_list


def pm_findings(nodes, edges, components, layers, policy, status):
    lookup = {node_identity(node): node for node in nodes}
    incoming = {identity: 0 for identity in lookup}
    outgoing = {identity: 0 for identity in lookup}
    numeric = {
        (node["server"], node["id"]): node_identity(node)
        for node in nodes
        if node["id"] is not None
    }
    for edge_value in edges:
        predecessor = edge_value["predecessor"]
        successor = edge_value["successor"]
        source = numeric[(predecessor["server"], predecessor["id"])]
        target = numeric[(successor["server"], successor["id"])]
        outgoing[source] += 1
        incoming[target] += 1
    ordered = sorted(lookup, key=lambda identity: identity_key(lookup[identity]))
    component_order = [component for layer in layers for component in layer]
    component_cycles = {
        component["id"]: component["cyclic"] for component in components
    }
    cycles = [
        component_id
        for component_id in component_order
        if component_cycles[component_id]
    ]
    assumptions = [f"resolved-{policy['resolved_mode']}"]
    if status == "partial":
        assumptions.append("partial-evidence")
    if cycles:
        assumptions.append("cycles-prevent-total-node-order")
    resolved = set(policy["resolved_statuses"])
    bottleneck_keys = sorted(
        (key for key in ordered if outgoing[key] > 1),
        key=lambda key: (-outgoing[key], identity_key(lookup[key])),
    )
    return {
        "bottlenecks": [
            {"fan_out": outgoing[key], "node": identity_reference(lookup[key])}
            for key in bottleneck_keys
        ],
        "execution_order": {
            "assumptions": sorted(assumptions),
            "component_order": component_order,
            "cycle_impediments": cycles,
            "incomplete_boundaries": [
                identity_reference(lookup[key])
                for key in ordered
                if lookup[key]["state"] in {"unknown", "boundary"}
            ],
        },
        "structural_leaves": [
            identity_reference(lookup[key]) for key in ordered if outgoing[key] == 0
        ],
        "structural_roots": [
            identity_reference(lookup[key]) for key in ordered if incoming[key] == 0
        ],
        "stale_blockers": [
            identity_reference(lookup[key])
            for key in ordered
            if lookup[key]["stale"] is True and outgoing[key] > 0
        ],
        "unassigned_blockers": [
            identity_reference(lookup[key])
            for key in ordered
            if lookup[key]["state"] == "known"
            and lookup[key]["status"] not in resolved
            and lookup[key]["assigned_to"] is None
            and outgoing[key] > 0
        ],
    }


def analyze(document, allow_partial=False):
    source_nodes = validate_collection(document, allow_partial)
    edges = canonical_edges(document["observations"])
    adjacency = graph_adjacency(source_nodes, edges)
    components, membership = build_components(source_nodes, adjacency)
    condensed = condensation_graph(components, membership, adjacency)
    layers = topological_layers(condensed)
    component_order = [component for layer in layers for component in layer]
    nodes, warnings = apply_staleness(
        source_nodes,
        document["policy"],
        parse_analysis_timestamp(document["analysis_timestamp"]),
    )
    result = {key: document[key] for key in COPIED_KEYS}
    result.update({
        "components": components,
        "edges": edges,
        "findings": pm_findings(
            nodes, edges, components, layers, document["policy"], document["status"]
        ),
        "layers": layers,
        "longest_chain": longest_chain(condensed, component_order),
        "nodes": nodes,
        "schema": ANALYSIS_SCHEMA,
        "warnings": warnings,
    })
    return result


def atomic_write(path, document):
    output = Path(path)
    output.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary = tempfile.mkstemp(
        prefix=f".{output.name}.", dir=str(output.parent)
    )
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


def unique_object(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise AnalysisInputError(f"duplicate JSON key: {key}")
        value[key] = item
    return value


def load_collection(path):
    try:
        with open(path, encoding="utf-8") as handle:
            value = json.load(handle, object_pairs_hook=unique_object)
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise AnalysisInputError("input must be readable valid JSON") from error
    return value


def parse_arguments(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--allow-partial", action="store_true")
    return parser.parse_args(argv)


def main(argv=None):
    args = parse_arguments(argv)
    try:
        document = analyze(load_collection(args.input), args.allow_partial)
    except AnalysisInputError as error:
        sys.stderr.write(f"analysis input error: {error}\n")
        return 2
    try:
        atomic_write(args.output, document)
    except OSError:
        sys.stderr.write("analysis output error: unable to write output\n")
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
