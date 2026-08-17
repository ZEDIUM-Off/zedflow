#!/usr/bin/env python3
"""THROWAWAY PROTOTYPE: controller contracts proposed for issue 18, not production.

Run: python3 tools/pi-port-swarm/controller-contract-prototype.py --self-check

This makes four proposed boundaries executable: evidence binds exact bytes and
SHAs; a single registry atomically leases every requested path; scope growth
waits for an explicit HITL approval; and shared files belong to integration
lots, never parallel writers.
"""
from __future__ import annotations

import argparse
import fcntl
import hashlib
import json
import multiprocessing
import tempfile
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def atomic_json(path: Path, value: dict[str, Any]) -> None:
    temporary = path.with_suffix(".tmp")
    temporary.write_text(json.dumps(value, sort_keys=True, indent=2) + "\n")
    temporary.replace(path)


def evidence(scenario: str, pi_sha: str, zedflow_sha: str, artifacts: list[Path]) -> dict[str, Any]:
    """Bind an actual run's declared artifacts; comparison stays outside this contract."""
    if not all(len(sha) == 40 and set(sha) <= set("0123456789abcdef") for sha in (pi_sha, zedflow_sha)):
        raise ValueError("evidence requires full lowercase Pi and Zedflow SHAs")
    return {
        "scenario": scenario,
        "pi_sha": pi_sha,
        "zedflow_sha": zedflow_sha,
        "artifacts": {path.name: sha256(path) for path in artifacts},
    }


class LeaseRegistry:
    """One lock protects one JSON registry; acquiring a set is all-or-nothing."""

    def __init__(self, directory: Path):
        self.path = directory / "leases.json"
        self.lock_path = directory / "leases.lock"
        directory.mkdir(parents=True, exist_ok=True)
        if not self.path.exists():
            atomic_json(self.path, {"leases": {}, "extensions": {}})

    def _update(self, operation):
        with self.lock_path.open("a+") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX)
            state = json.loads(self.path.read_text())
            result = operation(state)
            atomic_json(self.path, state)
            fcntl.flock(lock, fcntl.LOCK_UN)
            return result

    def acquire(self, owner: str, paths: list[str]) -> bool:
        paths = sorted(set(paths))
        if not paths:
            raise ValueError("a lease needs at least one file")

        def operation(state):
            leases = state["leases"]
            if any(path in leases and leases[path] != owner for path in paths):
                return False
            leases.update({path: owner for path in paths})
            return True

        return self._update(operation)

    def request_extension(self, owner: str, paths: list[str], request_id: str) -> None:
        """A request is not a lease; only a separate HITL approval can grant it."""
        paths = sorted(set(paths))

        def operation(state):
            if owner not in state["leases"].values():
                raise ValueError("only an active writer may request an extension")
            state["extensions"][request_id] = {"owner": owner, "paths": paths, "status": "PENDING_HITL"}

        self._update(operation)

    def approve_extension(self, request_id: str, approval: str) -> bool:
        if not approval:
            raise ValueError("scope extension needs recorded HITL approval")

        def operation(state):
            request = state["extensions"].get(request_id)
            if not request or request["status"] != "PENDING_HITL":
                return False
            paths, owner = request["paths"], request["owner"]
            if any(path in state["leases"] and state["leases"][path] != owner for path in paths):
                return False
            state["leases"].update({path: owner for path in paths})
            request.update(status="APPROVED", approval=approval)
            return True

        return self._update(operation)


def integration_lot(name: str, dependencies: list[str], shared_files: list[str]) -> dict[str, Any]:
    if not dependencies or not shared_files:
        raise ValueError("an integration lot needs producers and shared files")
    return {"id": name, "kind": "integration_lot", "depends_on": dependencies, "ownership": sorted(set(shared_files))}


def race_acquire(directory: str, owner: str, gate) -> None:
    gate.send("ready")
    gate.recv()
    gate.send((owner, LeaseRegistry(Path(directory)).acquire(owner, ["crates/shared.rs", "crates/shared-test.rs"])))


def self_check() -> None:
    # Keep prototype scratch state in this throwaway branch's worktree.
    with tempfile.TemporaryDirectory(prefix="zedflow-controller-contract-", dir=ROOT / ".pi") as temporary:
        directory = Path(temporary)
        pi_raw, zedflow_raw = directory / "pi.raw", directory / "zedflow.raw"
        pi_raw.write_bytes(b"pi terminal state")
        zedflow_raw.write_bytes(b"zedflow terminal state")
        record = evidence("real-tui", "a" * 40, "b" * 40, [pi_raw, zedflow_raw])
        assert record["artifacts"]["pi.raw"] == sha256(pi_raw)

        registry = LeaseRegistry(directory)
        context = multiprocessing.get_context("fork")
        gates = [context.Pipe() for _ in range(2)]
        workers = [context.Process(target=race_acquire, args=(str(directory), owner, child)) for owner, (_, child) in zip(("writer-a", "writer-b"), gates)]
        for worker in workers: worker.start()
        for parent, _ in gates: assert parent.recv() == "ready"
        for parent, _ in gates: parent.send("start")
        results = [parent.recv() for parent, _ in gates]
        for worker in workers: worker.join(timeout=5); assert worker.exitcode == 0
        assert sum(granted for _, granted in results) == 1, results

        winner = next(owner for owner, granted in results if granted)
        registry.request_extension(winner, ["crates/global.rs"], "scope-18")
        assert json.loads(registry.path.read_text())["extensions"]["scope-18"]["status"] == "PENDING_HITL"
        assert registry.approve_extension("scope-18", "issue-18 human approval")
        state = json.loads(registry.path.read_text())
        assert state["leases"]["crates/global.rs"] == winner

        lot = integration_lot("integrate-shared-config", ["writer-a", "writer-b"], ["crates/global.rs"])
        assert lot["kind"] == "integration_lot" and lot["ownership"] == ["crates/global.rs"]
    print("controller contract prototype: ok")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-check", action="store_true")
    args = parser.parse_args()
    if not args.self_check:
        parser.error("this throwaway prototype only supports --self-check")
    self_check()


if __name__ == "__main__":
    main()
