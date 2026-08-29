"""Behavioral contract for deterministic dependency-graph analysis."""

import copy
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import tracemalloc
import unittest


SKILL_ROOT = Path(__file__).resolve().parents[1]
ANALYZE = SKILL_ROOT / "scripts" / "analyze.py"
FIXTURES = Path(__file__).resolve().parent / "fixtures"
CASES = (
    "branch",
    "diamond",
    "cycle",
    "missing",
    "inaccessible",
    "resolved",
    "empty-partial",
    "cross-server",
    "stale",
)

ANALYZE_SPEC = importlib.util.spec_from_file_location("dependency_analyzer", ANALYZE)
if ANALYZE_SPEC is None or ANALYZE_SPEC.loader is None:
    raise RuntimeError("unable to load dependency analyzer")
ANALYZER = importlib.util.module_from_spec(ANALYZE_SPEC)
ANALYZE_SPEC.loader.exec_module(ANALYZER)


class AnalyzerTestCase(unittest.TestCase):
    maxDiff = None

    def run_analyzer(self, collection, *, allow_partial=False, initial=None):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            input_path = root / "collection.json"
            output_path = root / "nested" / "analysis.json"
            if isinstance(collection, str):
                input_path.write_bytes(
                    (FIXTURES / f"{collection}.collection.json").read_bytes()
                )
            elif isinstance(collection, bytes):
                input_path.write_bytes(collection)
            else:
                input_path.write_text(json.dumps(collection), encoding="utf-8")
            if initial is not None:
                output_path.parent.mkdir(parents=True)
                output_path.write_bytes(initial)
            command = [
                sys.executable,
                str(ANALYZE),
                "--input",
                str(input_path),
                "--output",
                str(output_path),
            ]
            if allow_partial:
                command.append("--allow-partial")
            result = subprocess.run(command, capture_output=True, check=False, timeout=5)
            output = output_path.read_bytes() if output_path.exists() else None
            return result, output

    def analyze(self, case, *, allow_partial=False):
        result, output = self.run_analyzer(case, allow_partial=allow_partial)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIsNotNone(output)
        self.assertTrue(output.endswith(b"\n"))
        return output, json.loads(output)

    def load_collection(self, case):
        return json.loads((FIXTURES / f"{case}.collection.json").read_text())

    def collection_with_isolated_nodes(self, count):
        collection = self.load_collection("branch")
        template = collection["nodes"][0]
        collection["bounds"]["max_nodes"] = count
        collection["nodes"] = [
            {
                **template,
                "id": bug_id,
                "requested": str(bug_id),
                "summary": f"Bug {bug_id}",
            }
            for bug_id in range(1, count + 1)
        ]
        collection["observations"] = []
        collection["roots"] = [{"id": 1, "requested": "1", "server": "primary"}]
        return collection

    def test_fixture_oracles_are_byte_exact(self):
        for case in CASES:
            with self.subTest(case=case):
                partial = case in {"cycle", "empty-partial"}
                output, _ = self.analyze(case, allow_partial=partial)
                self.assertEqual(
                    output,
                    (FIXTURES / f"{case}.analysis.json").read_bytes(),
                )

    def test_da01_branch_and_diamond_have_deterministic_layers_and_paths(self):
        _, branch = self.analyze("branch")
        self.assertEqual(branch["layers"], [["c0001"], ["c0002", "c0003"]])
        self.assertEqual(branch["longest_chain"], {
            "kind": "edge_count", "length": 1, "path": ["c0001", "c0002"]
        })
        self.assertEqual(branch["findings"]["bottlenecks"], [{
            "fan_out": 2,
            "node": {"id": 1, "requested": None, "server": "primary"},
        }])

        _, diamond = self.analyze("diamond")
        self.assertEqual(
            diamond["layers"],
            [["c0001"], ["c0002", "c0003"], ["c0004"]],
        )
        self.assertEqual(diamond["longest_chain"], {
            "kind": "edge_count",
            "length": 2,
            "path": ["c0001", "c0002", "c0004"],
        })
        self.assertEqual(diamond["edges"][0]["observations"], ["blocks", "depends_on"])

    def test_da03_duration_is_null_and_no_schedule_claim_is_emitted(self):
        output, document = self.analyze("diamond")
        self.assertIsNone(document["policy"]["duration"])
        lowered = output.lower()
        self.assertNotIn(b"critical path", lowered)
        self.assertNotIn(b"delivery date", lowered)
        self.assertEqual(document["longest_chain"]["kind"], "edge_count")

    def test_da05_missing_and_inaccessible_nodes_remain_visible(self):
        for case, error_type in (("missing", "not_found"), ("inaccessible", "inaccessible")):
            with self.subTest(case=case):
                _, document = self.analyze(case)
                unknown = next(node for node in document["nodes"] if node["state"] == "unknown")
                self.assertEqual(unknown["error_type"], error_type)
                self.assertEqual(unknown["stale"], "unknown")
                self.assertIn(
                    {"id": unknown["id"], "requested": None, "server": unknown["server"]},
                    document["findings"]["execution_order"]["incomplete_boundaries"],
                )

    def test_da06_cycle_is_collapsed_without_losing_partial_boundaries(self):
        _, document = self.analyze("cycle", allow_partial=True)
        self.assertEqual(document["components"][0], {
            "cyclic": True,
            "id": "c0001",
            "nodes": [
                {"id": 20, "requested": None, "server": "primary"},
                {"id": 21, "requested": None, "server": "primary"},
                {"id": 22, "requested": None, "server": "primary"},
            ],
        })
        self.assertEqual(
            document["findings"]["execution_order"]["cycle_impediments"],
            ["c0001"],
        )
        self.assertEqual(document["longest_chain"]["length"], 1)

    def test_da07_untrusted_node_text_is_copied_as_data(self):
        collection = self.load_collection("branch")
        payload = "<script>```mermaid\n%%{init: evil}%%\nA-->B"
        collection["nodes"][0]["summary"] = payload
        result, output = self.run_analyzer(collection)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(output)["nodes"][0]["summary"], payload)

    def test_da08_same_bug_number_on_two_servers_does_not_collide(self):
        _, document = self.analyze("cross-server")
        members = [component["nodes"][0] for component in document["components"]]
        self.assertEqual(members, [
            {"id": 60, "requested": None, "server": "alpha"},
            {"id": 60, "requested": None, "server": "beta"},
        ])

    def test_da09_resolved_blocker_is_recorded_but_not_unassigned_or_stale(self):
        _, document = self.analyze("resolved")
        resolved = next(node for node in document["nodes"] if node["id"] == 50)
        self.assertFalse(resolved["stale"])
        self.assertEqual(document["findings"]["unassigned_blockers"], [])
        self.assertEqual(document["edges"][0]["predecessor"]["id"], 50)

    def test_da10_scope_restriction_boundary_remains_in_findings(self):
        collection = self.load_collection("missing")
        boundary = collection["nodes"][1]
        boundary.update({
            "boundary_reason": "scope_restriction",
            "error_type": None,
            "state": "boundary",
        })
        result, output = self.run_analyzer(collection)
        self.assertEqual(result.returncode, 0, result.stderr)
        document = json.loads(output)
        self.assertEqual(document["nodes"][1]["stale"], "unknown")
        self.assertEqual(
            document["findings"]["execution_order"]["incomplete_boundaries"],
            [{"id": 31, "requested": None, "server": "primary"}],
        )

    def test_da11_reciprocal_observations_form_one_canonical_edge(self):
        _, document = self.analyze("diamond")
        matching = [
            edge for edge in document["edges"]
            if edge["predecessor"]["id"] == 10 and edge["successor"]["id"] == 11
        ]
        self.assertEqual(len(matching), 1)
        self.assertEqual(matching[0]["observations"], ["blocks", "depends_on"])
        self.assertEqual(document["findings"]["bottlenecks"][0]["fan_out"], 2)

    def test_da16_staleness_is_total_and_uses_only_collection_timestamp(self):
        _, document = self.analyze("stale")
        stale = {node["id"]: node["stale"] for node in document["nodes"]}
        self.assertEqual(stale, {
            70: True,
            71: "unknown",
            72: "unknown",
            73: "unknown",
            74: False,
            76: False,
        })
        self.assertEqual([warning["code"] for warning in document["warnings"]], [
            "stale-timestamp-future",
            "stale-timestamp-unknown",
        ])
        self.assertEqual(document["findings"]["stale_blockers"], [
            {"id": 70, "requested": None, "server": "primary"}
        ])

    def test_da19_pm_findings_preserve_roots_and_deterministic_assumptions(self):
        collection = self.load_collection("cycle")
        _, document = self.analyze("cycle", allow_partial=True)
        self.assertEqual(document["roots"], collection["roots"])
        findings = document["findings"]
        self.assertEqual(findings["structural_roots"], [
            {"id": None, "requested": "ghost", "server": "primary"}
        ])
        self.assertEqual(findings["structural_leaves"], [
            {"id": 23, "requested": None, "server": "primary"},
            {"id": None, "requested": "ghost", "server": "primary"},
        ])
        self.assertEqual(findings["bottlenecks"], [{
            "fan_out": 2,
            "node": {"id": 22, "requested": None, "server": "primary"},
        }])
        self.assertEqual(findings["unassigned_blockers"], [
            {"id": 22, "requested": None, "server": "primary"}
        ])
        self.assertEqual(findings["execution_order"], {
            "assumptions": [
                "cycles-prevent-total-node-order",
                "partial-evidence",
                "resolved-include-no-traverse",
            ],
            "component_order": ["c0001", "c0003", "c0002"],
            "cycle_impediments": ["c0001"],
            "incomplete_boundaries": [
                {"id": 23, "requested": None, "server": "primary"},
                {"id": None, "requested": "ghost", "server": "primary"},
            ],
        })

    def test_da20_empty_partial_graph_has_exact_empty_analysis(self):
        _, document = self.analyze("empty-partial", allow_partial=True)
        self.assertEqual(document["components"], [])
        self.assertEqual(document["edges"], [])
        self.assertEqual(document["layers"], [])
        self.assertEqual(document["longest_chain"], {
            "kind": "edge_count", "length": 0, "path": []
        })
        self.assertEqual(document["findings"], {
            "bottlenecks": [],
            "execution_order": {
                "assumptions": ["partial-evidence", "resolved-include-no-traverse"],
                "component_order": [],
                "cycle_impediments": [],
                "incomplete_boundaries": [],
            },
            "structural_leaves": [],
            "structural_roots": [],
            "stale_blockers": [],
            "unassigned_blockers": [],
        })

    def test_partial_input_requires_explicit_opt_in(self):
        result, output = self.run_analyzer("empty-partial")
        self.assertEqual(result.returncode, 2)
        self.assertIsNone(output)
        self.assertIn(b"--allow-partial", result.stderr)

    def test_collection_schema_is_strict_and_references_must_close(self):
        valid = self.load_collection("branch")
        mutations = []

        extra_top_level = copy.deepcopy(valid)
        extra_top_level["extra"] = True
        mutations.append(extra_top_level)

        wrong_schema = copy.deepcopy(valid)
        wrong_schema["schema"] = "bzr-dependency-collection/v2"
        mutations.append(wrong_schema)

        extra_node_key = copy.deepcopy(valid)
        extra_node_key["nodes"][0]["private"] = "leak"
        mutations.append(extra_node_key)

        dangling_endpoint = copy.deepcopy(valid)
        dangling_endpoint["observations"][0]["target"]["id"] = 999
        mutations.append(dangling_endpoint)

        dangling_root = copy.deepcopy(valid)
        dangling_root["roots"][0]["id"] = 999
        mutations.append(dangling_root)

        duration = copy.deepcopy(valid)
        duration["policy"]["duration"] = 5
        mutations.append(duration)

        empty_resolved_status = copy.deepcopy(valid)
        empty_resolved_status["policy"]["resolved_statuses"] = [""]
        mutations.append(empty_resolved_status)

        private_parameter = copy.deepcopy(valid)
        private_parameter["provenance"][0]["parameter_names"] = ["api_key"]
        mutations.append(private_parameter)

        inconsistent_status = copy.deepcopy(valid)
        inconsistent_status["limitations"] = ["graph-node-cap"]
        inconsistent_status["cap"]["graph_cap_reached"] = True
        mutations.append(inconsistent_status)

        empty_alias = copy.deepcopy(valid)
        empty_alias["nodes"][0]["requested_aliases"] = [""]
        mutations.append(empty_alias)

        for collection in mutations:
            with self.subTest(collection=collection):
                result, output = self.run_analyzer(collection)
                self.assertEqual(result.returncode, 2)
                self.assertIsNone(output)
                self.assertIn(b"analysis input error:", result.stderr)

    def test_partial_cap_limitations_require_matching_flag_and_count_state(self):
        graph_partial = self.load_collection("cycle")
        scope_partial = self.load_collection("empty-partial")
        mutations = []

        graph_limitation_without_flag = copy.deepcopy(graph_partial)
        graph_limitation_without_flag["cap"]["graph_cap_reached"] = False
        mutations.append(graph_limitation_without_flag)

        graph_flag_without_limitation = copy.deepcopy(graph_partial)
        graph_flag_without_limitation["limitations"] = ["collection-http"]
        mutations.append(graph_flag_without_limitation)

        omissions_without_graph_cap = copy.deepcopy(scope_partial)
        omissions_without_graph_cap["cap"]["omitted_discovered_identities"] = 1
        mutations.append(omissions_without_graph_cap)

        relationship_limitation_without_flag = copy.deepcopy(scope_partial)
        relationship_limitation_without_flag["limitations"] = ["relationship_cap"]
        mutations.append(relationship_limitation_without_flag)

        relationship_flag_without_limitation = copy.deepcopy(scope_partial)
        relationship_flag_without_limitation["cap"]["relationship_cap_reached"] = True
        mutations.append(relationship_flag_without_limitation)

        relationship_omissions_without_cap = copy.deepcopy(scope_partial)
        relationship_omissions_without_cap["cap"][
            "omitted_relationships_lower_bound"
        ] = 1
        mutations.append(relationship_omissions_without_cap)

        restriction_limitation_without_flag = copy.deepcopy(scope_partial)
        restriction_limitation_without_flag["cap"]["scope_truncated"] = False
        mutations.append(restriction_limitation_without_flag)

        scope_limitation_without_flag = copy.deepcopy(scope_partial)
        scope_limitation_without_flag["cap"]["scope_truncated"] = False
        scope_limitation_without_flag["limitations"] = ["scope-node-cap"]
        mutations.append(scope_limitation_without_flag)

        scope_flag_without_limitation = copy.deepcopy(scope_partial)
        scope_flag_without_limitation["limitations"] = ["collection-http"]
        mutations.append(scope_flag_without_limitation)

        both_scope_limitations = copy.deepcopy(scope_partial)
        both_scope_limitations["limitations"] = [
            "restriction-node-cap",
            "scope-node-cap",
        ]
        mutations.append(both_scope_limitations)

        for collection in mutations:
            with self.subTest(collection=collection):
                result, output = self.run_analyzer(collection, allow_partial=True)
                self.assertEqual(result.returncode, 2)
                self.assertIsNone(output)
                self.assertIn(b"analysis input error:", result.stderr)

    def test_unknown_limitation_is_rejected_without_replacing_output(self):
        collection = self.load_collection("branch")
        collection["status"] = "partial"
        collection["limitations"] = ["token=secret"]
        result, output = self.run_analyzer(
            collection,
            allow_partial=True,
            initial=b"keep\n",
        )
        self.assertEqual(result.returncode, 2)
        self.assertEqual(output, b"keep\n")
        self.assertEqual(
            result.stderr,
            b"analysis input error: limitations contains an unsupported value\n",
        )

    def test_analyzer_rejects_an_internal_unknown_warning_code(self):
        collection = self.load_collection("branch")
        original = ANALYZER.apply_staleness

        def hostile_staleness(*args):
            nodes, _ = original(*args)
            return nodes, [{"code": "credential=secret", "nodes": []}]

        try:
            ANALYZER.apply_staleness = hostile_staleness
            with self.assertRaisesRegex(
                ANALYZER.AnalysisInputError,
                "warnings contains an unsupported code",
            ):
                ANALYZER.analyze(collection)
        finally:
            ANALYZER.apply_staleness = original

    def test_component_namespace_accepts_9999_isolated_components(self):
        document = ANALYZER.analyze(self.collection_with_isolated_nodes(9_999))
        self.assertEqual(len(document["components"]), 9_999)
        self.assertEqual(document["components"][-1]["id"], "c9999")

    def test_9999_component_chain_is_bounded_and_deterministic(self):
        component_order = [f"c{index:04d}" for index in range(1, 10_000)]
        graph = {
            component: ({component_order[index + 1]} if index < 9_998 else set())
            for index, component in enumerate(component_order)
        }

        tracemalloc.start()
        started = time.monotonic()
        layers = ANALYZER.topological_layers(graph)
        chain = ANALYZER.longest_chain(graph, component_order)
        elapsed = time.monotonic() - started
        _, peak = tracemalloc.get_traced_memory()
        tracemalloc.stop()

        self.assertEqual(len(layers), 9_999)
        self.assertTrue(all(len(layer) == 1 for layer in layers))
        self.assertEqual(layers[0], ["c0001"])
        self.assertEqual(layers[-1], ["c9999"])
        self.assertEqual(chain["length"], 9_998)
        self.assertEqual(chain["path"], component_order)
        self.assertLess(elapsed, 5.0)
        self.assertLess(peak, 32 * 1024 * 1024)

    def test_max_nodes_rejects_10000_before_component_analysis(self):
        collection = self.load_collection("branch")
        collection["bounds"]["max_nodes"] = 10_000
        with self.assertRaisesRegex(
            ANALYZER.AnalysisInputError,
            "bounds.max_nodes must be at most 9999",
        ):
            ANALYZER.analyze(collection)

    def test_max_relationships_accepts_9999_and_rejects_10000(self):
        collection = self.load_collection("branch")
        collection["bounds"]["max_relationships"] = 9_999
        self.assertEqual(
            ANALYZER.analyze(collection)["bounds"]["max_relationships"],
            9_999,
        )
        collection["bounds"]["max_relationships"] = 10_000
        with self.assertRaisesRegex(
            ANALYZER.AnalysisInputError,
            "bounds.max_relationships must be at most 9999",
        ):
            ANALYZER.analyze(collection)

    def test_truncated_input_is_rejected_without_replacing_existing_output(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            input_path = root / "collection.json"
            output_path = root / "analysis.json"
            input_path.write_text('{"schema":', encoding="utf-8")
            output_path.write_text("keep\n", encoding="utf-8")
            result = subprocess.run([
                sys.executable,
                str(ANALYZE),
                "--input",
                str(input_path),
                "--output",
                str(output_path),
            ], capture_output=True, check=False)
            self.assertEqual(result.returncode, 2)
            self.assertEqual(output_path.read_text(encoding="utf-8"), "keep\n")

    def test_duplicate_json_keys_are_rejected(self):
        collection = (FIXTURES / "branch.collection.json").read_bytes()
        marker = b'"schema": "bzr-dependency-collection/v1",'
        duplicated = collection.replace(marker, marker + b"\n  " + marker, 1)
        self.assertNotEqual(duplicated, collection)
        result, output = self.run_analyzer(duplicated)
        self.assertEqual(result.returncode, 2)
        self.assertIsNone(output)
        self.assertIn(b"analysis input error:", result.stderr)


if __name__ == "__main__":
    unittest.main()
