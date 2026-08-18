#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import json
import multiprocessing
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("controller_contracts", ROOT / "tools/pi-port-swarm/controller_contracts.py")
assert SPEC and SPEC.loader
contracts = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(contracts)


def race(directory: str, unit: str, gate) -> None:
    gate.send("ready")
    gate.recv()
    gate.send(contracts.LeaseRegistry(Path(directory)).acquire(unit, ["crates/shared", "Cargo.toml"]))


class ControllerContractTests(unittest.TestCase):
    def test_atomic_all_path_race_and_audited_release(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            context = multiprocessing.get_context("fork")
            pipes = [context.Pipe() for _ in range(2)]
            workers = [context.Process(target=race, args=(temporary, unit, child)) for unit, (_, child) in zip(("A", "B"), pipes)]
            for worker in workers:
                worker.start()
            for parent, _ in pipes:
                self.assertEqual(parent.recv(), "ready")
            for parent, _ in pipes:
                parent.send("go")
            results = [parent.recv() for parent, _ in pipes]
            for worker in workers:
                worker.join(5)
                self.assertEqual(worker.exitcode, 0)
            self.assertEqual(sum(result is not None for result in results), 1)
            token = next(result for result in results if result)
            registry = contracts.LeaseRegistry(Path(temporary))
            self.assertTrue(registry.release(token, outcome="accepted"))
            self.assertEqual([event["event"] for event in json.loads(registry.path.read_text())["audit"]][-1], "RELEASED")

    def test_prefix_overlap_and_expiry_recovery_are_audited(self) -> None:
        now = [100]
        with tempfile.TemporaryDirectory() as temporary:
            registry = contracts.LeaseRegistry(Path(temporary), clock=lambda: now[0])
            token = registry.acquire("A", ["crates/a"], ttl=5)
            self.assertIsNotNone(token)
            self.assertIsNone(registry.acquire("B", ["crates/a/src/lib.rs"]))
            now[0] = 106
            replacement = registry.acquire("B", ["crates/a/src/lib.rs"])
            self.assertIsNotNone(replacement)
            events = [event["event"] for event in json.loads(registry.path.read_text())["audit"]]
            self.assertIn("EXPIRED", events)
            self.assertIn("ACQUIRED", events)

    def test_scope_needs_exact_github_review_binding_and_live_lease(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            evidence = directory / "evidence.json"
            evidence.write_text('{"exact":true}\n')
            registry = contracts.LeaseRegistry(directory)
            token = registry.acquire("U1", ["crates/u1.rs"])
            assert token
            request_id = registry.request_extension(token, "U1", ["crates/extra.rs"], evidence)
            state = json.loads(registry.path.read_text())
            request = state["extensions"][request_id]
            binding = {"request_id": request_id, "unit": "U1", "evidence_sha256": request["evidence_sha256"], "paths": ["crates/extra.rs"]}
            approval = {"id": 42, "node_id": "PRR_42", "html_url": "https://github.com/acme/repo/pull/1#pullrequestreview-42", "commit_id": "a" * 40, "submitted_at": "2026-01-01T00:00:00Z", "state": "APPROVED", "body": json.dumps(binding)}
            self.assertTrue(registry.approve_extension(request_id, approval))
            state = json.loads(registry.path.read_text())
            self.assertEqual(state["extensions"][request_id]["status"], "APPROVED")
            self.assertIn("crates/extra.rs", state["leases"][token]["paths"])

    def test_scope_rejects_mutated_evidence_binding(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            evidence = directory / "evidence"
            evidence.write_text("proof")
            registry = contracts.LeaseRegistry(directory)
            token = registry.acquire("U", ["owned"])
            assert token
            request_id = registry.request_extension(token, "U", ["extra"], evidence)
            approval = {"id": 1, "node_id": "PRR_1", "html_url": "https://github/review", "commit_id": "b" * 40, "submitted_at": "now", "state": "APPROVED", "body": json.dumps({"request_id": request_id, "unit": "U", "evidence_sha256": "0" * 64, "paths": ["extra"]})}
            with self.assertRaisesRegex(ValueError, "not bound"):
                registry.approve_extension(request_id, approval)


if __name__ == "__main__":
    unittest.main()
