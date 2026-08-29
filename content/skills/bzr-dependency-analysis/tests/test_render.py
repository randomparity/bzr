"""Behavioral contract for safe deterministic dependency-report rendering."""

import copy
import importlib.util
import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import unittest
from unittest import mock


SKILL_ROOT = Path(__file__).resolve().parents[1]
RENDER = SKILL_ROOT / "scripts" / "render.py"
FIXTURES = Path(__file__).resolve().parent / "fixtures"

RENDER_SPEC = importlib.util.spec_from_file_location("dependency_renderer", RENDER)
if RENDER_SPEC is None or RENDER_SPEC.loader is None:
    raise RuntimeError("unable to load dependency renderer")
RENDERER = importlib.util.module_from_spec(RENDER_SPEC)
RENDER_SPEC.loader.exec_module(RENDERER)


class RendererTestCase(unittest.TestCase):
    maxDiff = None

    def run_renderer(self, source, output_format, *, initial=None, option_names=None):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            input_path = root / "analysis.json"
            output_path = root / "nested" / f"report.{output_format}"
            if isinstance(source, bytes):
                input_path.write_bytes(source)
            else:
                input_path.write_text(json.dumps(source), encoding="utf-8")
            if initial is not None:
                output_path.parent.mkdir(parents=True)
                output_path.write_bytes(initial)
            option_names = option_names or {}
            command = [
                sys.executable,
                str(RENDER),
                option_names.get("--input", "--input"),
                str(input_path),
                option_names.get("--format", "--format"),
                output_format,
                option_names.get("--output", "--output"),
                str(output_path),
            ]
            result = subprocess.run(command, capture_output=True, check=False, timeout=5)
            output = output_path.read_bytes() if output_path.exists() else None
            return result, output

    def render_fixture(self, output_format, case="hostile"):
        source = (FIXTURES / f"{case}.analysis.json").read_bytes()
        result, output = self.run_renderer(source, output_format)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIsNotNone(output)
        self.assertTrue(output.endswith(b"\n"))
        return output

    def test_da07_fixture_outputs_are_byte_exact_and_deterministic(self):
        for output_format, extension in (("markdown", "md"), ("mermaid", "mmd")):
            with self.subTest(output_format=output_format):
                first = self.render_fixture(output_format)
                second = self.render_fixture(output_format)
                self.assertEqual(first, second)
                self.assertEqual(
                    first,
                    (FIXTURES / f"hostile.expected.{extension}").read_bytes(),
                )

    def test_da07_markdown_hostile_syntax_is_confined_to_a_complete_fence(self):
        output = self.render_fixture("markdown").decode("utf-8")
        match = re.search(
            r"(?ms)^(?P<fence>`{3,})text\n(?P<body>.*?)\n(?P=fence)$",
            output,
        )
        self.assertIsNotNone(match)
        body = match.group("body")
        outside = output[: match.start()] + output[match.end() :]
        for token in (
            "<script>",
            "</script>",
            "![image]",
            "<https://evil.example>",
            "%%{init:",
        ):
            self.assertIn(token, body)
            self.assertNotIn(token, outside)
        runs = [len(value) for value in re.findall(r"`+", body)]
        self.assertGreater(len(match.group("fence")), max(runs, default=0))
        self.assertIn("release &#91;nightly&#93; &lt;unsafe&gt;", outside)
        self.assertNotIn("token=secret", output)
        self.assertIn("Collection commands: bug search, bug view, query run", output)

    def test_da07_mermaid_hostile_values_remain_inside_quoted_tokens(self):
        output = self.render_fixture("mermaid").decode("utf-8")
        self.assertEqual(output.splitlines()[0], "flowchart TD")
        self.assertNotIn("\n%%{", output)
        self.assertNotIn("<script>", output)
        self.assertNotIn("</script>", output)
        self.assertNotIn("![image]", output)
        self.assertNotIn("<https://evil.example>", output)
        self.assertNotIn("token=secret", output)
        self.assertIn("#quot;", output)
        self.assertNotIn('\\"', output)
        token = re.compile(r'^    [a-z][a-z0-9_]*\["(?P<label>[^"]*)"\]$')
        for line in output.splitlines():
            if "[\"" in line:
                match = token.fullmatch(line)
                self.assertIsNotNone(match, line)
                self.assertNotIn("\\", match.group("label"))
                self.assertNotRegex(match.group("label"), r"[\r\n\t]")
        self.assertIn("n_7072696d617279_80", output)
        self.assertIn("n_7072696d617279_81", output)

    def test_pm_findings_and_incomplete_metadata_render_in_both_formats(self):
        expected_cycle = (
            "Longest dependency chain components: c0001, c0002",
            "Bottlenecks:",
            "fan-out 2",
            "Execution assumptions:",
            "cycles-prevent-total-node-order",
            "partial-evidence",
            "Execution component order: c0001, c0003, c0002",
            "Incomplete boundaries:",
            "Cycle impediments: c0001",
            "Graph cap reached: true",
            "Omitted discovered identities: 1",
            "Scope truncated: false",
            "Limitations: graph-node-cap",
        )
        expected_stale = (
            "Stale blockers:",
            "Analysis warnings:",
            "stale-timestamp-future",
            "stale-timestamp-unknown",
        )
        for output_format in ("markdown", "mermaid"):
            with self.subTest(output_format=output_format):
                cycle = self.render_fixture(output_format, "cycle").decode("utf-8")
                stale = self.render_fixture(output_format, "stale").decode("utf-8")
                normalized_cycle = cycle.replace("&#35;", "#").replace("&#45;", "-")
                normalized_stale = stale.replace("&#35;", "#").replace("&#45;", "-")
                for value in expected_cycle:
                    self.assertIn(value, normalized_cycle)
                for value in expected_stale:
                    self.assertIn(value, normalized_stale)

    def test_strict_schema_rejects_unknown_keys_without_replacing_output(self):
        document = json.loads((FIXTURES / "hostile.analysis.json").read_text())
        document["unexpected"] = True
        result, output = self.run_renderer(document, "markdown", initial=b"keep\n")
        self.assertEqual(result.returncode, 2)
        self.assertEqual(output, b"keep\n")
        self.assertEqual(result.stderr, b"render input error: analysis has invalid keys\n")

    def test_abbreviated_options_are_rejected_without_replacing_output(self):
        source = (FIXTURES / "hostile.analysis.json").read_bytes()
        for full, abbreviated in (
            ("--input", "--inp"),
            ("--format", "--for"),
            ("--output", "--out"),
        ):
            with self.subTest(option=abbreviated):
                result, output = self.run_renderer(
                    source,
                    "markdown",
                    initial=b"keep\n",
                    option_names={full: abbreviated},
                )
                self.assertEqual(result.returncode, 2)
                self.assertEqual(output, b"keep\n")

    def test_cap_metadata_contradictions_do_not_replace_output(self):
        complete = json.loads((FIXTURES / "hostile.analysis.json").read_text())
        partial = copy.deepcopy(complete)
        partial["status"] = "partial"
        partial["limitations"] = ["collection-http"]

        mutations = []

        graph_limitation_without_flag = copy.deepcopy(partial)
        graph_limitation_without_flag["limitations"] = ["graph-node-cap"]
        mutations.append(graph_limitation_without_flag)

        graph_flag_without_limitation = copy.deepcopy(partial)
        graph_flag_without_limitation["cap"]["graph_cap_reached"] = True
        mutations.append(graph_flag_without_limitation)

        omissions_without_graph_cap = copy.deepcopy(partial)
        omissions_without_graph_cap["cap"]["omitted_discovered_identities"] = 1
        mutations.append(omissions_without_graph_cap)

        for limitation in ("restriction-node-cap", "scope-node-cap"):
            scope_limitation_without_flag = copy.deepcopy(partial)
            scope_limitation_without_flag["limitations"] = [limitation]
            mutations.append(scope_limitation_without_flag)

        scope_flag_without_limitation = copy.deepcopy(partial)
        scope_flag_without_limitation["cap"]["scope_truncated"] = True
        mutations.append(scope_flag_without_limitation)

        both_scope_limitations = copy.deepcopy(partial)
        both_scope_limitations["cap"]["scope_truncated"] = True
        both_scope_limitations["limitations"] = [
            "restriction-node-cap",
            "scope-node-cap",
        ]
        mutations.append(both_scope_limitations)

        complete_with_limitation = copy.deepcopy(complete)
        complete_with_limitation["cap"]["graph_cap_reached"] = True
        complete_with_limitation["limitations"] = ["graph-node-cap"]
        mutations.append(complete_with_limitation)

        partial_without_limitation = copy.deepcopy(complete)
        partial_without_limitation["status"] = "partial"
        mutations.append(partial_without_limitation)

        for document in mutations:
            with self.subTest(
                cap=document["cap"],
                limitations=document["limitations"],
                status=document["status"],
            ):
                result, output = self.run_renderer(
                    document,
                    "markdown",
                    initial=b"keep\n",
                )
                self.assertEqual(result.returncode, 2)
                self.assertEqual(output, b"keep\n")
                self.assertIn(b"render input error:", result.stderr)

    def test_graph_cap_accepts_zero_or_positive_omission_counts(self):
        document = json.loads((FIXTURES / "hostile.analysis.json").read_text())
        document["status"] = "partial"
        document["limitations"] = ["graph-node-cap"]
        document["cap"]["graph_cap_reached"] = True
        for count in (0, 2):
            with self.subTest(count=count):
                document["cap"]["omitted_discovered_identities"] = count
                result, output = self.run_renderer(document, "markdown")
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIsNotNone(output)

    def test_duplicate_json_keys_are_rejected(self):
        source = b'{"schema":"bzr-dependency-analysis/v1","schema":"other"}'
        result, output = self.run_renderer(source, "mermaid")
        self.assertEqual(result.returncode, 2)
        self.assertIsNone(output)
        self.assertIn(b"duplicate JSON key: schema", result.stderr)

    def test_max_nodes_accepts_9999_and_rejects_10000(self):
        document = json.loads((FIXTURES / "hostile.analysis.json").read_text())
        document["bounds"]["max_nodes"] = 9_999
        accepted, output = self.run_renderer(document, "markdown")
        self.assertEqual(accepted.returncode, 0, accepted.stderr)
        self.assertIsNotNone(output)

        document["bounds"]["max_nodes"] = 10_000
        rejected, output = self.run_renderer(document, "markdown")
        self.assertEqual(rejected.returncode, 2)
        self.assertIsNone(output)
        self.assertIn(b"bounds.max_nodes must be at most 9999", rejected.stderr)

    def test_every_analysis_fixture_renders_in_both_formats(self):
        for fixture in sorted(FIXTURES.glob("*.analysis.json")):
            for output_format in ("markdown", "mermaid"):
                with self.subTest(fixture=fixture.name, output_format=output_format):
                    result, output = self.run_renderer(fixture.read_bytes(), output_format)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertIsNotNone(output)
                    self.assertTrue(output.endswith(b"\n"))

    def test_atomic_write_preserves_destination_when_replace_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "report.md"
            output.write_text("before\n", encoding="utf-8")
            with mock.patch.object(RENDERER.os, "replace", side_effect=OSError("fault")):
                with self.assertRaises(OSError):
                    RENDERER.atomic_write(output, "after\n")
            self.assertEqual(output.read_text(encoding="utf-8"), "before\n")
            self.assertEqual(list(output.parent.glob(f".{output.name}.*")), [])


if __name__ == "__main__":
    unittest.main()
