#!/usr/bin/env python3
"""Recorded command runner used only by dependency-analysis fixture tests."""

import json
import os
import sys


def main():
    scenario_path = os.environ["BZR_DEPENDENCY_RUNNER_SCENARIO"]
    log_path = os.environ["BZR_DEPENDENCY_RUNNER_LOG"]
    with open(scenario_path, encoding="utf-8") as handle:
        scenario = json.load(handle)

    argv = sys.argv[1:]
    prior = []
    if os.path.exists(log_path):
        with open(log_path, encoding="utf-8") as handle:
            prior = [json.loads(line) for line in handle if line.strip()]
    occurrence = sum(1 for item in prior if item == argv)
    with open(log_path, "a", encoding="utf-8") as handle:
        handle.write(json.dumps(argv, separators=(",", ":")) + "\n")

    matches = [item for item in scenario["responses"] if item["argv"] == argv]
    if occurrence >= len(matches):
        sys.stderr.write(
            json.dumps(
                {
                    "schema_version": "3.0.0",
                    "error": {
                        "type": "input",
                        "message": "fixture has no response for command",
                        "exit_code": 7,
                    },
                }
            )
            + "\n"
        )
        return 7

    response = matches[occurrence]
    if "stdout" in response:
        write_value(sys.stdout, response["stdout"])
    if "stderr" in response:
        write_value(sys.stderr, response["stderr"])
    return response["exit_code"]


def write_value(stream, value):
    if isinstance(value, str):
        stream.write(value)
    else:
        stream.write(json.dumps(value, separators=(",", ":")) + "\n")


if __name__ == "__main__":
    sys.exit(main())
