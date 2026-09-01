#!/usr/bin/env python3
"""Fixture-only checks for the dependency presentation example."""

import json
import subprocess
import sys
import tempfile
import unittest
from html.parser import HTMLParser
from pathlib import Path


HERE = Path(__file__).resolve().parent
ANALYSIS = HERE / "fixtures" / "presentation.analysis.json"
PAGE = HERE / "fixtures" / "presentation.expected.html"
RENDER = HERE.parent / "scripts" / "render.py"
HEADINGS = [
    "Executive summary",
    "Status and unresolved work",
    "Needs attention",
    "Dependency map",
    "Bottlenecks and oldest actionable bugs",
    "Limitations and provenance",
]
FORBIDDEN_TAGS = {
    "audio", "embed", "form", "frame", "iframe", "image", "img", "link",
    "object", "script", "source", "track", "video",
}


class ContractParser(HTMLParser):
    """Record the checked-in page structure without accepting external input."""

    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.errors = []
        self.headings = []
        self.text = []
        self.style_data = []
        self.svg_roles = []
        self.text_alternatives = 0
        self._heading = None
        self._style_depth = 0

    def handle_starttag(self, tag, attrs):
        tag = tag.lower()
        values = {name.lower(): value or "" for name, value in attrs}
        if tag in FORBIDDEN_TAGS:
            self.errors.append(f"forbidden tag: {tag}")
        for name, value in values.items():
            lowered = value.lower()
            if name.startswith("on") or name in {"src", "srcset"}:
                self.errors.append(f"forbidden attribute: {name}")
            if name == "href" and lowered.startswith(("http:", "https:", "//")):
                self.errors.append("remote href")
            if name == "http-equiv" and lowered == "refresh":
                self.errors.append("refresh directive")
            if name == "style" and ("url(" in lowered or "@import" in lowered):
                self.errors.append("remote style attribute")
        if tag == "style":
            self._style_depth += 1
        if tag in {"h2", "h3"}:
            self._heading = []
        if tag == "svg":
            self.svg_roles.append(values.get("role"))
        if "text-alternative" in values.get("class", "").split():
            self.text_alternatives += 1

    def handle_endtag(self, tag):
        tag = tag.lower()
        if tag == "style":
            self._style_depth -= 1
        if tag in {"h2", "h3"} and self._heading is not None:
            self.headings.append(" ".join("".join(self._heading).split()))
            self._heading = None

    def handle_data(self, data):
        self.text.append(data)
        if self._heading is not None:
            self._heading.append(data)
        if self._style_depth:
            self.style_data.append(data)


class PresentationFixtureTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.analysis = json.loads(ANALYSIS.read_text(encoding="utf-8"))
        cls.source = PAGE.read_text(encoding="utf-8")
        cls.parser = ContractParser()
        cls.parser.feed(cls.source)
        cls.visible = " ".join(" ".join(cls.parser.text).split())

    def test_analysis_passes_existing_v1_validator(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "presentation.md"
            result = subprocess.run(
                [sys.executable, str(RENDER), "--input", str(ANALYSIS),
                 "--format", "markdown", "--output", str(output)],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertTrue(output.is_file())

    def test_analysis_is_hostile_partial_and_truncated(self):
        self.assertEqual(self.analysis["status"], "partial")
        self.assertTrue(self.analysis["cap"]["relationship_cap_reached"])
        self.assertGreater(
            self.analysis["cap"]["omitted_relationships_lower_bound"], 0
        )
        self.assertEqual(
            {node["state"] for node in self.analysis["nodes"]},
            {"known", "boundary", "unknown"},
        )
        self.assertIn("<script>", self.analysis["nodes"][0]["summary"])
        self.assertIn("onerror", self.analysis["nodes"][0]["summary"])

    def test_page_has_no_remote_active_content(self):
        self.assertEqual(self.parser.errors, [])
        styles = " ".join(self.parser.style_data).lower()
        self.assertNotIn("url(", styles)
        self.assertNotIn("@import", styles)
        self.assertNotIn("<script", self.source.lower())
        self.assertNotIn("<img", self.source.lower())
        self.assertIn('&lt;script&gt;alert(&quot;x&quot;)&lt;/script&gt;', self.source)
        self.assertIn("&lt;img src=x onerror=alert(1)&gt;", self.source)

    def test_page_contains_ordered_sections_and_accessible_map(self):
        positions = [self.parser.headings.index(heading) for heading in HEADINGS]
        self.assertEqual(positions, sorted(positions))
        self.assertIn("img", self.parser.svg_roles)
        self.assertEqual(self.parser.text_alternatives, 1)
        for relationship in (
            "primary#1 (known) blocks primary#2 (boundary: pending fetch)",
            "primary#1 (known) blocks primary#3 (unknown: inaccessible)",
        ):
            self.assertIn(relationship, self.visible)

    def test_page_preserves_attention_and_evidence_metadata(self):
        required = (
            "status: partial", "1 known", "1 boundary", "1 unknown",
            "Known unresolved 1", "Resolution unknown 2",
            "Stale blockers", "Unassigned blockers", "primary#1 · fan-out 2",
            "last changed 2026-08-01T09:30:00Z", "maximum depth 3",
            "maximum nodes 10", "maximum relationships 2", "relationship cap true",
            "omitted relationships lower bound 2", "primary#2 boundary pending_fetch",
            "primary#3 unknown inaccessible", "server primary",
            "saved query release <unsafe>", "collection command bug view",
        )
        for value in required:
            self.assertIn(value, self.visible)

    def test_page_makes_no_schedule_or_date_claim(self):
        lowered = self.visible.lower()
        for phrase in ("delivery date", "project schedule", "critical path"):
            self.assertNotIn(phrase, lowered)


if __name__ == "__main__":
    unittest.main()
