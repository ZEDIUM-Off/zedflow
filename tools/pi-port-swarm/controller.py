#!/usr/bin/env python3
"""Small, event-driven controller for the frozen Pi TypeScript → Rust port.

`run` accepts one unit; `run --continuous` chains units immediately. Runtime
state, worktrees, sessions, and logs stay outside the repository. No scheduler
is installed or invoked by this program.
"""
from __future__ import annotations

import argparse
import fcntl
import hashlib
import json
import os
import subprocess
import sys
import tempfile
import time
import uuid
from pathlib import Path
from typing import Any

KINDS = {"writer", "checkpoint", "validator", "reviewer"}
MUTATING_KINDS = {"writer", "checkpoint"}
CONTROL_OWNERSHIP = ("tools/pi-port-swarm/dag.json", ".agents/port-swarm/state.json", "docs/porting")


class ControllerError(RuntimeError):
    pass


def run_cmd(args: list[str], cwd: Path, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(args, cwd=cwd, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=check)


def git(source: Path, *args: str, check: bool = True) -> str:
    try:
        result = run_cmd(["git", *args], source, check)
    except subprocess.CalledProcessError as error:
        raise ControllerError(error.stderr.strip() or "git command failed") from error
    if check and result.returncode:
        raise ControllerError(result.stderr.strip() or "git command failed")
    return result.stdout.strip()


def git_ok(source: Path, *args: str) -> bool:
    return run_cmd(["git", *args], source, check=False).returncode == 0


def atomic_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile("w", dir=path.parent, prefix=".tmp-", delete=False, encoding="utf-8") as file:
        json.dump(value, file, indent=2, sort_keys=True)
        file.write("\n")
        temporary = Path(file.name)
    temporary.replace(path)


def load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ControllerError(f"cannot read {path}: {error}") from error
    if not isinstance(value, dict):
        raise ControllerError(f"{path} must contain an object")
    return value


def load_dag(source: Path, dag_path: Path, integration_ref: str) -> dict[str, Any]:
    """Read the integration DAG once that ref exists, otherwise the tracked seed."""
    if not git_ok(source, "show-ref", "--verify", "--quiet", integration_ref):
        return load_json(dag_path)
    try:
        relative = dag_path.relative_to(source).as_posix()
        return json.loads(git(source, "show", f"{integration_ref}:{relative}"))
    except (ValueError, json.JSONDecodeError) as error:
        raise ControllerError(f"cannot read integration DAG: {error}") from error


def dag_hash(dag: dict[str, Any]) -> str:
    return hashlib.sha256(json.dumps(dag, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def pinned_gitlink(dag: dict[str, Any]) -> str:
    pin = dag.get("source_gitlink")
    if not isinstance(pin, str) or not pin.startswith("references/pi@"):
        raise ControllerError("source_gitlink must be references/pi@<sha>")
    sha = pin.removeprefix("references/pi@")
    if len(sha) != 40 or any(char not in "0123456789abcdef" for char in sha):
        raise ControllerError("source_gitlink must use a lowercase 40-character SHA")
    return sha


def owns(ownership: list[str], path: str) -> bool:
    return any(path == prefix or path.startswith(prefix.rstrip("/") + "/") for prefix in ownership)


def validate_dag(dag: dict[str, Any]) -> list[dict[str, Any]]:
    if dag.get("version") != 2 or dag.get("max_active_writers") != 1:
        raise ControllerError("DAG must be version 2 with max_active_writers equal to 1")
    pinned_gitlink(dag)
    units = dag.get("units")
    if not isinstance(units, list) or not units:
        raise ControllerError("DAG units must be a non-empty list")
    by_id: dict[str, dict[str, Any]] = {}
    for unit in units:
        if not isinstance(unit, dict):
            raise ControllerError("DAG unit must be an object")
        identifier = unit.get("id")
        if not isinstance(identifier, str) or not identifier or identifier in by_id:
            raise ControllerError(f"duplicate or invalid unit ID: {identifier!r}")
        if unit.get("kind") not in KINDS:
            raise ControllerError(f"{identifier}: invalid kind")
        for field in ("depends_on", "ownership", "validation"):
            if not isinstance(unit.get(field), list) or not all(isinstance(value, str) for value in unit[field]):
                raise ControllerError(f"{identifier}: {field} must be a list of strings")
        if unit["kind"] in MUTATING_KINDS and not unit["ownership"]:
            raise ControllerError(f"{identifier}: mutating unit requires ownership")
        for prefix in unit["ownership"]:
            if not prefix or Path(prefix).is_absolute() or ".." in Path(prefix).parts:
                raise ControllerError(f"{identifier}: unsafe ownership prefix {prefix!r}")
        by_id[identifier] = unit
    for identifier, unit in by_id.items():
        for dependency in unit["depends_on"]:
            if dependency not in by_id:
                raise ControllerError(f"{identifier}: unknown dependency {dependency}")
    visiting: set[str] = set()
    visited: set[str] = set()
    def walk(identifier: str) -> None:
        if identifier in visiting:
            raise ControllerError(f"DAG cycle at {identifier}")
        if identifier not in visited:
            visiting.add(identifier)
            for dependency in by_id[identifier]["depends_on"]:
                walk(dependency)
            visiting.remove(identifier)
            visited.add(identifier)
    for identifier in by_id:
        walk(identifier)
    # Same files may be reused only when a dependency serializes the writers.
    def depends(left: str, right: str) -> bool:
        return left == right or any(depends(child, right) for child in by_id[left]["depends_on"])
    mutating = [unit for unit in units if unit["kind"] in MUTATING_KINDS]
    for index, left in enumerate(mutating):
        for right in mutating[index + 1:]:
            if set(left["ownership"]) & set(right["ownership"]) and not (depends(left["id"], right["id"]) or depends(right["id"], left["id"])):
                raise ControllerError(f"concurrent ownership conflict: {left['id']} and {right['id']}")
    return units


def verify_gitlink(source: Path, commit: str, pin: str) -> None:
    output = git(source, "ls-tree", commit, "references/pi")
    fields = output.split()
    if len(fields) < 3 or fields[2] != pin:
        raise ControllerError("frozen references/pi gitlink differs from DAG pin")


def seed_runtime(source: Path, dag: dict[str, Any], integration_ref: str) -> dict[str, Any]:
    seed = load_json(source / ".agents/port-swarm/state.json")
    closed = seed.get("closed")
    if not isinstance(closed, list) or not all(isinstance(value, str) for value in closed):
        raise ControllerError("tracked state closed must be a string list")
    ids = {unit["id"] for unit in validate_dag(dag)}
    if len(set(closed)) != len(closed) or any(identifier not in ids for identifier in closed):
        raise ControllerError("tracked state contains unknown or duplicate closed unit")
    current = seed.get("current", {})
    if not isinstance(current, dict) or current.get("id") not in ids or current["id"] in closed:
        raise ControllerError("tracked state current is invalid")
    base = current.get("base")
    if not isinstance(base, str) or len(base) != 40:
        raise ControllerError("tracked state current.base must be an immutable SHA")
    return {
        "version": 3,
        "integration_ref": integration_ref,
        "pi_gitlink": pinned_gitlink(dag),
        "dag_sha256": dag_hash(dag),
        "units": {identifier: {"status": "ACCEPTED"} for identifier in closed},
        "history": [],
    }


def load_runtime(source: Path, dag: dict[str, Any], state_path: Path, integration_ref: str, write: bool) -> dict[str, Any]:
    if state_path.exists():
        state = load_json(state_path)
        if state.get("version") != 3 or state.get("integration_ref") != integration_ref:
            raise ControllerError("runtime state does not match this controller integration ref")
        if state.get("pi_gitlink") != pinned_gitlink(dag):
            raise ControllerError("runtime state Pi gitlink differs from DAG")
        return state
    state = seed_runtime(source, dag, integration_ref)
    if write:
        atomic_json(state_path, state)
    return state


def ready_units(units: list[dict[str, Any]], state: dict[str, Any]) -> list[dict[str, Any]]:
    records = state.get("units", {})
    return [unit for unit in units if records.get(unit["id"], {}).get("status") not in {"ACCEPTED", "RUNNING", "FAILED", "BLOCKED", "SUPERSEDED"} and all(records.get(dep, {}).get("status") == "ACCEPTED" for dep in unit["depends_on"])]


def integration_sha(source: Path, ref: str) -> str:
    if git_ok(source, "show-ref", "--verify", "--quiet", ref):
        return git(source, "rev-parse", ref)
    return git(source, "rev-parse", "HEAD")


def ensure_integration_ref(source: Path, ref: str, base: str) -> None:
    if not git_ok(source, "show-ref", "--verify", "--quiet", ref):
        git(source, "update-ref", ref, base)


def result_line(stdout: str) -> dict[str, Any]:
    matches: list[dict[str, Any]] = []
    for line in stdout.splitlines():
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict) and value.get("status") in {"DONE", "BLOCKED", "PLAN_CHANGE"}:
            matches.append(value)
    if len(matches) != 1:
        raise ControllerError("Pi must emit exactly one structured final result line")
    return matches[0]


def pi_command(prompt: Path, session_dir: Path, name: str, message: str) -> list[str]:
    return ["pi", "-p", "--session-dir", str(session_dir), "--name", name, "--approve", f"@{prompt}", message]


def allowed_changes(source: Path, base: str, candidate: str, ownership: list[str]) -> list[str]:
    changed = [line for line in git(source, "diff", "--name-only", f"{base}..{candidate}").splitlines() if line]
    if not changed:
        raise ControllerError("candidate has an empty diff")
    outside = [path for path in changed if not owns(ownership, path)]
    if outside:
        raise ControllerError(f"candidate changes outside ownership: {', '.join(outside)}")
    return changed


def validate_candidate(source: Path, worktree: Path, base: str, candidate: str, unit: dict[str, Any], pin: str) -> None:
    if not git_ok(source, "merge-base", "--is-ancestor", base, candidate):
        raise ControllerError("candidate is not a descendant of its immutable base")
    allowed_changes(source, base, candidate, unit["ownership"])
    verify_gitlink(source, candidate, pin)
    for command in unit["validation"]:
        completed = subprocess.run(command, cwd=worktree, shell=True, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        if completed.returncode:
            raise ControllerError(f"declared validation failed: {command}\n{completed.stdout}{completed.stderr}")


def create_worktree(source: Path, state_dir: Path, base: str, unit: dict[str, Any], attempt: int, pin: str) -> tuple[Path, Path]:
    nonce = uuid.uuid4().hex[:12]
    worktree = state_dir / "worktrees" / f"{unit['id'].lower()}-{attempt}-{nonce}"
    branch = f"automation/pi-port/{unit['id'].lower()}-{attempt}-{nonce}"
    worktree.parent.mkdir(parents=True, exist_ok=True)
    git(source, "worktree", "add", "-b", branch, str(worktree), base)
    git(worktree, "-c", "protocol.file.allow=always", "submodule", "update", "--init", "--no-fetch", "references/pi")
    verify_gitlink(worktree, "HEAD", pin)
    return worktree, state_dir / "sessions" / f"{unit['id'].lower()}-{attempt}-{nonce}"


def launch(source: Path, worktree: Path, session_dir: Path, unit: dict[str, Any], base: str, coordinator: bool = False) -> dict[str, Any]:
    prompt = source / ".pi/prompts" / ("pi-port-coordinator.md" if coordinator else "pi-port-worker-session.md")
    capsule = {"unit": unit, "base": base, "ownership": unit["ownership"], "validation": unit["validation"], "intent": unit.get("intent", ""), "result_schema": {"status": "DONE|BLOCKED|PLAN_CHANGE", "unit": unit["id"], "base": base, "candidate": "40-hex SHA for DONE"}}
    session_dir.mkdir(parents=True, exist_ok=True)
    command = pi_command(prompt, session_dir, f"pi-port-{unit['id'].lower()}-{uuid.uuid4().hex[:8]}", json.dumps(capsule, separators=(",", ":")))
    completed = run_cmd(command, worktree, check=False)
    log = session_dir / "controller.log"
    log.write_text(completed.stdout + completed.stderr, encoding="utf-8")
    if completed.returncode:
        raise ControllerError(f"Pi exited {completed.returncode}; see {log}")
    return result_line(completed.stdout)


def accept_plan_change(source: Path, state_dir: Path, base: str, unit: dict[str, Any], result: dict[str, Any], pin: str, integration_ref: str) -> str:
    control = {"id": f"REPLAN-{unit['id']}", "kind": "checkpoint", "depends_on": [], "ownership": list(CONTROL_OWNERSHIP), "validation": ["python3 tools/pi-port-swarm/controller.py validate"], "intent": result.get("reason", "evidence-backed plan mutation")}
    worktree, session = create_worktree(source, state_dir, base, control, 1, pin)
    coordinator_result = launch(source, worktree, session, control, base, coordinator=True)
    candidate = coordinator_result.get("candidate")
    if coordinator_result.get("status") != "DONE" or not isinstance(candidate, str):
        raise ControllerError("coordinator did not return a plan-mutation candidate")
    validate_candidate(source, worktree, base, candidate, control, pin)
    revised = load_json(worktree / "tools/pi-port-swarm/dag.json")
    validate_dag(revised)
    if pinned_gitlink(revised) != pin:
        raise ControllerError("coordinator changed the frozen Pi gitlink")
    git(source, "update-ref", integration_ref, candidate, base)
    return candidate


def run_one(source: Path, dag: dict[str, Any], state: dict[str, Any], state_path: Path, integration_ref: str, state_dir: Path, requested: str | None) -> str:
    units = validate_dag(dag)
    ready = ready_units(units, state)
    if not ready:
        return "complete"
    unit = ready[0]
    if requested and requested != unit["id"]:
        raise ControllerError(f"requested {requested} is not the next ready unit ({unit['id']})")
    if git(source, "status", "--porcelain"):
        raise ControllerError("source worktree is dirty")
    pin = pinned_gitlink(dag)
    base = integration_sha(source, integration_ref)
    ensure_integration_ref(source, integration_ref, base)
    attempt = state.get("units", {}).get(unit["id"], {}).get("attempt", 0) + 1
    worktree, session = create_worktree(source, state_dir, base, unit, attempt, pin)
    record = {"status": "RUNNING", "attempt": attempt, "base": base, "dag_sha256": dag_hash(dag), "worktree": str(worktree), "session": str(session), "candidate": None, "blocker": None}
    state.setdefault("units", {})[unit["id"]] = record
    atomic_json(state_path, state)
    try:
        result = launch(source, worktree, session, unit, base)
        if result.get("unit") != unit["id"] or result.get("base") != base:
            raise ControllerError("result unit/base does not match dispatch capsule")
        if result["status"] == "BLOCKED":
            record.update(status="BLOCKED", blocker=result.get("blocker") or result.get("reason") or "worker blocked")
        elif result["status"] == "PLAN_CHANGE":
            record.update(status="BLOCKED", blocker=result.get("blocker") or result.get("reason") or "plan change requested")
            candidate = accept_plan_change(source, state_dir, base, unit, result, pin, integration_ref)
            record.update(status="SUPERSEDED", candidate=candidate, plan_change=result.get("reason"))
        else:
            candidate = result.get("candidate")
            if unit["kind"] in MUTATING_KINDS:
                if not isinstance(candidate, str):
                    raise ControllerError("mutating unit must return candidate SHA")
                validate_candidate(source, worktree, base, candidate, unit, pin)
                git(source, "update-ref", integration_ref, candidate, base)
                record.update(status="ACCEPTED", candidate=candidate)
            else:
                if candidate is not None or integration_sha(source, integration_ref) != base:
                    raise ControllerError("read-only unit must not change the integration ref")
                for command in unit["validation"]:
                    completed = subprocess.run(command, cwd=worktree, shell=True)
                    if completed.returncode:
                        raise ControllerError(f"declared validation failed: {command}")
                record.update(status="ACCEPTED")
        state.setdefault("history", []).append({"unit": unit["id"], "status": record["status"], "base": base, "candidate": record.get("candidate"), "at": int(time.time())})
        atomic_json(state_path, state)
        return record["status"].lower()
    except ControllerError as error:
        record.update(status="FAILED", blocker=str(error))
        atomic_json(state_path, state)
        raise


def progress(units: list[dict[str, Any]], state: dict[str, Any]) -> dict[str, Any]:
    records = state.get("units", {})
    by_package: dict[str, dict[str, int]] = {}
    for unit in units:
        package = next((path.split("/")[1] for path in unit["ownership"] if path.startswith("crates/") and len(path.split("/")) > 1), "control")
        bucket = by_package.setdefault(package, {"total": 0, "accepted": 0})
        bucket["total"] += 1
        bucket["accepted"] += records.get(unit["id"], {}).get("status") == "ACCEPTED"
    return {"units": {"total": len(units), "accepted": sum(record.get("status") == "ACCEPTED" for record in records.values())}, "packages": by_package}


def snapshot(source: Path, dag: dict[str, Any], state: dict[str, Any], integration_ref: str) -> dict[str, Any]:
    units = validate_dag(dag)
    return {"integration_sha": integration_sha(source, integration_ref), "dag_sha256": dag_hash(dag), "ready": [unit["id"] for unit in ready_units(units, state)], "units": state.get("units", {}), "progress": progress(units, state)}


def paths(args: argparse.Namespace) -> tuple[Path, Path, Path, Path]:
    source = Path(args.source).resolve()
    root = Path(git(source, "rev-parse", "--show-toplevel"))
    state_dir = Path(args.state_dir) if args.state_dir else Path(os.environ.get("XDG_STATE_HOME", Path.home() / ".local/state")) / "zedflow-pi-port"
    return root, root / args.dag, state_dir, state_dir / "state.json"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", default=".")
    parser.add_argument("--state-dir")
    parser.add_argument("--integration-ref", default="refs/heads/automation/pi-port")
    parser.add_argument("--dag", default="tools/pi-port-swarm/dag.json")
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("validate")
    sub.add_parser("status")
    sub.add_parser("monitor")
    run_parser = sub.add_parser("run")
    run_parser.add_argument("--unit")
    run_parser.add_argument("--continuous", action="store_true")
    args = parser.parse_args()
    try:
        source, dag_path, state_dir, state_path = paths(args)
        dag = load_dag(source, dag_path, args.integration_ref)
        units = validate_dag(dag)
        verify_gitlink(source, "HEAD", pinned_gitlink(dag))
        if args.command == "validate":
            load_runtime(source, dag, state_path, args.integration_ref, write=False)
            print(json.dumps({"status": "valid", "dag_sha256": dag_hash(dag), "pi_gitlink": pinned_gitlink(dag)}, sort_keys=True))
            return 0
        state = load_runtime(source, dag, state_path, args.integration_ref, write=False)
        if args.command in {"status", "monitor"}:
            print(json.dumps(snapshot(source, dag, state, args.integration_ref), sort_keys=True))
            return 0
        state_dir.mkdir(parents=True, exist_ok=True)
        with (state_dir / "controller.lock").open("w") as lock:
            try:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                return 75
            state = load_runtime(source, dag, state_path, args.integration_ref, write=True)
            while True:
                outcome = run_one(source, dag, state, state_path, args.integration_ref, state_dir, args.unit)
                if outcome in {"blocked", "failed", "superseded", "complete"} or not args.continuous:
                    print(json.dumps({"status": outcome, **snapshot(source, dag, state, args.integration_ref)}, sort_keys=True))
                    return 0 if outcome in {"accepted", "complete"} else 1
                dag = load_dag(source, dag_path, args.integration_ref)
                state = load_runtime(source, dag, state_path, args.integration_ref, write=False)
                args.unit = None
    except ControllerError as error:
        print(f"controller: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
