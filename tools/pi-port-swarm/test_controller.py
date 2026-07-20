#!/usr/bin/env python3
"""Focused no-network tests for controller structural and launch invariants."""
from __future__ import annotations

import copy
import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("controller", ROOT / "tools/pi-port-swarm/controller.py")
assert SPEC and SPEC.loader
controller = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = controller
SPEC.loader.exec_module(controller)


def dag() -> dict:
    return {
        "version": 2,
        "source_gitlink": "references/pi@" + "a" * 40,
        "max_active_writers": 1,
        "units": [
            {"id": "A", "kind": "writer", "depends_on": [], "ownership": ["crates/a"], "validation": [], "intent": "a"},
            {"id": "B", "kind": "writer", "depends_on": ["A"], "ownership": ["crates/a"], "validation": [], "intent": "b"},
            {"id": "C", "kind": "writer", "depends_on": ["A"], "ownership": ["crates/c"], "validation": [], "intent": "c"},
        ],
    }


class ControllerTests(unittest.TestCase):
    def test_validate_dag_and_rejections(self) -> None:
        self.assertEqual([unit["id"] for unit in controller.validate_dag(dag())], ["A", "B", "C"])
        for mutate in (
            lambda value: value["units"].append(copy.deepcopy(value["units"][0])),
            lambda value: value["units"][1].update(depends_on=["missing"]),
            lambda value: value["units"][0].update(depends_on=["B"]),
            lambda value: value.update(source_gitlink="references/pi@bad"),
            lambda value: value["units"][0].update(kind="bad"),
            lambda value: value["units"][0].update(ownership=["../escape"]),
            lambda value: value.update(max_active_writers=2),
        ):
            value = dag()
            mutate(value)
            with self.assertRaises(controller.ControllerError):
                controller.validate_dag(value)

    def test_concurrent_ownership_is_rejected_but_serialized_is_allowed(self) -> None:
        value = dag()
        value["units"][2]["ownership"] = ["crates/a"]
        with self.assertRaises(controller.ControllerError):
            controller.validate_dag(value)
        value["units"][2]["depends_on"] = ["B"]
        self.assertEqual(len(controller.validate_dag(value)), 3)

    def test_readiness_is_dependency_and_file_order_deterministic(self) -> None:
        value = dag()
        state = {"units": {"A": {"status": "ACCEPTED"}}}
        self.assertEqual([unit["id"] for unit in controller.ready_units(value["units"], state)], ["B", "C"])
        state["units"]["B"] = {"status": "RUNNING"}
        self.assertEqual([unit["id"] for unit in controller.ready_units(value["units"], state)], ["C"])

    def test_result_requires_exactly_one_final_schema(self) -> None:
        result = controller.result_line('noise\n{"status":"DONE"}\n')
        self.assertEqual(result["status"], "DONE")
        with self.assertRaises(controller.ControllerError):
            controller.result_line('{"status":"DONE"}\n{"status":"BLOCKED"}')

    def test_assignment_command_is_fresh_and_has_no_resume(self) -> None:
        first = controller.pi_command(Path("worker.md"), Path("/tmp/session-one"), "one", "capsule")
        second = controller.pi_command(Path("worker.md"), Path("/tmp/session-two"), "two", "capsule")
        self.assertIn("--session-dir", first)
        self.assertNotIn("--continue", first)
        self.assertNotIn("--resume", first)
        self.assertNotEqual(first, second)

    def test_ownership_and_control_restriction(self) -> None:
        self.assertTrue(controller.owns(["docs/porting"], "docs/porting/a.md"))
        self.assertFalse(controller.owns(["docs/porting"], "docs/planning/a.md"))
        self.assertTrue(all(controller.owns(list(controller.CONTROL_OWNERSHIP), path) for path in ["tools/pi-port-swarm/dag.json", ".agents/port-swarm/state.json", "docs/porting/a.md"]))
        self.assertFalse(controller.owns(list(controller.CONTROL_OWNERSHIP), "crates/zedflow-agent/src/lib.rs"))

    def test_seed_migration_uses_immutable_sha(self) -> None:
        repo = ROOT
        actual = json.loads((repo / "tools/pi-port-swarm/dag.json").read_text())
        state = controller.seed_runtime(repo, actual, "refs/heads/automation/pi-port")
        self.assertEqual(state["version"], 3)
        self.assertIn("AG-P1", state["units"])
        self.assertNotIn("AG-R1-JSONL-LEAF-ERROR", state["units"])
        self.assertEqual(len(state["pi_gitlink"]), 40)

    def test_status_and_monitor_are_nonmutating(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            state_home = Path(temporary)
            before = list(state_home.rglob("*"))
            for command in ("status", "monitor"):
                completed = subprocess.run([sys.executable, str(ROOT / "tools/pi-port-swarm/controller.py"), "--source", str(ROOT), "--state-dir", str(state_home), command], text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
                self.assertEqual(completed.returncode, 0, completed.stderr)
                self.assertIn("progress", completed.stdout)
            self.assertEqual(before, list(state_home.rglob("*")))

    def port_repo(self) -> tuple[Path, str, str]:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        repo = Path(temporary.name)
        def git(*args: str) -> str:
            return subprocess.check_output(["git", *args], cwd=repo, text=True).strip()
        git("init", "-q")
        git("config", "user.email", "test@example.invalid")
        git("config", "user.name", "test")
        (repo / "owned").mkdir()
        (repo / "owned" / "one").write_text("base")
        pin = "b" * 40
        git("update-index", "--add", "--cacheinfo", f"160000,{pin},references/pi")
        git("add", "owned/one")
        git("commit", "-qm", "base")
        base = git("rev-parse", "HEAD")
        (repo / "owned" / "one").write_text("candidate")
        git("add", "owned/one")
        git("commit", "-qm", "candidate")
        return repo, base, git("rev-parse", "HEAD")

    def test_candidate_acceptance_and_cas_shape(self) -> None:
        repo, base, candidate = self.port_repo()
        unit = {"id": "A", "kind": "writer", "ownership": ["owned"], "validation": []}
        controller.validate_candidate(repo, repo, base, candidate, unit, "b" * 40)
        controller.git(repo, "update-ref", "refs/heads/automation/pi-port", base)
        controller.git(repo, "update-ref", "refs/heads/automation/pi-port", candidate, base)
        self.assertEqual(controller.integration_sha(repo, "refs/heads/automation/pi-port"), candidate)
        with self.assertRaises(controller.ControllerError):
            controller.git(repo, "update-ref", "refs/heads/automation/pi-port", base, base)

    def test_candidate_rejects_outside_ownership(self) -> None:
        repo, base, _candidate = self.port_repo()
        (repo / "outside").write_text("no")
        subprocess.check_call(["git", "add", "outside"], cwd=repo)
        subprocess.check_call(["git", "commit", "-qm", "outside"], cwd=repo)
        candidate = controller.git(repo, "rev-parse", "HEAD")
        unit = {"id": "A", "kind": "writer", "ownership": ["owned"], "validation": []}
        with self.assertRaises(controller.ControllerError):
            controller.validate_candidate(repo, repo, base, candidate, unit, "b" * 40)

    def test_nonblocking_lock_semantics(self) -> None:
        # The controller uses LOCK_NB and returns 75; the primitive is explicit and portable.
        import fcntl
        with tempfile.NamedTemporaryFile() as lock:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            with self.assertRaises(BlockingIOError):
                other = open(lock.name)
                try:
                    fcntl.flock(other, fcntl.LOCK_EX | fcntl.LOCK_NB)
                finally:
                    other.close()


if __name__ == "__main__":
    unittest.main()
