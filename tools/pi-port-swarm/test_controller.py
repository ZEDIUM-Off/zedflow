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
from unittest.mock import patch

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
        nested = dag()
        nested["units"][0]["ownership"] = ["crates/a"]
        nested["units"][2]["ownership"] = ["crates/a/src/file.rs"]
        with self.assertRaises(controller.ControllerError):
            controller.validate_dag(nested)

    def test_readiness_is_dependency_and_file_order_deterministic(self) -> None:
        value = dag()
        state = {"units": {"A": {"status": "ACCEPTED"}}}
        self.assertEqual([unit["id"] for unit in controller.ready_units(value["units"], state)], ["B", "C"])
        state["units"]["B"] = {"status": "RUNNING"}
        self.assertEqual([unit["id"] for unit in controller.ready_units(value["units"], state)], ["C"])

    def test_result_requires_exactly_one_final_schema(self) -> None:
        result = controller.result_line('noise\n{"status":"DONE"}\n')
        self.assertEqual(result["status"], "DONE")
        sentinel = controller.result_line('{"status":"PLAN_CHANGE","candidate":"absent"}')
        self.assertNotIn("candidate", sentinel)
        with self.assertRaises(controller.ControllerError):
            controller.result_line('{"status":"DONE"}\n{"status":"BLOCKED"}')

    def test_blocked_reason_preserves_recovery_evidence(self) -> None:
        self.assertEqual(controller.blocked_reason({"summary": "cargo check: tests/a.rs:7 E0308"}), "cargo check: tests/a.rs:7 E0308")
        self.assertEqual(controller.blocked_reason({"reason": "exact", "summary": "short"}), "exact")

    def test_coordinator_receives_source_evidence_and_persists_arbitration(self) -> None:
        unit = dag()["units"][0]
        result = {
            "status": "PLAN_CHANGE",
            "unit": "A",
            "base": "b" * 40,
            "classification": "PLAN_CHANGE_REQUIRED",
            "reason": "needs dependency ownership",
        }
        blocked = {
            "status": "BLOCKED",
            "unit": "REPLAN-A",
            "base": "b" * 40,
            "classification": "ARBITRATION_REQUIRED",
            "reason": "dependency choice required",
        }
        state = {"units": {"A": {"status": "BLOCKED"}}, "history": []}
        with tempfile.TemporaryDirectory() as directory:
            state_path = Path(directory) / "state.json"
            with (
                patch.object(controller, "create_worktree", return_value=(Path(directory), Path(directory) / "session")),
                patch.object(controller, "launch", return_value=blocked) as launch,
            ):
                with self.assertRaises(controller.OutcomeBlocker):
                    controller.accept_plan_change(ROOT, Path(directory), "b" * 40, unit, result, "a" * 40, controller.INTEGRATION_REF, state, state_path)
            control = launch.call_args.args[3]
            self.assertEqual(control["repair_context"]["source_result"], result)
            self.assertEqual(state["units"]["A"]["classification"], "ARBITRATION_REQUIRED")
            self.assertEqual(state["units"]["A"]["blocker"], "dependency choice required")
            self.assertEqual(json.loads(state_path.read_text())["units"]["A"]["coordinator_result"], blocked)

    def test_fixed_integration_ref(self) -> None:
        controller.require_integration_ref(controller.INTEGRATION_REF)
        with self.assertRaises(controller.ControllerError):
            controller.require_integration_ref("refs/heads/main")

    def test_graph_is_not_complete_with_active_or_failed_state(self) -> None:
        units = dag()["units"]
        state = {"units": {"A": {"status": "ACCEPTED"}, "B": {"status": "FAILED"}, "C": {"status": "ACCEPTED"}}}
        self.assertFalse(controller.graph_complete(units, state))

    def test_progress_ignores_historical_failures_outside_active_dag(self) -> None:
        units = dag()["units"]
        state = {"units": {"A": {"status": "ACCEPTED"}, "B": {"status": "FAILED", "blocker": "active"}, "OLD": {"status": "FAILED", "blocker": "stale"}}}
        self.assertEqual(controller.progress(units, state)["blockers"], {"B": "active"})

    def test_progress_prioritizes_failure_blocking_downstream(self) -> None:
        units = [
            {"id": "OLD", "kind": "reviewer", "depends_on": [], "ownership": [], "validation": []},
            {"id": "V", "kind": "validator", "depends_on": [], "ownership": [], "validation": []},
            {"id": "NEXT", "kind": "reviewer", "depends_on": ["V"], "ownership": [], "validation": []},
        ]
        state = {"units": {"OLD": {"status": "FAILED", "blocker": "stale"}, "V": {"status": "BLOCKED", "blocker": "active"}}}
        self.assertEqual(controller.progress(units, state)["blockers"], {"V": "active"})

    def test_assignment_command_is_fresh_and_has_no_resume(self) -> None:
        first = controller.pi_command(Path("worker.md"), Path("/tmp/session-one"), "one", "capsule")
        second = controller.pi_command(Path("worker.md"), Path("/tmp/session-two"), "two", "capsule")
        self.assertIn("--session-dir", first)
        self.assertNotIn("--continue", first)
        self.assertNotIn("--resume", first)
        self.assertNotEqual(first, second)

    def test_manifest_closure_commands_are_allow_listed(self) -> None:
        self.assertEqual(controller.validation_argv("python3 tools/pi-port-swarm/manifest.py check"), ["python3", "tools/pi-port-swarm/manifest.py", "check"])
        self.assertEqual(controller.validation_argv("python3 tools/pi-port-swarm/manifest.py check --package zedflow-ai")[-1], "zedflow-ai")
        with self.assertRaises(controller.ControllerError):
            controller.validation_argv("python3 tools/pi-port-swarm/manifest.py status")

    def test_ownership_and_control_restriction(self) -> None:
        self.assertTrue(controller.owns(["docs/porting"], "docs/porting/a.md"))
        self.assertFalse(controller.owns(["docs/porting"], "docs/planning/a.md"))
        self.assertTrue(all(controller.owns(list(controller.CONTROL_OWNERSHIP), path) for path in ["tools/pi-port-swarm/dag.json", ".agents/port-swarm/state.json", "docs/porting/a.md"]))
        self.assertFalse(controller.owns(list(controller.CONTROL_OWNERSHIP), "crates/zedflow-agent/src/lib.rs"))

    def test_seed_migration_uses_immutable_sha(self) -> None:
        repo = ROOT
        actual = json.loads((repo / "tools/pi-port-swarm/dag.json").read_text())
        state = controller.seed_runtime(repo, actual, "refs/heads/automation/pi-port")
        self.assertEqual(state["version"], controller.RUNTIME_VERSION)
        self.assertEqual(set(("controller_sha", "integration_sha", "dag_sha", "plan_sha", "pi_gitlink")), {key for key in state if key in {"controller_sha", "integration_sha", "dag_sha", "plan_sha", "pi_gitlink"}})
        self.assertIn("AG-P1", state["units"])
        self.assertIn("AG-R1-JSONL-LEAF-ERROR", state["units"])
        self.assertEqual(len(state["pi_gitlink"]), 40)

    def test_v3_migration_is_in_memory_until_explicit_write(self) -> None:
        actual = json.loads((ROOT / "tools/pi-port-swarm/dag.json").read_text())
        v3 = controller.seed_runtime(ROOT, actual)
        v3["version"] = 3
        v3.pop("controller_sha"); v3.pop("integration_sha"); v3.pop("dag_sha"); v3.pop("plan_sha"); v3.pop("terminal_ids")
        v3["units"]["AG-P1"].update(blocker="kept", worktree="/tmp/worktree")
        v3["history"] = [{"unit": "AG-P1", "status": "FAILED"}]
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "state.json"
            controller.atomic_json(path, v3)
            migrated = controller.load_runtime(ROOT, actual, path, write=False)
            self.assertEqual(migrated["version"], 4)
            self.assertEqual(migrated["units"]["AG-P1"]["worktree"], "/tmp/worktree")
            self.assertEqual(controller.load_json(path)["version"], 3)
            controller.load_runtime(ROOT, actual, path, write=True)
            self.assertEqual(controller.load_json(path)["version"], 4)

    def test_v3_migration_rejects_mismatched_pi_pin(self) -> None:
        actual = json.loads((ROOT / "tools/pi-port-swarm/dag.json").read_text())
        v3 = controller.seed_runtime(ROOT, actual)
        v3.update(version=3, pi_gitlink="0" * 40)
        with self.assertRaises(controller.ControllerError):
            controller.migrate_runtime_v3(ROOT, actual, v3, controller.INTEGRATION_REF)

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

    def test_validate_is_static_and_does_not_read_runtime_identity(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            state_home = Path(temporary)
            (state_home / "state.json").write_text('{"invalid":"runtime"}')
            completed = subprocess.run([sys.executable, str(ROOT / "tools/pi-port-swarm/controller.py"), "--source", str(ROOT), "--state-dir", str(state_home), "validate"], text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            self.assertEqual(completed.returncode, 0, completed.stderr)
            self.assertIn('"status": "valid"', completed.stdout)

    def test_control_upgrade_may_change_only_pinned_control_identity(self) -> None:
        actual = json.loads((ROOT / "tools/pi-port-swarm/dag.json").read_text())
        state = controller.seed_runtime(ROOT, actual)
        state.update(controller_sha="0" * 40, plan_sha="1" * 40)
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "state.json"
            controller.atomic_json(path, state)
            with self.assertRaises(controller.ControllerError):
                controller.load_runtime(ROOT, actual, path)
            loaded = controller.load_runtime(ROOT, actual, path, allow_control_upgrade=True)
            self.assertEqual(loaded["pi_gitlink"], controller.pinned_gitlink(actual))

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

    def test_upgrade_control_preserves_history_and_supersedes_active_failed_frontier(self) -> None:
        repo, _initial, _candidate = self.port_repo()
        pin = controller.git(repo, "ls-tree", "HEAD", "references/pi").split()[2]
        dag_path = repo / controller.DAG_FILE
        dag_path.parent.mkdir(parents=True)
        old_dag = {"version": 2, "source_gitlink": f"references/pi@{pin}", "max_active_writers": 1, "units": [
            {"id": "OLD", "kind": "reviewer", "depends_on": [], "ownership": [], "validation": [], "intent": "failed"},
            {"id": "DOWN", "kind": "writer", "depends_on": ["OLD"], "ownership": ["owned"], "validation": [], "intent": "blocked"},
        ]}
        dag_path.write_text(json.dumps(old_dag), encoding="utf-8")
        plan_path = repo / controller.PLAN_FILE
        plan_path.parent.mkdir(parents=True)
        plan_path.write_text("# approved\n", encoding="utf-8")
        controller.git(repo, "add", controller.DAG_FILE, controller.PLAN_FILE)
        controller.git(repo, "commit", "-qm", "old control")
        base = controller.git(repo, "rev-parse", "HEAD")
        controller.git(repo, "update-ref", controller.INTEGRATION_REF, base)
        target_dag = copy.deepcopy(old_dag)
        target_dag["units"].append({"id": "NEXT", "kind": "writer", "depends_on": [], "ownership": ["recovery"], "validation": [], "intent": "next"})
        dag_path.write_text(json.dumps(target_dag), encoding="utf-8")
        controller.git(repo, "add", controller.DAG_FILE)
        controller.git(repo, "commit", "-qm", "recovery")
        candidate = controller.git(repo, "rev-parse", "HEAD")
        with tempfile.TemporaryDirectory() as temporary:
            state_path = Path(temporary) / "state.json"
            state = {"version": 4, "integration_ref": controller.INTEGRATION_REF, "pi_gitlink": pin, "units": {"OLD": {"status": "FAILED", "blocker": "stale"}}, "terminal_ids": ["OLD"], "history": [{"unit": "OLD"}]}
            controller.atomic_json(state_path, state)
            revised = controller.upgrade_control(repo, state_path, state, candidate, ["OLD"], "approved recovery")
            self.assertEqual(revised, target_dag)
            self.assertEqual(controller.integration_sha(repo), candidate)
            self.assertEqual(state["units"]["OLD"]["status"], "SUPERSEDED")
            checkpoint = f"CONTROL-RECOVERY-{candidate[:12].upper()}"
            self.assertEqual(state["units"][checkpoint]["status"], "ACCEPTED")
            self.assertEqual(state["controller_sha"], candidate)

    def test_control_only_upgrade_preserves_active_dag_and_blocker(self) -> None:
        repo, _initial, _candidate = self.port_repo()
        pin = controller.git(repo, "ls-tree", "HEAD", "references/pi").split()[2]
        dag_path = repo / controller.DAG_FILE
        dag_path.parent.mkdir(parents=True)
        dag = {"version": 2, "source_gitlink": f"references/pi@{pin}", "max_active_writers": 1, "units": [
            {"id": "BLOCKED", "kind": "validator", "depends_on": [], "ownership": [], "validation": [], "intent": "blocked"},
            {"id": "DOWN", "kind": "writer", "depends_on": ["BLOCKED"], "ownership": ["owned"], "validation": [], "intent": "downstream"},
        ]}
        dag_path.write_text(json.dumps(dag), encoding="utf-8")
        plan_path = repo / controller.PLAN_FILE
        plan_path.parent.mkdir(parents=True)
        plan_path.write_text("# approved\n", encoding="utf-8")
        controller.git(repo, "add", controller.DAG_FILE, controller.PLAN_FILE)
        controller.git(repo, "commit", "-qm", "blocked control")
        base = controller.git(repo, "rev-parse", "HEAD")
        controller.git(repo, "update-ref", controller.INTEGRATION_REF, base)
        (repo / "control.txt").write_text("fix\n")
        controller.git(repo, "add", "control.txt")
        controller.git(repo, "commit", "-qm", "control fix")
        candidate = controller.git(repo, "rev-parse", "HEAD")
        with tempfile.TemporaryDirectory() as temporary:
            state_path = Path(temporary) / "state.json"
            state = {"version": 4, "integration_ref": controller.INTEGRATION_REF, "pi_gitlink": pin, "units": {"BLOCKED": {"status": "BLOCKED", "blocker": "test"}}, "terminal_ids": ["BLOCKED"], "history": []}
            controller.atomic_json(state_path, state)
            controller.upgrade_control(repo, state_path, state, candidate, [], "control-only")
            self.assertEqual(state["units"]["BLOCKED"]["status"], "BLOCKED")
            self.assertEqual(controller.integration_sha(repo), candidate)

    def test_cleanup_only_removes_durable_reachable_accepted_worktrees(self) -> None:
        repo, _base, candidate = self.port_repo()
        controller.git(repo, "update-ref", controller.INTEGRATION_REF, candidate)
        with tempfile.TemporaryDirectory() as temporary:
            state_dir = Path(temporary)
            worktree = state_dir / "worktrees" / "a-1-test"
            session = state_dir / "sessions" / "a-1-test"
            session.mkdir(parents=True)
            (session / "controller.log").write_text("durable", encoding="utf-8")
            branch = "automation/pi-port-unit/a-1-test"
            controller.git(repo, "worktree", "add", "-b", branch, str(worktree), candidate)
            state_path = state_dir / "state.json"
            controller.atomic_json(state_path, {"durable": True})
            state = {"units": {
                "A": {"status": "ACCEPTED", "candidate": candidate, "worktree": str(worktree), "session": str(session)},
                "FAILED": {"status": "FAILED", "candidate": candidate, "worktree": str(worktree), "session": str(session)},
            }}
            eligible, retained = controller.cleanup_candidates(repo, state_dir, state_path, state)
            self.assertEqual([action["unit"] for action in eligible], ["A"])
            self.assertIn({"unit": "FAILED", "reason": "not accepted"}, retained)
            self.assertTrue(worktree.is_dir())  # dry-run discovery is read-only
            controller.cleanup_accepted(repo, eligible)
            self.assertFalse(worktree.exists())
            self.assertFalse(controller.git_ok(repo, "show-ref", "--verify", "--quiet", f"refs/heads/{branch}"))

    def test_cleanup_retains_accepted_candidate_not_reachable_from_integration(self) -> None:
        repo, base, candidate = self.port_repo()
        controller.git(repo, "update-ref", controller.INTEGRATION_REF, base)
        with tempfile.TemporaryDirectory() as temporary:
            state_dir = Path(temporary)
            worktree = state_dir / "worktrees" / "a-1-test"
            session = state_dir / "sessions" / "a-1-test"
            session.mkdir(parents=True)
            (session / "controller.log").write_text("durable", encoding="utf-8")
            controller.git(repo, "worktree", "add", "-b", "automation/pi-port-unit/a-1-test", str(worktree), candidate)
            state_path = state_dir / "state.json"
            controller.atomic_json(state_path, {"durable": True})
            state = {"units": {"A": {"status": "ACCEPTED", "candidate": candidate, "worktree": str(worktree), "session": str(session)}}}
            eligible, retained = controller.cleanup_candidates(repo, state_dir, state_path, state)
            self.assertEqual(eligible, [])
            self.assertIn({"unit": "A", "reason": "candidate is not reachable from integration"}, retained)
            self.assertTrue(worktree.is_dir())

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

    def test_worktree_head_overrides_a_misreported_candidate(self) -> None:
        repo, _base, candidate = self.port_repo()
        unit = {"id": "A", "kind": "writer"}
        reported = "9" * 40
        result = controller.authoritative_result(unit, {"status": "DONE", "candidate": reported}, repo)
        self.assertEqual(result["candidate"], candidate)
        self.assertEqual(result["reported_candidate"], reported)

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
        controller.validate_result(units[3], {"status": "PLAN_CHANGE", "unit": "RV-FID", "base": base, "classification": "PLAN_CHANGE_REQUIRED"}, base)
        with self.assertRaises(controller.ControllerError):
            controller.validate_result(units[3], {"status": "PLAN_CHANGE", "unit": "RV-FID", "base": base, "classification": "ARBITRATION_REQUIRED"}, base)
        with self.assertRaises(controller.ControllerError):
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
            controller.mark_plan_acceptance(state, "AG-R1-JSONL-LEAF-ERROR", "b" * 40, "repair order", controller.dag_hash(revised), True)
            controller.atomic_json(path, state)
            self.assertEqual(controller.load_runtime(ROOT, revised, path)["dag_sha256"], controller.dag_hash(revised))

    def test_checkpoint_dag_transition_recovers_legacy_and_interrupted_state(self) -> None:
        repo, _, _ = self.port_repo()
        initial = {
            "version": 2,
            "source_gitlink": "references/pi@" + "a" * 40,
            "max_active_writers": 1,
            "units": [{"id": "NEXT", "kind": "checkpoint", "depends_on": [], "ownership": [controller.DAG_FILE], "validation": []}],
        }
        dag_path = repo / controller.DAG_FILE
        dag_path.parent.mkdir(parents=True)
        dag_path.write_text(json.dumps(initial))
        controller.git(repo, "add", controller.DAG_FILE)
        controller.git(repo, "commit", "-m", "initial dag")
        base = controller.git(repo, "rev-parse", "HEAD")
        revised = copy.deepcopy(initial)
        revised["units"] = [{"id": "LATER", "kind": "reviewer", "depends_on": [], "ownership": [], "validation": []}]
        dag_path.write_text(json.dumps(revised))
        controller.git(repo, "add", controller.DAG_FILE)
        controller.git(repo, "commit", "-m", "extend dag")
        candidate = controller.git(repo, "rev-parse", "HEAD")
        controller.git(repo, "update-ref", controller.INTEGRATION_REF, candidate)

        legacy = {"dag_sha256": controller.dag_hash(initial), "units": {"NEXT": {"status": "ACCEPTED", "base": base, "candidate": candidate, "dag_sha256": controller.dag_hash(initial)}}}
        self.assertTrue(controller.recover_accepted_checkpoint_dag(repo, revised, legacy, controller.INTEGRATION_REF))
        self.assertEqual(legacy["dag_sha256"], controller.dag_hash(revised))

        interrupted = {"dag_sha256": controller.dag_hash(initial), "units": {"NEXT": {"status": "ACCEPTING", "base": base, "candidate": candidate, "dag_sha256": controller.dag_hash(initial), "revised_dag_sha256": controller.dag_hash(revised)}}}
        self.assertTrue(controller.reconcile_runtime(repo, revised, interrupted, controller.INTEGRATION_REF))
        self.assertEqual(interrupted["units"]["NEXT"]["status"], "ACCEPTED")
        self.assertEqual(interrupted["integration_sha"], candidate)
        self.assertEqual(interrupted["dag_sha256"], controller.dag_hash(revised))

    def test_validator_replan_preserves_gate_and_downstream(self) -> None:
        original = [
            {"id": "A", "kind": "writer", "depends_on": [], "ownership": ["a"], "validation": []},
            {"id": "X", "kind": "writer", "depends_on": ["A"], "ownership": ["x"], "validation": []},
            {"id": "V1", "kind": "validator", "depends_on": ["A"], "ownership": [], "validation": ["cargo check"]},
            {"id": "RV", "kind": "reviewer", "depends_on": ["V1", "X"], "ownership": [], "validation": []},
        ]
        revised = [
            original[0],
            original[1],
            {"id": "FIX", "kind": "writer", "depends_on": ["A"], "ownership": ["fix"], "validation": ["cargo test"]},
            {"id": "V2", "kind": "validator", "depends_on": ["FIX"], "ownership": [], "validation": ["cargo check"]},
            {"id": "RV", "kind": "reviewer", "depends_on": ["V2", "X"], "ownership": [], "validation": []},
        ]
        controller.validate_replan_transition(original[2], original, revised)
        with self.assertRaises(controller.ControllerError):
            controller.validate_replan_transition(original[2], original, [original[0]])
        bypass = copy.deepcopy(revised)
        bypass[3]["depends_on"] = ["A"]
        with self.assertRaises(controller.ControllerError):
            controller.validate_replan_transition(original[2], original, bypass)
        dropped_dependency = copy.deepcopy(revised)
        dropped_dependency[4]["depends_on"] = ["V2"]
        with self.assertRaises(controller.ControllerError):
            controller.validate_replan_transition(original[2], original, dropped_dependency)

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

    def test_validation_commands_are_allow_listed(self) -> None:
        self.assertEqual(controller.validation_argv("cargo check --workspace --all-targets")[:2], ["cargo", "check"])
        self.assertEqual(controller.validation_argv("git diff --check"), ["git", "diff", "--check"])
        with self.assertRaises(controller.ControllerError):
            controller.validation_argv("python3 -c 'import os'")
        with self.assertRaises(controller.ControllerError):
            controller.validation_argv("cargo test; rm -rf /")
        with self.assertRaises(controller.ControllerError):
            controller.validation_argv("cargo test -p untrusted-package")

    def test_repairable_writer_retries_without_dag_mutation(self) -> None:
        units = dag()["units"]
        state = {"units": {"A": {"status": "RETRY", "attempt": 1, "classification": "REPAIRABLE"}}}
        self.assertEqual([unit["id"] for unit in controller.ready_units(units, state)], ["A"])
        self.assertTrue(controller.should_continue("repairing", True))
        self.assertFalse(controller.should_continue("repairing", False))

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
