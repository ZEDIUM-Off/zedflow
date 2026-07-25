#!/usr/bin/env python3
"""Focused tests for read-only recovery classification plumbing."""
from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("recovery", ROOT / "tools/pi-port-swarm/recovery.py")
assert SPEC and SPEC.loader
recovery = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = recovery
SPEC.loader.exec_module(recovery)


class RecoveryTests(unittest.TestCase):
    def test_final_result_requires_one_known_classification(self) -> None:
        result = recovery.final_result('{"classification":"PLAN_CHANGE_REQUIRED","unit":"V1","summary":"tests/a.rs:7"}\n')
        self.assertEqual(result["classification"], "PLAN_CHANGE_REQUIRED")
        with self.assertRaises(RuntimeError):
            recovery.final_result('{"classification":"REPAIRABLE"}\n{"classification":"TRANSIENT"}\n')

    def test_active_failures_excludes_historical_records(self) -> None:
        state = {"units": {"V1": {"status": "BLOCKED"}, "OLD": {"status": "FAILED"}, "DONE": {"status": "ACCEPTED"}}}
        snapshot = {"dag_progress": {"blockers": {"V1": "cargo check"}}}
        self.assertEqual(recovery.active_failures(state, snapshot), {"V1": {"status": "BLOCKED"}})
        self.assertEqual(recovery.active_failures(state, {"error": "monitor failed"}), {})

    def test_resume_records_classification_then_starts_service(self) -> None:
        calls = []
        class Result: returncode = 0
        self.assertTrue(recovery.resume("PLAN_CHANGE_REQUIRED", "V1", "tests/a.rs:7", runner=lambda *args, **kwargs: calls.append(args[0]) or Result(), starter=lambda: True))
        self.assertEqual(calls[0][-6:], ["--unit", "V1", "--classification", "PLAN_CHANGE_REQUIRED", "--reason", "tests/a.rs:7"])

    def test_start_controller_is_bounded_to_reset_and_start(self) -> None:
        calls = []
        class Result: returncode = 0
        self.assertTrue(recovery.start_controller(lambda command, **kwargs: calls.append(command) or Result()))
        self.assertEqual([call[2] for call in calls], ["reset-failed", "start"] )


if __name__ == "__main__":
    unittest.main()
