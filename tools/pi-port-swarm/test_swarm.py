import importlib.util
import json
import subprocess
import tempfile
import unittest
from pathlib import Path

SPEC = importlib.util.spec_from_file_location("swarm", Path(__file__).with_name("swarm.py"))
swarm = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(swarm)


def dag(units):
    return {"units": units}


def unit(name, deps=(), model="openai-codex/gpt-5.6-terra", ownership=(), role="writer"):
    return {"id": name, "role": role, "model": model, "depends_on": list(deps), "ownership": list(ownership)}


def orchestrated(**fields):
    return {"status": "DONE", "orchestration": {"listed_agents": True, "waited_for_all": True}, **fields}


def repository(path):
    subprocess.run(["git", "init", "-q", path], check=True)
    subprocess.run(["git", "-C", path, "config", "user.email", "test@example.invalid"], check=True)
    subprocess.run(["git", "-C", path, "config", "user.name", "Test"], check=True)
    (path / "a").write_text("x")
    subprocess.run(["git", "-C", path, "add", "a"], check=True)
    subprocess.run(["git", "-C", path, "commit", "-qm", "base"], check=True)
    return swarm.sha(path)


class SwarmTest(unittest.TestCase):
    def test_cycle_is_rejected(self):
        with self.assertRaisesRegex(swarm.DagError, "cycle"):
            swarm.validate_dag(dag([unit("a", ["b"]), unit("b", ["a"])]))

    def test_unknown_dependency_is_rejected(self):
        with self.assertRaisesRegex(swarm.DagError, "unknown dependency"):
            swarm.validate_dag(dag([unit("a", ["missing"])]))

    def test_forbidden_model_is_rejected(self):
        with self.assertRaisesRegex(swarm.DagError, "forbidden model"):
            swarm.validate_dag(dag([unit("a", model="vendor/nope")]))

    def test_ready_selection_has_one_writer_but_parallel_readers(self):
        d = dag([unit("writer-a", ownership=["src/a.rs"]), unit("writer-b", ownership=["src/b.rs"]), unit("review-a", role="reviewer"), unit("review-b", role="reviewer")])
        self.assertEqual([item["id"] for item in swarm.ready_units(d, {"units": {}})], ["writer-a", "review-a", "review-b"])

    def test_snapshot_does_not_change_source_head_or_index(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp)
            before = repository(repo)
            (repo / "a").write_text("two")
            self.assertTrue(swarm.snapshot(repo))
            self.assertEqual(before, swarm.sha(repo))
            self.assertEqual("", swarm.git(repo, "diff", "--cached").stdout)

    def test_forged_integrated_result_is_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo, base = Path(tmp), None
            base = repository(repo)
            subprocess.run(["git", "-C", repo, "branch", "automation/pi-port", base], check=True)
            with self.assertRaisesRegex(swarm.DagError, "unit reported CLOSED"):
                swarm.accept_result(repo, unit("a", ownership=["a"]), {"status": "CLOSED"}, base)

    def test_blocked_result_preserves_summary_and_failed_gates(self):
        result = {
            "status": "BLOCKED",
            "summary": "review evidence was rejected",
            "reviews": [{"kind": "fidelity", "status": "FAIL"}],
            "validation": {"status": "FAIL"},
        }
        with self.assertRaisesRegex(swarm.DagError, "review evidence was rejected; failed gates: fidelity, validation"):
            swarm.require_done(result)

    def test_parse_result_ignores_trailing_acceptance_report_json(self):
        result = orchestrated(sha="abc", review="PASS")
        acceptance = {"criteriaSatisfied": [{"status": "satisfied"}], "changedFiles": ["a"]}
        self.assertEqual(swarm.parse_result(f"{json.dumps(result)}\n{json.dumps(acceptance)}"), result)

    def test_invocation_uses_selected_source_prompt(self):
        current = unit("a")
        current.update(_retry_head="candidate", _retry_error="missing import")
        command, _ = swarm.invocation(current, Path("/sessions"), Path("/worktree"), Path("/selected-source"))
        self.assertIn("@/selected-source/.pi/prompts/pi-port-swarm.md", command)
        self.assertIn("acceptance:none", command[-1])
        self.assertIn("propagation owned by a later unit", command[-1])
        self.assertIn("starts at retained candidate candidate", command[-1])
        self.assertIn("check-owned a", command[-1])

    def test_owned_compile_errors_only_reports_primary_owned_spans(self):
        messages = [
            {"reason": "compiler-message", "message": {"level": "error", "message": "owned", "spans": [{"file_name": "src/a.rs", "line_start": 7, "is_primary": True}]}},
            {"reason": "compiler-message", "message": {"level": "error", "message": "downstream", "spans": [{"file_name": "src/b.rs", "line_start": 9, "is_primary": True}]}},
        ]
        original = swarm.run
        swarm.run = lambda *_args, **_kwargs: subprocess.CompletedProcess([], 101, "\n".join(json.dumps(item) for item in messages), "")
        try:
            self.assertEqual(swarm.owned_compile_errors(Path("/repo"), unit("a", ownership=["src/a.rs"])), ["src/a.rs:7: owned"])
        finally:
            swarm.run = original

    def test_dag_requires_ownership_field(self):
        bad = {"id": "a", "role": "writer", "model": "openai-codex/gpt-5.6-terra", "depends_on": []}
        with self.assertRaisesRegex(swarm.DagError, "ownership and dependencies are required"):
            swarm.validate_dag(dag([bad]))

    def test_reviewer_and_validator_require_expected_sha_without_commit(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp)
            base = repository(repo)
            subprocess.run(["git", "-C", repo, "branch", "automation/pi-port", base], check=True)
            self.assertTrue(swarm.accept_result(repo, unit("review", role="reviewer"), orchestrated(sha=base, review="PASS"), base))
            self.assertTrue(swarm.accept_result(repo, unit("validate", role="validator"), orchestrated(sha=base, validation={"status": "PASS", "sha": base, "run_id": "validator-1"}), base))
            with self.assertRaisesRegex(swarm.DagError, "no commit"):
                swarm.accept_result(repo, unit("review", role="reviewer"), orchestrated(sha=base, commit=base, review="PASS"), base)

    def test_blocked_owned_candidate_is_reused_as_repair_base(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp)
            base = repository(repo)
            (repo / "a").write_text("candidate")
            subprocess.run(["git", "-C", repo, "commit", "-qam", "candidate"], check=True)
            candidate = swarm.sha(repo)
            result = {"status": "BLOCKED", "commit": candidate, "sha": candidate}
            self.assertTrue(swarm.retryable_candidate(repo, unit("a", ownership=["a"]), result, base))
            self.assertFalse(swarm.retryable_candidate(repo, unit("a", ownership=["elsewhere"]), result, base))

    def test_writer_requires_independent_review_evidence(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp)
            base = repository(repo)
            subprocess.run(["git", "-C", repo, "branch", "automation/pi-port", base], check=True)
            (repo / "a").write_text("y")
            subprocess.run(["git", "-C", repo, "commit", "-qam", "candidate"], check=True)
            candidate = swarm.sha(repo)
            result = orchestrated(commit=candidate, sha=candidate, validation={"status": "PASS", "sha": candidate, "run_id": "v"})
            with self.assertRaisesRegex(swarm.DagError, "fidelity PASS evidence"):
                swarm.accept_result(repo, unit("write", ownership=["a"]), result, base)
            forged = orchestrated(
                commit=candidate,
                sha=candidate,
                reviews=[
                    {"kind": "fidelity", "status": "PASS", "sha": candidate, "run_id": "deadbeef"},
                    {"kind": "rust", "status": "PASS", "sha": candidate, "run_id": "cafebabe"},
                ],
                validation={"status": "PASS", "sha": candidate, "run_id": "feedface"},
            )
            with self.assertRaisesRegex(swarm.DagError, "parent-session evidence"):
                swarm.accept_result(repo, unit("write", ownership=["a"]), forged, base)

    def test_evidence_agents_use_compact_json_without_generic_acceptance(self):
        agents = Path(__file__).parents[2] / ".pi/agents"
        for name in ("pi-fidelity-reviewer.md", "pi-rust-reviewer.md", "pi-port-validator.md"):
            prompt = (agents / name).read_text()
            self.assertIn("Return exactly one JSON line", prompt)
            self.assertNotIn("acceptance-report", prompt)

    def test_child_artifact_uses_swarm_tmpdir(self):
        with tempfile.TemporaryDirectory() as tmp:
            old_tmp = swarm.SWARM_TMP
            swarm.SWARM_TMP = Path(tmp)
            try:
                artifact = Path(tmp) / "pi-subagents-uid-1000/async-subagent-runs/deadbeef"
                artifact.mkdir(parents=True)
                (artifact / "status.json").write_text(json.dumps({
                    "state": "complete",
                    "sessionId": "/parent.jsonl",
                    "steps": [{"agent": "pi-fidelity-reviewer"}],
                }))
                (artifact / "output-0.log").write_text(json.dumps({"status": "PASS", "sha": "abc"}))
                swarm.child_artifact("deadbeef", "abc", "pi-fidelity-reviewer", "/parent.jsonl")
                with self.assertRaisesRegex(swarm.DagError, "not bound to pi-rust-reviewer"):
                    swarm.child_artifact("deadbeef", "abc", "pi-rust-reviewer", "/parent.jsonl")
            finally:
                swarm.SWARM_TMP = old_tmp

    def test_resumed_evidence_run_is_bound_to_its_agent(self):
        with tempfile.TemporaryDirectory() as tmp:
            session = Path(tmp) / "session.jsonl"
            calls = [
                {"type": "toolCall", "id": "list", "name": "subagent", "arguments": {"action": "list"}},
                {"type": "toolCall", "id": "fid", "name": "subagent", "arguments": {"agent": "pi-fidelity-reviewer"}},
                {"type": "toolCall", "id": "resume", "name": "subagent", "arguments": {"action": "resume", "id": "oldfid00"}},
                {"type": "toolCall", "id": "rust", "name": "subagent", "arguments": {"agent": "pi-rust-reviewer"}},
                {"type": "toolCall", "id": "validator", "name": "subagent", "arguments": {"agent": "pi-port-validator"}},
                {"type": "toolCall", "id": "wait", "name": "wait", "arguments": {"all": True}},
            ]
            entries = [{"type": "message", "message": {"role": "assistant", "content": calls}}]
            tool_results = {
                "fid": "Async: pi-fidelity-reviewer [oldfid00]",
                "resume": "Revived run: newfid00\nAgent: pi-fidelity-reviewer",
                "rust": "Async: pi-rust-reviewer [rust0000]",
                "validator": "Async: pi-port-validator [valid000]",
            }
            entries.extend(
                {"type": "message", "message": {"role": "toolResult", "toolCallId": call_id, "content": [{"type": "text", "text": text}]}}
                for call_id, text in tool_results.items()
            )
            session.write_text("\n".join(json.dumps(entry) for entry in entries))
            result = orchestrated(
                reviews=[
                    {"kind": "fidelity", "status": "PASS", "sha": "abc", "run_id": "newfid00"},
                    {"kind": "rust", "status": "PASS", "sha": "abc", "run_id": "rust0000"},
                ],
                validation={"status": "PASS", "sha": "abc", "run_id": "valid000"},
            )
            original = swarm.child_artifact
            swarm.child_artifact = lambda *_: None
            try:
                swarm.verify_session_evidence(session, result, "abc")
            finally:
                swarm.child_artifact = original

    def test_batched_evidence_agents_are_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            session = Path(tmp) / "session.jsonl"
            entries = [
                {"type": "message", "message": {"role": "assistant", "content": [
                    {"type": "toolCall", "id": "list", "name": "subagent", "arguments": {"action": "list"}},
                    {"type": "toolCall", "id": "batch", "name": "subagent", "arguments": {"tasks": [
                        {"agent": "pi-fidelity-reviewer"}, {"agent": "pi-rust-reviewer"}
                    ]}},
                    {"type": "toolCall", "id": "wait", "name": "wait", "arguments": {"all": True}},
                ]}},
                {"type": "message", "message": {"role": "toolResult", "toolCallId": "batch", "content": [{"type": "text", "text": "deadbeef cafebabe"}]}},
            ]
            session.write_text("\n".join(json.dumps(entry) for entry in entries))
            result = {
                "status": "DONE",
                "orchestration": {"listed_agents": True, "waited_for_all": True},
                "reviews": [
                    {"kind": "fidelity", "status": "PASS", "sha": "abc", "run_id": "deadbeef"},
                    {"kind": "rust", "status": "PASS", "sha": "abc", "run_id": "cafebabe"},
                ],
                "validation": {"status": "PASS", "sha": "abc", "run_id": "feedface"},
            }
            with self.assertRaisesRegex(swarm.DagError, "separate subagent calls"):
                swarm.verify_session_evidence(session, result, "abc")

    def test_dirty_slot_is_retained_and_replaced(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            repo = root / "repo"
            repo.mkdir()
            base = repository(repo)
            slots = root / "worktrees"
            subprocess.run(["git", "-C", repo, "branch", "automation/pi-port", base], check=True)
            subprocess.run(["git", "-C", repo, "worktree", "add", "-q", "-b", "slot-1", slots / "slot-1", base], check=True)
            (slots / "slot-1" / "dirty").write_text("do not remove")
            old_data, old_slots, old_init = swarm.DATA, swarm.MAX_SLOTS, swarm.init_pi
            swarm.DATA, swarm.MAX_SLOTS, swarm.init_pi = root, 1, lambda *args: None
            try:
                state = {}
                replacement = swarm.prepare_slot(unit("a"), base, 1, repo, "unused", state)
                self.assertEqual(replacement, slots / "slot-2")
                self.assertIn(str(slots / "slot-1"), state["recovery"])
                self.assertTrue((slots / "slot-1" / "dirty").exists())
            finally:
                swarm.DATA, swarm.MAX_SLOTS, swarm.init_pi = old_data, old_slots, old_init

    def test_reconcile_retains_green_candidate_when_evidence_is_delayed(self):
        with tempfile.TemporaryDirectory() as tmp:
            state_path = Path(tmp) / "state.json"
            current = unit("a", ownership=["a"])
            result = orchestrated(commit="candidate", sha="candidate")
            state = {
                "units": {"a": {"status": "FAILED", "attempts": 0}},
                "pending_integration": {
                    "unit": current,
                    "result": result,
                    "expected_head": "base",
                    "parent_session": "/parent.jsonl",
                },
            }
            original_sha, original_accept = swarm.sha, swarm.accept_result
            swarm.sha = lambda *_: "base"
            swarm.accept_result = lambda *_: (_ for _ in ()).throw(swarm.DagError("parent session does not bind run to reviewer"))
            try:
                self.assertTrue(swarm.reconcile_pending(state, state_path))
                self.assertIn("pending_integration", state)
                self.assertEqual(state["pending_integration"]["retries"], 1)
                self.assertEqual(state["units"]["a"]["status"], "FAILED")
            finally:
                swarm.sha, swarm.accept_result = original_sha, original_accept

    def test_tick_immediately_runs_a_newly_ready_successor(self):
        with tempfile.TemporaryDirectory() as tmp:
            root, old_state = Path(tmp), swarm.STATE
            d = {"source_gitlink": "references/pi@pin", "units": [unit("a", ownership=["a"]), unit("b", ["a"], ownership=["b"])]}
            calls = []
            originals = {name: getattr(swarm, name) for name in ("STATE", "runtime_dag", "bootstrap", "pinned_pi", "sha", "prepare_slot", "execute_pi", "accept_result", "owned_compile_errors")}
            swarm.STATE = root / "state"
            swarm.runtime_dag = lambda _: d
            swarm.bootstrap = lambda *_: {"ok": True}
            swarm.pinned_pi = lambda _: "pin"
            swarm.sha = lambda *_: "base"
            swarm.prepare_slot = lambda unit, *_: root / unit["id"]
            swarm.execute_pi = lambda current, *_: (calls.append(current["id"]) or subprocess.CompletedProcess([], 0, json.dumps(orchestrated(commit="candidate", sha="candidate")), ""), None, 0.1)
            swarm.accept_result = lambda *_: True
            swarm.owned_compile_errors = lambda *_: []
            try:
                swarm.tick(type("Args", (), {"dag": root / "dag.json", "source": root})())
                self.assertEqual(calls, ["a", "b"])
            finally:
                for name, value in originals.items():
                    setattr(swarm, name, value)

    def test_parser_accepts_global_options_before_command(self):
        args = swarm.parser().parse_args(["--source", "/tmp/source", "--dag", "/tmp/dag.json", "status"])
        self.assertEqual(args.command, "status")
        self.assertEqual(args.source, Path("/tmp/source"))
        check = swarm.parser().parse_args(["--source", "/tmp/worktree", "check-owned", "AG-L1"])
        self.assertEqual((check.command, check.unit), ("check-owned", "AG-L1"))


if __name__ == "__main__":
    unittest.main()
