#!/usr/bin/env python3
"""Focused tests for automatic recovery classification plumbing."""

from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("recovery", ROOT / "tools/pi-port-swarm/recovery.py")
assert SPEC and SPEC.loader
recovery = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = recovery
SPEC.loader.exec_module(recovery)


class RecoveryTests(unittest.TestCase):
    def test_final_result_accepts_replan(self) -> None:
        result = recovery.final_result('{"action":"replan","unit":"V1","reason":"tests/a.rs:7","summary":"repairable"}\n')
        self.assertEqual(result["action"], "replan")
        with self.assertRaises(RuntimeError):
            recovery.final_result('{"action":"replan"}\n{"action":"human"}\n')

    def test_active_failures_excludes_historical_records(self) -> None:
        state = {
            "units": {
                "V1": {"status": "BLOCKED"},
                "OLD": {"status": "FAILED"},
                "DONE": {"status": "ACCEPTED"},
            }
        }
        snapshot = {"dag_progress": {"blockers": {"V1": "cargo check"}}}
        self.assertEqual(recovery.active_failures(state, snapshot), {"V1": {"status": "BLOCKED"}})
        self.assertEqual(recovery.active_failures(state, {"error": "monitor failed"}), {})

    def test_ready_dag_without_active_failure_restarts_controller(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            state_dir = Path(directory)
            state_path = state_dir / "state.json"
            state_path.write_text(json.dumps({"units": {}}))
            with (
                mock.patch.object(recovery, "STATE_DIR", state_dir),
                mock.patch.object(recovery, "STATE_PATH", state_path),
                mock.patch.object(recovery, "RECOVERY_DIR", state_dir / "recovery"),
                mock.patch.object(recovery, "monitor", return_value={"ready": ["REPAIR"], "dag_progress": {"blockers": {}}}),
                mock.patch.object(recovery, "bounded_action", return_value=True),
                mock.patch.object(recovery, "start_controller", return_value=True) as start,
                mock.patch.object(recovery, "notify"),
                mock.patch.object(recovery.subprocess, "check_output", return_value="abc\n"),
            ):
                self.assertEqual(recovery.main(), 0)
                start.assert_called_once_with()


if __name__ == "__main__":
    unittest.main()
