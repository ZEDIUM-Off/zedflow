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

    def test_fixed_integration_ref(self) -> None:
        controller.require_integration_ref(controller.INTEGRATION_REF)
        with self.assertRaises(controller.ControllerError):
            controller.require_integration_ref("refs/heads/main")

    def test_graph_is_not_complete_with_active_or_failed_state(self) -> None:
        units = dag()["units"]
        state = {"units": {"A": {"status": "ACCEPTED"}, "B": {"status": "FAILED"}, "C": {"status": "ACCEPTED"}}}
        self.assertFalse(controller.graph_complete(units, state))

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
                self.assertIn("dag_progress", completed.stdout)
                self.assertIn("mechanical_port_inventory", completed.stdout)
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
        submodule = repo / "references/pi"
        submodule.mkdir(parents=True)
        subprocess.check_call(["git", "init", "-q"], cwd=submodule)
        subprocess.check_call(["git", "config", "user.email", "test@example.invalid"], cwd=submodule)
        subprocess.check_call(["git", "config", "user.name", "test"], cwd=submodule)
        (submodule / "pin").write_text("pin")
        subprocess.check_call(["git", "add", "pin"], cwd=submodule)
        subprocess.check_call(["git", "commit", "-qm", "pin"], cwd=submodule)
        pin = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=submodule, text=True).strip()
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
        pin = controller.git(repo / "references/pi", "rev-parse", "HEAD")
        controller.validate_candidate(repo, repo, base, candidate, unit, pin)
        controller.git(repo, "update-ref", "refs/heads/automation/pi-port", base)
        controller.git(repo, "update-ref", "refs/heads/automation/pi-port", candidate, base)
        self.assertEqual(controller.integration_sha(repo, "refs/heads/automation/pi-port"), candidate)
        with self.assertRaises(controller.ControllerError):
            controller.git(repo, "update-ref", "refs/heads/automation/pi-port", base, base)

    def test_integration_ref_is_created_only_from_null_oid(self) -> None:
        repo, base, candidate = self.port_repo()
        controller.ensure_integration_ref(repo, controller.INTEGRATION_REF, base)
        self.assertEqual(controller.integration_sha(repo), base)
        controller.ensure_integration_ref(repo, controller.INTEGRATION_REF, candidate)
        self.assertEqual(controller.integration_sha(repo), base)

    def test_reconcile_accepting_and_interrupted_running(self) -> None:
        repo, base, candidate = self.port_repo()
        controller.git(repo, "update-ref", controller.INTEGRATION_REF, candidate)
        state = {"dag_sha256": controller.dag_hash(dag()), "units": {"A": {"status": "ACCEPTING", "base": base, "candidate": candidate}}}
        self.assertTrue(controller.reconcile_runtime(repo, dag(), state, controller.INTEGRATION_REF))
        self.assertEqual(state["units"]["A"]["status"], "ACCEPTED")
        controller.git(repo, "update-ref", controller.INTEGRATION_REF, base, candidate)
        state = {"dag_sha256": controller.dag_hash(dag()), "units": {"A": {"status": "RUNNING", "base": base, "candidate": None}}}
        controller.reconcile_runtime(repo, dag(), state, controller.INTEGRATION_REF)
        self.assertEqual(state["units"]["A"]["status"], "FAILED")

    def test_candidate_rejects_outside_ownership(self) -> None:
        repo, base, _candidate = self.port_repo()
        (repo / "outside").write_text("no")
        subprocess.check_call(["git", "add", "outside"], cwd=repo)
        subprocess.check_call(["git", "commit", "-qm", "outside"], cwd=repo)
        candidate = controller.git(repo, "rev-parse", "HEAD")
        unit = {"id": "A", "kind": "writer", "ownership": ["owned"], "validation": []}
        pin = controller.git(repo / "references/pi", "rev-parse", "HEAD")
        with self.assertRaises(controller.ControllerError):
            controller.validate_candidate(repo, repo, base, candidate, unit, pin)

    def test_candidate_rejects_head_mismatch_and_dirty_worktree(self) -> None:
        repo, base, candidate = self.port_repo()
        unit = {"id": "A", "kind": "writer", "ownership": ["owned"], "validation": []}
        pin = controller.git(repo / "references/pi", "rev-parse", "HEAD")
        with self.assertRaises(controller.ControllerError):
            controller.validate_candidate(repo, repo, base, base, unit, pin)
        (repo / "untracked").write_text("dirty")
        with self.assertRaises(controller.ControllerError):
            controller.validate_candidate(repo, repo, base, candidate, unit, pin)
        (repo / "untracked").unlink()
        (repo / "references/pi" / "dirty").write_text("dirty")
        with self.assertRaises(controller.ControllerError):
            controller.validate_candidate(repo, repo, base, candidate, unit, pin)

    def test_kind_specific_prompts_and_results(self) -> None:
        source = ROOT
        base = "a" * 40
        units = [
            {"id": "W", "kind": "writer", "ownership": ["owned"], "validation": []},
            {"id": "C", "kind": "checkpoint", "ownership": ["docs"], "validation": []},
            {"id": "V1", "kind": "validator", "ownership": [], "validation": []},
            {"id": "RV-FID", "kind": "reviewer", "ownership": [], "validation": []},
        ]
        expected = ["pi-port-worker-session.md", "pi-port-checkpoint.md", "pi-port-validator.md", "pi-port-reviewer.md"]
        self.assertEqual([controller.prompt_for(source, unit).name for unit in units], expected)
        for unit in units[:2]:
            controller.validate_result(unit, {"status": "DONE", "unit": unit["id"], "base": base, "candidate": base}, base)
        for unit in units[2:]:
            controller.validate_result(unit, {"status": "DONE", "unit": unit["id"], "base": base}, base)
            with self.assertRaises(controller.ControllerError):
                controller.validate_result(unit, {"status": "DONE", "unit": unit["id"], "base": base, "candidate": base}, base)
        with self.assertRaises(controller.ControllerError):
            controller.validate_result(units[2], {"status": "PLAN_CHANGE", "unit": "V1", "base": base}, base)
        controller.validate_result(units[3], {"status": "PLAN_CHANGE", "unit": "RV-FID", "base": base}, base)
        with self.assertRaises(controller.ControllerError):
            controller.validate_result(units[0], {"status": "DONE", "unit": "W", "base": base}, base)

    def test_runtime_dag_revision_and_pending_plan_acceptance(self) -> None:
        actual = json.loads((ROOT / "tools/pi-port-swarm/dag.json").read_text())
        state = controller.seed_runtime(ROOT, actual)
        revised = copy.deepcopy(actual)
        revised["units"][0]["intent"] = "revised"
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "state.json"
            controller.atomic_json(path, state)
            with self.assertRaises(controller.ControllerError):
                controller.load_runtime(ROOT, revised, path)
            controller.mark_plan_acceptance(state, "AG-R1-JSONL-LEAF-ERROR", "b" * 40, "repair order", controller.dag_hash(revised))
            controller.atomic_json(path, state)
            self.assertEqual(controller.load_runtime(ROOT, revised, path)["dag_sha256"], controller.dag_hash(revised))

    def test_continuous_replan_reaches_later_read_only_units(self) -> None:
        revised = {
            "version": 2,
            "source_gitlink": "references/pi@" + "a" * 40,
            "max_active_writers": 1,
            "units": [
                {"id": "OLD", "kind": "writer", "depends_on": [], "ownership": ["old"], "validation": []},
                {"id": "REPAIR", "kind": "writer", "depends_on": [], "ownership": ["repair"], "validation": []},
                {"id": "RV-FID", "kind": "reviewer", "depends_on": ["REPAIR"], "ownership": [], "validation": []},
                {"id": "V1", "kind": "validator", "depends_on": ["RV-FID"], "ownership": [], "validation": []},
            ],
        }
        state = {"units": {"OLD": {"status": "SUPERSEDED"}, "REPAIR": {"status": "ACCEPTED"}, "RV-FID": {"status": "ACCEPTED"}}}
        self.assertTrue(controller.should_continue("replanned", True))
        self.assertTrue(controller.should_continue("accepted", True))
        self.assertFalse(controller.should_continue("blocked", True))
        self.assertEqual([unit["id"] for unit in controller.ready_units(revised["units"], state)], ["V1"])

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
