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
INTEGRATION_REF = "refs/heads/automation/pi-port"
NULL_OID = "0" * 40
DAG_FILE = "tools/pi-port-swarm/dag.json"
CONTROL_OWNERSHIP = (DAG_FILE, ".agents/port-swarm/state.json", "docs/porting")
PROMPTS = {
    "writer": "pi-port-worker-session.md",
    "checkpoint": "pi-port-checkpoint.md",
    "validator": "pi-port-validator.md",
    "reviewer": "pi-port-reviewer.md",
}


class ControllerError(RuntimeError):
    pass


def run_cmd(args: list[str], cwd: Path, check: bool = True) -> subprocess.CompletedProcess[str]:
    try:
        return subprocess.run(args, cwd=cwd, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=check)
    except OSError as error:
        raise ControllerError(f"failed to run {args[0]}: {error}") from error


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
    require_integration_ref(integration_ref)
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


def require_integration_ref(ref: str) -> None:
    if ref != INTEGRATION_REF:
        raise ControllerError(f"integration ref must be {INTEGRATION_REF}")


def clean_worktree(worktree: Path) -> None:
    if git(worktree, "status", "--porcelain", "--untracked-files=all"):
        raise ControllerError(f"worktree is dirty: {worktree}")


def verify_gitlink(source: Path, commit: str, pin: str) -> None:
    fields = git(source, "ls-tree", commit, "references/pi").split()
    if len(fields) < 4 or fields[0] != "160000" or fields[1] != "commit" or fields[2] != pin or fields[3] != "references/pi":
        raise ControllerError("references/pi must be a mode-160000 commit gitlink at the frozen pin")


def verify_candidate_worktree(worktree: Path, candidate: str, pin: str) -> None:
    if git(worktree, "rev-parse", "HEAD") != candidate:
        raise ControllerError("candidate SHA does not equal the worktree HEAD")
    clean_worktree(worktree)
    submodule = worktree / "references/pi"
    if not submodule.is_dir() or git(submodule, "rev-parse", "HEAD") != pin:
        raise ControllerError("checked-out references/pi HEAD differs from frozen pin")
    clean_worktree(submodule)


def seed_runtime(source: Path, dag: dict[str, Any], integration_ref: str = INTEGRATION_REF) -> dict[str, Any]:
    require_integration_ref(integration_ref)
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
    if not isinstance(base, str) or len(base) != 40 or not git_ok(source, "merge-base", "--is-ancestor", base, "HEAD"):
        raise ControllerError("tracked state current.base must be an immutable ancestor of the controller baseline")
    return {
        "version": 3,
        "integration_ref": integration_ref,
        "pi_gitlink": pinned_gitlink(dag),
        "dag_sha256": dag_hash(dag),
        "units": {identifier: {"status": "ACCEPTED"} for identifier in closed},
        "history": [],
    }


def pending_dag_change(state: dict[str, Any]) -> bool:
    return any(
        record.get("status") == "ACCEPTING" and record.get("candidate") and record.get("revised_dag_sha256")
        for record in state.get("units", {}).values()
    )


def recover_accepted_checkpoint_dag(source: Path, dag: dict[str, Any], state: dict[str, Any], integration_ref: str) -> bool:
    """Recover state written by the pre-transactional checkpoint implementation."""
    revised_hash = dag_hash(dag)
    if state.get("dag_sha256") == revised_hash:
        return False
    ref = integration_sha(source, integration_ref)
    matches = []
    for identifier, record in state.get("units", {}).items():
        base = record.get("base")
        if record.get("status") != "ACCEPTED" or record.get("candidate") != ref or record.get("dag_sha256") != state.get("dag_sha256") or not base:
            continue
        try:
            previous = json.loads(git(source, "show", f"{base}:{DAG_FILE}"))
        except (ControllerError, json.JSONDecodeError):
            continue
        kinds = {unit["id"]: unit["kind"] for unit in validate_dag(previous)}
        if kinds.get(identifier) == "checkpoint" and not git_ok(source, "diff", "--quiet", f"{base}..{ref}", "--", DAG_FILE):
            matches.append(record)
    if len(matches) != 1:
        return False
    matches[0]["revised_dag_sha256"] = revised_hash
    state["dag_sha256"] = revised_hash
    return True


def load_runtime(source: Path, dag: dict[str, Any], state_path: Path, integration_ref: str = INTEGRATION_REF, write: bool = False) -> dict[str, Any]:
    require_integration_ref(integration_ref)
    if state_path.exists():
        state = load_json(state_path)
        if state.get("version") != 3 or state.get("integration_ref") != integration_ref:
            raise ControllerError("runtime state does not match this controller integration ref")
        if state.get("pi_gitlink") != pinned_gitlink(dag):
            raise ControllerError("runtime state Pi gitlink differs from DAG")
        recovered = False
        if state.get("dag_sha256") != dag_hash(dag) and not pending_dag_change(state):
            recovered = recover_accepted_checkpoint_dag(source, dag, state, integration_ref)
            if not recovered:
                raise ControllerError("runtime state DAG revision differs from the integration DAG")
        if recovered and write:
            atomic_json(state_path, state)
        return state
    state = seed_runtime(source, dag, integration_ref)
    if write:
        atomic_json(state_path, state)
    return state


def reconcile_runtime(source: Path, dag: dict[str, Any], state: dict[str, Any], integration_ref: str) -> bool:
    """Resolve interrupted acceptance without guessing or retrying an agent."""
    changed = False
    ref = integration_sha(source, integration_ref)
    for record in state.get("units", {}).values():
        if record.get("status") not in {"RUNNING", "ACCEPTING"}:
            continue
        base, candidate = record.get("base"), record.get("candidate")
        if record.get("status") == "ACCEPTING" and candidate and ref == candidate:
            if record.get("plan_change"):
                record["status"] = "RETRY" if record.get("replan_retry") else "SUPERSEDED"
            else:
                record["status"] = "ACCEPTED"
            if record.get("revised_dag_sha256"):
                state["dag_sha256"] = record["revised_dag_sha256"]
            record["blocker"] = None
        elif ref == base:
            record["status"] = "FAILED"
            record["blocker"] = "interrupted before CAS; use retry --unit <id>"
            if record.get("revised_dag_sha256"):
                state["dag_sha256"] = record.get("dag_sha256", dag_hash(dag))
        else:
            record["status"] = "FAILED"
            record["blocker"] = "integration ref changed while recovering interrupted unit"
        changed = True
    if state.get("dag_sha256") != dag_hash(dag) and not pending_dag_change(state):
        raise ControllerError("runtime state DAG revision differs from the integration DAG")
    return changed


def ready_units(units: list[dict[str, Any]], state: dict[str, Any]) -> list[dict[str, Any]]:
    records = state.get("units", {})
    terminal = {"ACCEPTED", "RUNNING", "ACCEPTING", "FAILED", "BLOCKED", "SUPERSEDED"}
    return [unit for unit in units if records.get(unit["id"], {}).get("status") not in terminal and all(records.get(dep, {}).get("status") == "ACCEPTED" for dep in unit["depends_on"])]


def graph_complete(units: list[dict[str, Any]], state: dict[str, Any]) -> bool:
    return all(state.get("units", {}).get(unit["id"], {}).get("status") in {"ACCEPTED", "SUPERSEDED"} for unit in units)


def integration_sha(source: Path, ref: str = INTEGRATION_REF) -> str:
    require_integration_ref(ref)
    if git_ok(source, "show-ref", "--verify", "--quiet", ref):
        return git(source, "rev-parse", ref)
    return git(source, "rev-parse", "HEAD")


def ensure_integration_ref(source: Path, ref: str, base: str) -> None:
    require_integration_ref(ref)
    if not git_ok(source, "show-ref", "--verify", "--quiet", ref):
        # Creation is a CAS from the null OID: never overwrite a racing ref.
        git(source, "update-ref", ref, base, NULL_OID)


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


def prompt_for(source: Path, unit: dict[str, Any], coordinator: bool = False) -> Path:
    if coordinator:
        return source / ".pi/prompts/pi-port-coordinator.md"
    return source / ".pi/prompts" / PROMPTS[unit["kind"]]


def is_full_sha(value: Any) -> bool:
    return isinstance(value, str) and len(value) == 40 and all(char in "0123456789abcdef" for char in value)


def result_schema(unit: dict[str, Any]) -> dict[str, str]:
    base = {"unit": unit["id"], "base": "40-hex"}
    if unit["kind"] in MUTATING_KINDS:
        return {"status": "DONE|BLOCKED|PLAN_CHANGE", **base, "candidate": "40-hex for DONE"}
    statuses = "DONE|BLOCKED|PLAN_CHANGE" if unit["kind"] == "reviewer" else "DONE|BLOCKED"
    return {"status": statuses, **base, "candidate": "absent"}


def validate_result(unit: dict[str, Any], result: dict[str, Any], base: str) -> None:
    if result.get("unit") != unit["id"] or result.get("base") != base:
        raise ControllerError("result unit/base does not match dispatch capsule")
    status, candidate = result.get("status"), result.get("candidate")
    if status not in {"DONE", "BLOCKED", "PLAN_CHANGE"}:
        raise ControllerError("result has an invalid status")
    if status == "PLAN_CHANGE" and unit["kind"] not in MUTATING_KINDS | {"reviewer"}:
        raise ControllerError("this unit kind cannot request a plan change")
    if status == "DONE" and unit["kind"] in MUTATING_KINDS:
        if not is_full_sha(candidate):
            raise ControllerError("mutating unit must return a full candidate SHA")
    elif candidate is not None:
        raise ControllerError("non-mutating result must not return a candidate")


def blocked_reason(result: dict[str, Any]) -> str:
    return str(result.get("blocker") or result.get("reason") or result.get("summary") or "worker blocked")


def allowed_changes(source: Path, base: str, candidate: str, ownership: list[str]) -> list[str]:
    changed = [line for line in git(source, "diff", "--name-only", f"{base}..{candidate}").splitlines() if line]
    if not changed:
        raise ControllerError("candidate has an empty diff")
    outside = [path for path in changed if not owns(ownership, path)]
    if outside:
        raise ControllerError(f"candidate changes outside ownership: {', '.join(outside)}")
    return changed


def validate_candidate(source: Path, worktree: Path, base: str, candidate: str, unit: dict[str, Any], pin: str) -> None:
    if not is_full_sha(candidate):
        raise ControllerError("candidate must be a full SHA")
    if not git_ok(source, "merge-base", "--is-ancestor", base, candidate):
        raise ControllerError("candidate is not a descendant of its immutable base")
    verify_candidate_worktree(worktree, candidate, pin)
    allowed_changes(source, base, candidate, unit["ownership"])
    verify_gitlink(source, candidate, pin)
    for command in unit["validation"]:
        completed = subprocess.run(command, cwd=worktree, shell=True, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        if completed.returncode:
            raise ControllerError(f"declared validation failed: {command}\n{completed.stdout}{completed.stderr}")


def create_worktree(source: Path, state_dir: Path, base: str, unit: dict[str, Any], attempt: int, pin: str) -> tuple[Path, Path]:
    nonce = uuid.uuid4().hex[:12]
    worktree = state_dir / "worktrees" / f"{unit['id'].lower()}-{attempt}-{nonce}"
    branch = f"automation/pi-port-unit/{unit['id'].lower()}-{attempt}-{nonce}"
    worktree.parent.mkdir(parents=True, exist_ok=True)
    git(source, "worktree", "add", "-b", branch, str(worktree), base)
    git(worktree, "config", "submodule.references/pi.url", str(source / "references/pi"))
    git(worktree, "-c", "protocol.file.allow=always", "submodule", "update", "--init", "--no-fetch", "references/pi")
    verify_gitlink(worktree, "HEAD", pin)
    return worktree, state_dir / "sessions" / f"{unit['id'].lower()}-{attempt}-{nonce}"


def launch(source: Path, worktree: Path, session_dir: Path, unit: dict[str, Any], base: str, coordinator: bool = False) -> dict[str, Any]:
    prompt = prompt_for(source, unit, coordinator)
    capsule = {"unit": unit, "base": base, "ownership": unit["ownership"], "validation": unit["validation"], "intent": unit.get("intent", ""), "result_schema": result_schema(unit)}
    session_dir.mkdir(parents=True, exist_ok=True)
    command = pi_command(prompt, session_dir, f"pi-port-{unit['id'].lower()}-{uuid.uuid4().hex[:8]}", json.dumps(capsule, separators=(",", ":")))
    completed = run_cmd(command, worktree, check=False)
    log = session_dir / "controller.log"
    log.write_text(completed.stdout + completed.stderr, encoding="utf-8")
    if completed.returncode:
        raise ControllerError(f"Pi exited {completed.returncode}; see {log}")
    return result_line(completed.stdout)


def mark_plan_acceptance(state: dict[str, Any], unit_id: str, candidate: str, reason: Any, revised_hash: str, replan_retry: bool) -> None:
    state.setdefault("units", {}).setdefault(unit_id, {}).update(
        status="ACCEPTING",
        candidate=candidate,
        plan_change=reason,
        replan_retry=replan_retry,
        revised_dag_sha256=revised_hash,
    )
    state["dag_sha256"] = revised_hash


def validate_replan_transition(source_unit: dict[str, Any], original_units: list[dict[str, Any]], revised_units: list[dict[str, Any]]) -> None:
    if source_unit["kind"] != "validator":
        return
    source_id = source_unit["id"]
    original_ids = {unit["id"] for unit in original_units}
    revised = {unit["id"]: unit for unit in revised_units}
    if source_id in revised:
        raise ControllerError("validator replan must supersede the blocked validator")
    downstream = [unit for unit in original_units if source_id in unit["depends_on"]]
    replacements = [
        unit for unit in revised_units
        if unit["id"] not in original_ids
        and unit["kind"] == "validator"
        and unit["validation"] == source_unit["validation"]
        and any(revised.get(dependency, {}).get("kind") == "writer" and dependency not in original_ids for dependency in unit["depends_on"])
    ]
    if len(replacements) != 1:
        raise ControllerError("validator replan requires one fresh equivalent validator after a new repair writer")
    replacement = replacements[0]["id"]
    for unit in downstream:
        required = (set(unit["depends_on"]) - {source_id}) | {replacement}
        if unit["id"] not in revised or not required.issubset(revised[unit["id"]]["depends_on"]):
            raise ControllerError("validator replan must preserve and reconnect direct downstream units")


def accept_plan_change(source: Path, state_dir: Path, base: str, unit: dict[str, Any], result: dict[str, Any], pin: str, integration_ref: str, state: dict[str, Any], state_path: Path) -> tuple[str, bool]:
    require_integration_ref(integration_ref)
    control = {
        "id": f"REPLAN-{unit['id']}",
        "kind": "checkpoint",
        "depends_on": [],
        "ownership": list(CONTROL_OWNERSHIP),
        "validation": ["python3 tools/pi-port-swarm/controller.py validate"],
        "intent": result.get("reason", "evidence-backed plan mutation"),
        "source_unit": unit,
    }
    worktree, session = create_worktree(source, state_dir, base, control, 1, pin)
    coordinator_result = launch(source, worktree, session, control, base, coordinator=True)
    validate_result(control, coordinator_result, base)
    candidate = coordinator_result.get("candidate")
    if coordinator_result.get("status") != "DONE":
        raise ControllerError("coordinator did not return a plan-mutation candidate")
    validate_candidate(source, worktree, base, candidate, control, pin)
    if git_ok(source, "diff", "--quiet", f"{base}..{candidate}", "--", "tools/pi-port-swarm/dag.json"):
        raise ControllerError("PLAN_CHANGE coordinator did not modify dag.json")
    revised = load_json(worktree / "tools/pi-port-swarm/dag.json")
    revised_units = validate_dag(revised)
    original_units = validate_dag(json.loads(git(source, "show", f"{base}:{DAG_FILE}")))
    validate_replan_transition(unit, original_units, revised_units)
    if pinned_gitlink(revised) != pin:
        raise ControllerError("coordinator changed the frozen Pi gitlink")
    revised_hash = dag_hash(revised)
    replan_retry = any(revised_unit["id"] == unit["id"] for revised_unit in revised_units)
    prospective = json.loads(json.dumps(state))
    prospective.setdefault("units", {})[unit["id"]] = {"status": "RETRY" if replan_retry else "SUPERSEDED"}
    if not ready_units(revised_units, prospective) and not graph_complete(revised_units, prospective):
        raise ControllerError("PLAN_CHANGE leaves no reachable replacement or ready unit")
    mark_plan_acceptance(state, unit["id"], candidate, result.get("reason"), revised_hash, replan_retry)
    atomic_json(state_path, state)
    git(source, "update-ref", integration_ref, candidate, base)
    return candidate, replan_retry


def run_one(source: Path, dag: dict[str, Any], state: dict[str, Any], state_path: Path, integration_ref: str, state_dir: Path, requested: str | None) -> str:
    require_integration_ref(integration_ref)
    units = validate_dag(dag)
    ready = ready_units(units, state)
    if not ready:
        return "complete" if graph_complete(units, state) else "blocked"
    unit = ready[0]
    if requested and requested != unit["id"]:
        raise ControllerError(f"requested {requested} is not the next ready unit ({unit['id']})")
    clean_worktree(source)
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
        validate_result(unit, result, base)
        if result["status"] == "BLOCKED":
            record.update(status="BLOCKED", blocker=blocked_reason(result))
        elif result["status"] == "PLAN_CHANGE":
            record.update(status="BLOCKED", blocker=result.get("blocker") or result.get("reason") or "plan change requested")
            atomic_json(state_path, state)
            candidate, replan_retry = accept_plan_change(source, state_dir, base, unit, result, pin, integration_ref, state, state_path)
            record.update(status="RETRY" if replan_retry else "SUPERSEDED", candidate=candidate, plan_change=result.get("reason"), replan_retry=replan_retry)
        else:
            candidate = result.get("candidate")
            if unit["kind"] in MUTATING_KINDS:
                validate_candidate(source, worktree, base, candidate, unit, pin)
                revised_hash = None
                if unit["kind"] == "checkpoint" and not git_ok(source, "diff", "--quiet", f"{base}..{candidate}", "--", DAG_FILE):
                    revised = load_json(worktree / DAG_FILE)
                    revised_units = validate_dag(revised)
                    if pinned_gitlink(revised) != pin:
                        raise ControllerError("checkpoint changed the frozen Pi gitlink")
                    prospective = json.loads(json.dumps(state))
                    prospective.setdefault("units", {})[unit["id"]] = {"status": "ACCEPTED"}
                    if not ready_units(revised_units, prospective) and not graph_complete(revised_units, prospective):
                        raise ControllerError("checkpoint DAG change leaves no ready unit")
                    revised_hash = dag_hash(revised)
                # Persist the exact candidate before CAS so startup can recover a crash.
                record.update(status="ACCEPTING", candidate=candidate)
                if revised_hash:
                    record["revised_dag_sha256"] = revised_hash
                atomic_json(state_path, state)
                git(source, "update-ref", integration_ref, candidate, base)
                record.update(status="ACCEPTED", candidate=candidate)
                if revised_hash:
                    state["dag_sha256"] = revised_hash
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
        return "replanned" if record.get("plan_change") else record["status"].lower()
    except ControllerError as error:
        record.update(status="FAILED", blocker=str(error))
        atomic_json(state_path, state)
        raise


def progress(units: list[dict[str, Any]], state: dict[str, Any]) -> dict[str, Any]:
    records = state.get("units", {})
    by_id = {unit["id"]: unit for unit in units}
    done = [unit["id"] for unit in units if records.get(unit["id"], {}).get("status") == "ACCEPTED"]
    remaining = [unit["id"] for unit in units if unit["id"] not in done and records.get(unit["id"], {}).get("status") != "SUPERSEDED"]
    failed = {identifier for identifier, record in records.items() if identifier in by_id and record.get("status") in {"FAILED", "BLOCKED", "RUNNING", "ACCEPTING"}}

    def depends_on(identifier: str, dependency: str) -> bool:
        return dependency in by_id[identifier]["depends_on"] or any(depends_on(child, dependency) for child in by_id[identifier]["depends_on"])

    blocking = {identifier for identifier in failed if any(other != identifier and depends_on(other, identifier) for other in remaining)}
    visible = blocking or failed
    blockers = {identifier: records[identifier].get("blocker") for identifier in visible}
    return {"label": "DAG execution progress, not Pi fidelity completion", "total": len(units), "done": done, "remaining": remaining, "blockers": blockers}


def mechanical_manifest_progress(source: Path, integration_ref: str) -> dict[str, Any]:
    """Cheap mapping inventory; target presence is not semantic port completion."""
    manifests = source / ".agents/port-manifests"
    packages: dict[str, dict[str, Any]] = {}
    revision = integration_sha(source, integration_ref)
    for manifest in sorted(manifests.glob("*.tsv")):
        name, _, kind = manifest.stem.rpartition("-")
        if kind not in {"src", "tests"}:
            continue
        rows = [line.split("\t") for line in manifest.read_text(encoding="utf-8").splitlines() if line]
        present = sum(git_ok(source, "cat-file", "-e", f"{revision}:{row[1]}") for row in rows if len(row) >= 2)
        package = packages.setdefault(name, {"source_rows": 0, "source_targets_present": 0, "test_rows": 0, "test_targets_present": 0})
        field = "source" if kind == "src" else "test"
        package[f"{field}_rows"] = len(rows)
        package[f"{field}_targets_present"] = present
    ignored = len(git(source, "grep", "-l", "#\\[ignore", revision, "--", "crates", check=False).splitlines())
    placeholders = len(git(source, "grep", "-l", "PORT PLACEHOLDER", revision, "--", "crates", check=False).splitlines())
    return {"label": "mechanical mapping/target presence only; not fidelity completion", "packages": packages, "ignored_test_files": ignored, "port_placeholder_files": placeholders}


def snapshot(source: Path, dag: dict[str, Any], state: dict[str, Any], integration_ref: str) -> dict[str, Any]:
    require_integration_ref(integration_ref)
    units = validate_dag(dag)
    return {"integration_sha": integration_sha(source, integration_ref), "dag_sha256": dag_hash(dag), "current": next((identifier for identifier, record in state.get("units", {}).items() if record.get("status") in {"RUNNING", "ACCEPTING"}), None), "ready": [unit["id"] for unit in ready_units(units, state)], "units": state.get("units", {}), "dag_progress": progress(units, state), "mechanical_port_inventory": mechanical_manifest_progress(source, integration_ref)}


def should_continue(outcome: str, continuous: bool) -> bool:
    return continuous and outcome in {"accepted", "replanned"}


def paths(args: argparse.Namespace) -> tuple[Path, Path, Path, Path]:
    source = Path(args.source).resolve()
    root = Path(git(source, "rev-parse", "--show-toplevel"))
    state_dir = Path(args.state_dir) if args.state_dir else Path(os.environ.get("XDG_STATE_HOME", Path.home() / ".local/state")) / "zedflow-pi-port"
    return root, root / args.dag, state_dir, state_dir / "state.json"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", default=".")
    parser.add_argument("--state-dir")
    parser.add_argument("--dag", default="tools/pi-port-swarm/dag.json")
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("validate")
    sub.add_parser("status")
    sub.add_parser("monitor")
    run_parser = sub.add_parser("run")
    run_parser.add_argument("--unit")
    run_parser.add_argument("--continuous", action="store_true")
    retry_parser = sub.add_parser("retry")
    retry_parser.add_argument("--unit", required=True)
    replan_parser = sub.add_parser("replan")
    replan_parser.add_argument("--unit", required=True)
    replan_parser.add_argument("--reason", required=True)
    args = parser.parse_args()
    try:
        source, dag_path, state_dir, state_path = paths(args)
        dag = load_dag(source, dag_path, INTEGRATION_REF)
        units = validate_dag(dag)
        verify_gitlink(source, "HEAD", pinned_gitlink(dag))
        if args.command == "validate":
            load_runtime(source, dag, state_path, INTEGRATION_REF, write=False)
            print(json.dumps({"status": "valid", "dag_sha256": dag_hash(dag), "pi_gitlink": pinned_gitlink(dag)}, sort_keys=True))
            return 0
        state = load_runtime(source, dag, state_path, INTEGRATION_REF, write=False)
        if args.command in {"status", "monitor"}:
            print(json.dumps(snapshot(source, dag, state, INTEGRATION_REF), sort_keys=True))
            return 0
        state_dir.mkdir(parents=True, exist_ok=True)
        with (state_dir / "controller.lock").open("w") as lock:
            try:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                return 75
            state = load_runtime(source, dag, state_path, INTEGRATION_REF, write=True)
            if reconcile_runtime(source, dag, state, INTEGRATION_REF):
                atomic_json(state_path, state)
            if args.command == "retry":
                record = state.get("units", {}).get(args.unit)
                if not record or record.get("status") not in {"FAILED", "BLOCKED", "SUPERSEDED"}:
                    raise ControllerError("retry requires a FAILED, BLOCKED, or SUPERSEDED unit")
                record.update(status="RETRY", candidate=None, blocker=None)
                atomic_json(state_path, state)
                print(json.dumps({"status": "retry-ready", **snapshot(source, dag, state, INTEGRATION_REF)}, sort_keys=True))
                return 0
            if args.command == "replan":
                unit = next((unit for unit in units if unit["id"] == args.unit), None)
                record = state.get("units", {}).get(args.unit)
                if not unit or not record or record.get("status") not in {"FAILED", "BLOCKED"}:
                    raise ControllerError("replan requires an active FAILED or BLOCKED unit")
                base = integration_sha(source, INTEGRATION_REF)
                record["base"] = base
                result = {"status": "PLAN_CHANGE", "unit": args.unit, "base": base, "reason": args.reason}
                candidate, replan_retry = accept_plan_change(source, state_dir, base, unit, result, pinned_gitlink(dag), INTEGRATION_REF, state, state_path)
                record.update(status="RETRY" if replan_retry else "SUPERSEDED", candidate=candidate, blocker=None, plan_change=args.reason, replan_retry=replan_retry)
                state.setdefault("history", []).append({"unit": args.unit, "status": record["status"], "base": base, "candidate": candidate, "at": int(time.time())})
                atomic_json(state_path, state)
                revised = load_dag(source, dag_path, INTEGRATION_REF)
                print(json.dumps({"status": "replanned", **snapshot(source, revised, state, INTEGRATION_REF)}, sort_keys=True))
                return 0
            while True:
                outcome = run_one(source, dag, state, state_path, INTEGRATION_REF, state_dir, args.unit)
                if not should_continue(outcome, args.continuous):
                    print(json.dumps({"status": outcome, **snapshot(source, dag, state, INTEGRATION_REF)}, sort_keys=True))
                    return 0 if outcome in {"accepted", "replanned", "complete"} else 1
                dag = load_dag(source, dag_path, INTEGRATION_REF)
                state = load_runtime(source, dag, state_path, INTEGRATION_REF, write=False)
                args.unit = None
    except ControllerError as error:
        print(f"controller: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
