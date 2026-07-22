#!/usr/bin/env python3
"""Focused tests for automatic recovery classification plumbing."""

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


if __name__ == "__main__":
    unittest.main()
