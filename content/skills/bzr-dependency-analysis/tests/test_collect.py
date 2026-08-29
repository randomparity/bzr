"""Behavioral contract for the dependency-evidence collector."""

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SKILL_ROOT = Path(__file__).resolve().parents[1]
COLLECT = SKILL_ROOT / "scripts" / "collect.py"
ANALYZE = SKILL_ROOT / "scripts" / "analyze.py"
FIXTURES = Path(__file__).resolve().parent / "fixtures"
RUNNER = FIXTURES / "recording_runner.py"
TIMESTAMP = "2026-08-28T12:00:00Z"
SCHEMA_VERSION = "0.6.1"
DETAIL_FIELDS = (
    "id,summary,status,resolution,assigned_to,last_change_time,blocks,depends_on"
)


def policy(scopes, *, max_nodes=10, max_depth=3, direction="both", restriction=None):
    servers = sorted({scope["server"] for scope in scopes} | (
        {restriction["server"]} if restriction else set()
    ))
    return {
        "bounds": {"max_depth": max_depth, "max_nodes": max_nodes},
        "bzr": "bzr",
        "direction": direction,
        "resolved_mode": "include-no-traverse",
        "resolved_statuses": ["RESOLVED"],
        "restriction": restriction,
        "scopes": scopes,
        "servers": servers,
        "stale_after_days": 14,
    }


def bug_scope(server, *ids):
    return {"kind": "bug-ids", "server": server, "ids": list(ids)}


def alias_scope(server, alias):
    return {"kind": "alias", "server": server, "alias": alias}


def view_argv(server, requested, extra_field=None):
    fields = DETAIL_FIELDS + (f",{extra_field}" if extra_field else "")
    return [
        "--server", server, "--json", "bug", "view", str(requested),
        "--fields", fields,
    ]


def list_argv(server, limit, offset, *scope_args):
    return [
        "--server", server, "--json", *scope_args, "--limit", str(limit),
        "--offset", str(offset), "--fields", "id", "--sort", "bug_id",
        "--order", "asc",
    ]


def ok(argv, data):
    return {
        "argv": argv,
        "exit_code": 0,
        "stdout": {"schema_version": SCHEMA_VERSION, "data": data},
    }


def failed(argv, error_type, *, api_code=None, raw="private server detail"):
    error = {"type": error_type, "message": raw, "exit_code": 4 if error_type == "api" else 5}
    if api_code is not None:
        error["api_code"] = api_code
    return {
        "argv": argv,
        "exit_code": error["exit_code"],
        "stderr": {"schema_version": SCHEMA_VERSION, "error": error},
    }


def bug(bug_id, *, depends=(), blocks=(), status="NEW", summary=None, extra=None):
    value = {
        "assigned_to": None,
        "blocks": list(blocks),
        "depends_on": list(depends),
        "id": bug_id,
        "last_change_time": "2026-08-27T12:00:00Z",
        "resolution": None,
        "status": status,
        "summary": summary or f"Bug {bug_id}",
    }
    if extra:
        value.update(extra)
    return value


class CollectorTestCase(unittest.TestCase):
    maxDiff = None

    def run_collector(self, input_policy, responses, *, timestamp=TIMESTAMP):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            policy_path = root / "policy.json"
            scenario_path = root / "scenario.json"
            output_path = root / "nested" / "collection.json"
            log_path = root / "commands.ndjson"
            if isinstance(input_policy, bytes):
                policy_path.write_bytes(input_policy)
            else:
                policy_path.write_text(json.dumps(input_policy), encoding="utf-8")
            scenario_path.write_text(json.dumps({"responses": responses}), encoding="utf-8")
            env = os.environ.copy()
            env["BZR_DEPENDENCY_RUNNER_SCENARIO"] = str(scenario_path)
            env["BZR_DEPENDENCY_RUNNER_LOG"] = str(log_path)
            command = [
                sys.executable, str(COLLECT), "--policy", str(policy_path),
                "--output", str(output_path), "--runner", str(RUNNER),
            ]
            if timestamp is not None:
                command.extend(["--analysis-timestamp", timestamp])
            result = subprocess.run(command, capture_output=True, env=env, check=False)
            output = output_path.read_bytes() if output_path.exists() else None
            log = []
            if log_path.exists():
                log = [json.loads(line) for line in log_path.read_text(encoding="utf-8").splitlines()]
            self.assert_read_only_commands(log)
            return result, output, log

    def assert_read_only_commands(self, log):
        allowed = {("bug", "view"), ("bug", "list"), ("bug", "search"), ("query", "run")}
        for argv in log:
            self.assertGreaterEqual(len(argv), 5)
            self.assertEqual(argv[:3:2], ["--server", "--json"])
            self.assertIn(tuple(argv[3:5]), allowed)
            self.assertNotIn("--paginate", argv)
            if tuple(argv[3:5]) != ("bug", "view"):
                self.assertEqual(argv[argv.index("--sort") + 1], "bug_id")
                self.assertEqual(argv[argv.index("--order") + 1], "asc")

    def parse(self, output):
        self.assertIsNotNone(output)
        self.assertTrue(output.endswith(b"\n"))
        return json.loads(output)

    def test_da04_scope_and_detail_use_one_canonical_snapshot(self):
        scope = {"kind": "saved-query", "server": "primary", "name": "delivery"}
        first = list_argv("primary", 5, 0, "query", "run", "delivery")
        probe = list_argv("primary", 4, 1, "query", "run", "delivery")
        responses = [
            ok(first, [{"id": 1}]),
            ok(probe, []),
            ok(view_argv("primary", 1), bug(1, depends=[2])),
            ok(view_argv("primary", 2), bug(2, blocks=[])),
        ]
        result, output, log = self.run_collector(policy([scope], max_nodes=4), responses)
        self.assertEqual(result.returncode, 0, result.stderr)
        document = self.parse(output)
        self.assertEqual([node["id"] for node in document["nodes"]], [1, 2])
        self.assertEqual(len([argv for argv in log if argv[3:5] == ["bug", "view"]]), 2)
        self.assertEqual(document["observations"][0]["target"]["id"], 2)

    def test_da06_exact_cap_boundary_analyzes_without_omissions(self):
        responses = [
            ok(view_argv("primary", 1), bug(1, depends=[2])),
            ok(view_argv("primary", 2), bug(2, depends=[3, 1])),
        ]
        result, output, log = self.run_collector(
            policy([bug_scope("primary", 1)], max_nodes=3), responses
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        document = self.parse(output)
        self.assertEqual([node["id"] for node in document["nodes"]], [1, 2, 3])
        self.assertEqual(document["nodes"][-1]["boundary_reason"], "pending_fetch")
        self.assertEqual(document["cap"], {
            "graph_cap_reached": True,
            "omitted_discovered_identities": 0,
            "scope_truncated": False,
        })
        self.assertEqual(document["limitations"], ["graph-node-cap"])
        self.assertEqual(len(log), len({tuple(argv) for argv in log}))

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            collection_path = root / "collection.json"
            analysis_path = root / "analysis.json"
            collection_path.write_bytes(output)
            analysis_result = subprocess.run([
                sys.executable,
                str(ANALYZE),
                "--input",
                str(collection_path),
                "--output",
                str(analysis_path),
                "--allow-partial",
            ], capture_output=True, check=False, timeout=5)
            self.assertEqual(analysis_result.returncode, 0, analysis_result.stderr)
            analysis = json.loads(analysis_path.read_text(encoding="utf-8"))
        self.assertEqual(analysis["cap"], document["cap"])
        self.assertEqual(analysis["status"], "partial")

    def test_da13_broad_scope_stops_at_cap_plus_one(self):
        scope = {"kind": "product", "server": "primary", "value": "Widget"}
        argv = list_argv("primary", 3, 0, "bug", "list", "--product", "Widget")
        responses = [
            ok(argv, [{"id": 3}, {"id": 1}, {"id": 2}]),
            ok(view_argv("primary", 1), bug(1)),
            ok(view_argv("primary", 2), bug(2)),
        ]
        result, output, log = self.run_collector(
            policy([scope], max_nodes=2), responses
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        document = self.parse(output)
        self.assertEqual([root["id"] for root in document["roots"]], [1, 2])
        self.assertEqual(document["limitations"], ["scope-node-cap"])
        self.assertTrue(document["cap"]["scope_truncated"])
        self.assertEqual(len(log), 3)

    def test_da13_oversized_query_restriction_refuses_before_traversal(self):
        restriction = {"kind": "saved-query", "server": "primary", "name": "allowed"}
        argv = list_argv("primary", 3, 0, "query", "run", "allowed")
        result, output, log = self.run_collector(
            policy([bug_scope("primary", 1)], max_nodes=2, restriction=restriction),
            [ok(argv, [{"id": 1}, {"id": 2}, {"id": 3}])],
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        document = self.parse(output)
        self.assertEqual(document["nodes"], [])
        self.assertEqual(document["roots"], [])
        self.assertEqual(document["limitations"], ["restriction-node-cap"])
        self.assertEqual(log, [argv])

    def test_da14_mixed_scope_and_response_permutations_are_byte_identical(self):
        query = {"kind": "saved-query", "server": "zeta", "name": "q"}
        custom = {
            "kind": "custom-search",
            "server": "zeta",
            "url": "https://bugs.invalid/buglist.cgi?product=X&token=secret",
            "parameter_names": ["product"],
        }
        numeric = bug_scope("zeta", 3, 1, 3)
        policies = [
            policy([query, custom, numeric], max_nodes=4),
            policy([numeric, custom, query], max_nodes=4),
        ]
        query_first = list_argv("zeta", 5, 0, "query", "run", "q")
        query_probe = list_argv("zeta", 4, 1, "query", "run", "q")
        search_first = list_argv(
            "zeta", 5, 0, "bug", "search", "--from-url", custom["url"]
        )
        search_probe = list_argv("zeta", 4, 1, "bug", "search", "--from-url", custom["url"])
        common = [
            ok(query_first, [{"id": 2}]), ok(query_probe, []),
            ok(search_first, [{"id": 4}]), ok(search_probe, []),
            ok(view_argv("zeta", 1), bug(1, depends=[4, 2])),
            ok(view_argv("zeta", 2), bug(2)),
            ok(view_argv("zeta", 3), bug(3)),
            ok(view_argv("zeta", 4), bug(4)),
        ]
        first = self.run_collector(policies[0], common)[1]
        second = self.run_collector(policies[1], list(reversed(common)))[1]
        self.assertEqual(first, second)
        document = self.parse(first)
        self.assertEqual(
            [entry["scope_kind"] for entry in document["provenance"]],
            ["bug-ids", "saved-query", "custom-search"],
        )
        self.assertNotIn(b"token=secret", first)

    def test_da15a_api_100_and_101_are_nonfatal_not_found_nodes(self):
        responses = [
            ok(view_argv("primary", 1), bug(1, depends=[4, 3, 2])),
            failed(view_argv("primary", 2), "api", api_code=100),
            failed(view_argv("primary", 3), "api", api_code=101),
            ok(view_argv("primary", 4), bug(4)),
        ]
        result, output, log = self.run_collector(policy([bug_scope("primary", 1)]), responses)
        self.assertEqual(result.returncode, 0, result.stderr)
        document = self.parse(output)
        states = {node["id"]: (node["state"], node["error_type"]) for node in document["nodes"]}
        self.assertEqual(states[2], ("unknown", "not_found"))
        self.assertEqual(states[3], ("unknown", "not_found"))
        self.assertEqual(states[4], ("known", None))
        self.assertEqual(len(log), 4)
        self.assertNotIn(b"private server detail", output)

    def test_da15b_api_102_is_nonfatal_inaccessible_and_continues(self):
        responses = [
            ok(view_argv("primary", 1), bug(1, depends=[2, 3])),
            failed(view_argv("primary", 2), "api", api_code=102),
            ok(view_argv("primary", 3), bug(3)),
        ]
        result, output, log = self.run_collector(policy([bug_scope("primary", 1)]), responses)
        self.assertEqual(result.returncode, 0, result.stderr)
        document = self.parse(output)
        node = next(node for node in document["nodes"] if node["id"] == 2)
        self.assertEqual((node["state"], node["error_type"]), ("unknown", "inaccessible"))
        self.assertEqual(len(log), 3)
        self.assertNotIn(b"private server detail", output)

    def test_da15c_other_api_and_http_failures_are_run_fatal(self):
        for error_type, api_code, limitation in [
            ("api", 103, "collection-api"),
            ("http", None, "collection-http"),
        ]:
            with self.subTest(error_type=error_type):
                responses = [
                    ok(view_argv("primary", 1), bug(1, depends=[2, 3])),
                    failed(view_argv("primary", 2), error_type, api_code=api_code),
                ]
                result, output, log = self.run_collector(
                    policy([bug_scope("primary", 1)]), responses
                )
                self.assertEqual(result.returncode, 1)
                document = self.parse(output)
                self.assertEqual(document["limitations"], [limitation])
                self.assertEqual(document["status"], "partial")
                interrupted = [
                    node for node in document["nodes"] if node["boundary_reason"] == "fetch_interrupted"
                ]
                self.assertEqual([node["id"] for node in interrupted], [2, 3])
                self.assertNotIn(b"private server detail", output + result.stderr)
                self.assertEqual(len(log), 2)

    def test_da17_fatal_after_success_preserves_valid_partial_inventory(self):
        responses = [
            ok(view_argv("primary", 1), bug(1, depends=[2, 3])),
            ok(view_argv("primary", 2), bug(2, blocks=[1])),
            failed(view_argv("primary", 3), "http", raw="credential=do-not-copy"),
        ]
        result, output, _ = self.run_collector(policy([bug_scope("primary", 1)]), responses)
        self.assertEqual(result.returncode, 1)
        document = self.parse(output)
        self.assertEqual({node["id"]: node["state"] for node in document["nodes"]}, {
            1: "known", 2: "known", 3: "boundary",
        })
        self.assertEqual(document["observations"], [
            {
                "field": "depends_on",
                "source": {"id": 1, "server": "primary"},
                "target": {"id": 2, "server": "primary"},
            },
            {
                "field": "depends_on",
                "source": {"id": 1, "server": "primary"},
                "target": {"id": 3, "server": "primary"},
            },
            {
                "field": "blocks",
                "source": {"id": 2, "server": "primary"},
                "target": {"id": 1, "server": "primary"},
            },
        ])
        self.assertNotIn(b"credential=do-not-copy", output + result.stderr)

    def test_da18_alias_success_collapses_with_numeric_root_at_caps_and_permutations(self):
        fixture_policy = json.loads((FIXTURES / "alias-collapse.policy.json").read_text())
        response = ok(
            view_argv("primary", "delivery"),
            bug(10, depends=[11], summary="Delivery"),
        )
        result, output, log = self.run_collector(fixture_policy, [response])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(output, (FIXTURES / "alias-collapse.expected.json").read_bytes())
        self.assertEqual(log, [view_argv("primary", "delivery")])

        reversed_policy = dict(fixture_policy)
        reversed_policy["scopes"] = list(reversed(fixture_policy["scopes"]))
        self.assertEqual(self.run_collector(reversed_policy, [response])[1], output)

        cap_two = json.loads(json.dumps(fixture_policy))
        cap_two["bounds"]["max_nodes"] = 2
        result, output, log = self.run_collector(cap_two, [response])
        self.assertEqual(result.returncode, 0, result.stderr)
        document = self.parse(output)
        self.assertEqual([node["id"] for node in document["nodes"]], [10, 11])
        self.assertEqual(document["nodes"][1]["boundary_reason"], "pending_fetch")
        self.assertEqual(log, [view_argv("primary", "delivery")])

    def test_da18_alias_not_found_keeps_reserved_nonnumeric_root(self):
        response = failed(view_argv("primary", "missing-alias"), "api", api_code=101)
        result, output, log = self.run_collector(
            policy([alias_scope("primary", "missing-alias"), bug_scope("primary", 10)], max_nodes=1),
            [response],
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        document = self.parse(output)
        self.assertEqual(document["roots"], [
            {"id": None, "requested": "missing-alias", "server": "primary"}
        ])
        self.assertEqual(document["nodes"][0]["error_type"], "not_found")
        self.assertEqual(log, [view_argv("primary", "missing-alias")])

    def test_da18_resolved_alias_outside_saved_query_is_not_staged_or_traversed(self):
        restriction = {"kind": "saved-query", "server": "primary", "name": "allowed"}
        first = list_argv("primary", 4, 0, "query", "run", "allowed")
        probe = list_argv("primary", 3, 1, "query", "run", "allowed")
        responses = [
            ok(first, [{"id": 1}]),
            ok(probe, []),
            ok(view_argv("primary", "delivery"), bug(2, depends=[1], summary="Delivery")),
            ok(view_argv("primary", 1), bug(1)),
        ]
        result, output, log = self.run_collector(
            policy(
                [alias_scope("primary", "delivery"), bug_scope("primary", 1)],
                max_nodes=3,
                restriction=restriction,
            ),
            responses,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        document = self.parse(output)
        alias_node = next(node for node in document["nodes"] if node["id"] == 2)
        self.assertEqual((alias_node["state"], alias_node["boundary_reason"]), (
            "boundary", "scope_restriction",
        ))
        self.assertEqual(alias_node["requested_aliases"], ["delivery"])
        self.assertEqual(document["observations"], [])
        self.assertEqual(log, [
            first,
            probe,
            view_argv("primary", "delivery"),
            view_argv("primary", 1),
        ])

    def test_da21_direction_isolation_and_two_pass_reciprocal_evidence(self):
        expected_nodes = {
            "depends_on": [1, 2, 4],
            "blocks": [1, 2, 3],
            "both": [1, 2, 3],
        }
        for direction in ("depends_on", "blocks", "both"):
            with self.subTest(direction=direction):
                responses = [
                    ok(view_argv("primary", 1), bug(1, depends=[2], blocks=[2, 3])),
                    ok(view_argv("primary", 2), bug(2, depends=[1, 4], blocks=[1])),
                ]
                result, output, _ = self.run_collector(
                    policy([bug_scope("primary", 2, 1)], max_nodes=3, direction=direction),
                    responses,
                )
                self.assertEqual(result.returncode, 0, result.stderr)
                document = self.parse(output)
                self.assertEqual([node["id"] for node in document["nodes"]], expected_nodes[direction])
                reciprocal = [
                    observation["field"] for observation in document["observations"]
                    if {observation["source"]["id"], observation["target"]["id"]} == {1, 2}
                ]
                self.assertEqual(sorted(reciprocal), ["blocks", "blocks", "depends_on", "depends_on"])

    def test_malformed_structured_error_fields_are_rejected(self):
        string_fields = [
            "type", "message", "field", "value", "last_change_time",
            "if_match_token", "resource", "identifier", "server", "expected", "actual",
        ]
        integer_fields = ["bug_id", "status", "api_code", "succeeded", "failed"]
        argv = view_argv("primary", 1)
        for field, invalid in [
            *((field, 7) for field in string_fields),
            *((field, "7") for field in integer_fields),
        ]:
            with self.subTest(field=field):
                error = {"type": "api", "message": "private", "exit_code": 4, field: invalid}
                response = {
                    "argv": argv,
                    "exit_code": 4,
                    "stderr": {"schema_version": SCHEMA_VERSION, "error": error},
                }
                result, output, log = self.run_collector(
                    policy([bug_scope("primary", 1)]), [response]
                )
                self.assertEqual(result.returncode, 1)
                self.assertEqual(self.parse(output)["limitations"], [
                    "collection-malformed-output"
                ])
                self.assertEqual(log, [argv])

    def test_structured_error_exit_code_must_be_in_contract_range(self):
        argv = view_argv("primary", 1)
        response = {
            "argv": argv,
            "exit_code": 15,
            "stderr": {
                "schema_version": SCHEMA_VERSION,
                "error": {"type": "api", "message": "private", "exit_code": 15},
            },
        }
        result, output, log = self.run_collector(
            policy([bug_scope("primary", 1)]), [response]
        )
        self.assertEqual(result.returncode, 1)
        self.assertEqual(self.parse(output)["limitations"], ["collection-malformed-output"])
        self.assertEqual(log, [argv])

    def test_unknown_policy_keys_are_rejected_before_runner(self):
        malformed = policy([bug_scope("primary", 1)])
        malformed["surprise"] = True
        result, output, log = self.run_collector(malformed, [])
        self.assertEqual(result.returncode, 2)
        self.assertIsNone(output)
        self.assertEqual(log, [])

    def test_max_nodes_accepts_9999_and_crosses_into_analyzer(self):
        input_policy = policy([bug_scope("primary", 1)], max_nodes=9_999)
        response = ok(view_argv("primary", 1), bug(1))
        result, output, log = self.run_collector(input_policy, [response])
        self.assertEqual(result.returncode, 0, result.stderr)
        collection = self.parse(output)
        self.assertEqual(collection["bounds"]["max_nodes"], 9_999)
        self.assertEqual(log, [view_argv("primary", 1)])

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            collection_path = root / "collection.json"
            analysis_path = root / "analysis.json"
            collection_path.write_bytes(output)
            analysis_result = subprocess.run(
                [
                    sys.executable,
                    str(ANALYZE),
                    "--input",
                    str(collection_path),
                    "--output",
                    str(analysis_path),
                ],
                capture_output=True,
                check=False,
            )
        self.assertEqual(analysis_result.returncode, 0, analysis_result.stderr)

    def test_max_nodes_rejects_10000_before_runner(self):
        input_policy = policy([bug_scope("primary", 1)], max_nodes=10_000)
        result, output, log = self.run_collector(input_policy, [])
        self.assertEqual(result.returncode, 2)
        self.assertIsNone(output)
        self.assertEqual(log, [])
        self.assertIn(b"bounds.max_nodes must be at most 9999", result.stderr)

    def test_duplicate_policy_keys_are_rejected_before_runner(self):
        serialized = json.dumps(policy([bug_scope("primary", 1)]))
        duplicate_cases = [
            serialized.replace(
                '"max_nodes": 10',
                '"max_nodes": 10, "max_nodes": 9',
                1,
            ),
            serialized.replace(
                '"direction": "both"',
                '"direction": "both", "direction": "blocks"',
                1,
            ),
        ]
        for duplicate in duplicate_cases:
            with self.subTest(policy=duplicate):
                result, output, log = self.run_collector(
                    duplicate.encode("utf-8"),
                    [],
                )
                self.assertEqual(result.returncode, 2)
                self.assertIsNone(output)
                self.assertEqual(log, [])
                self.assertIn(b"duplicate JSON key", result.stderr)

    def test_invalid_utf8_policy_is_rejected_cleanly_before_runner(self):
        result, output, log = self.run_collector(b"\xff", [])
        self.assertEqual(result.returncode, 2)
        self.assertIsNone(output)
        self.assertEqual(log, [])
        self.assertEqual(
            result.stderr,
            b"policy error: policy must be readable valid JSON\n",
        )


if __name__ == "__main__":
    unittest.main()
