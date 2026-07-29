"""Tests for the fail-closed staging isolation evidence runner."""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from cog_isolation import (
    IsolationError,
    gather,
    load_policy,
    refused,
    run_allowed,
)  # noqa: E402


class CogIsolationTests(unittest.TestCase):
    def test_allowed_command_runs_and_negative_control_never_spawns(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "cog.toml"
            manifest.write_text("""
[console]
allowed_commands = ["hello"]
max_runtime_secs = 2
output_limit_bytes = 128
""")
            policy = load_policy(manifest)
            run = run_allowed(Path("/bin/echo"), "hello", policy)
            self.assertEqual(run["exit"], 0)
            self.assertTrue(run["evidence"]["within_runtime_limit"])
            control = refused("--not-allowed", policy)
            self.assertTrue(control["refused"])
            self.assertFalse(control["spawned"])

    def test_gather_emits_no_process_output_and_passes_exact_allowlist(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "cog.toml"
            manifest.write_text("""
[console]
allowed_commands = ["safe"]
max_runtime_secs = 2
output_limit_bytes = 128
""")
            evidence = gather("test-cog", manifest, Path("/bin/echo"))
            self.assertTrue(evidence["passed"])
            self.assertNotIn("stdout", json.dumps(evidence))
            self.assertEqual(evidence["runs"][-1]["exit"], 4)

    def test_invalid_limits_empty_allowlist_and_overflow_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "cog.toml"
            manifest.write_text("""
[console]
allowed_commands = []
max_runtime_secs = 0
output_limit_bytes = 0
""")
            with self.assertRaises(IsolationError):
                load_policy(manifest)

            policy = {
                "allowed_commands": ["200"],
                "max_runtime_secs": 2,
                "output_limit_bytes": 16,
            }
            run = run_allowed(Path("/usr/bin/seq"), "200", policy)
            self.assertFalse(run["evidence"]["within_output_limit"])


if __name__ == "__main__":
    unittest.main()
