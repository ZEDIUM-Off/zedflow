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
            with self.assertRaisesRegex(swarm.DagError, "only a structured DONE"):
                swarm.accept_result(repo, unit("a", ownership=["a"]), {"status": "CLOSED"}, base)

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

    def test_parser_accepts_global_options_before_command(self):
        args = swarm.parser().parse_args(["--source", "/tmp/source", "--dag", "/tmp/dag.json", "status"])
        self.assertEqual(args.command, "status")
        self.assertEqual(args.source, Path("/tmp/source"))


if __name__ == "__main__":
    unittest.main()
