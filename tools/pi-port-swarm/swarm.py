#!/usr/bin/env python3
"""Bounded, non-interactive coordinator for the frozen Pi TypeScript to Rust port."""
from __future__ import annotations

import argparse
import concurrent.futures
import contextlib
import fcntl
import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile
import time
from pathlib import Path

MODELS = {"openai-codex/gpt-5.6-luna", "openai-codex/gpt-5.6-terra", "openai-codex/gpt-5.6-sol"}
MUTATING_ROLES = {"writer", "reconcile"}
STATES = {"CLAIMED", "IMPLEMENTED", "REVIEWED", "VALIDATED", "INTEGRATED", "CLOSED", "FAILED", "BLOCKED"}
TERMINAL = {"CLOSED", "BLOCKED"}
MAX_SLOTS, MAX_RECOVERY_SLOTS, MAX_SUBAGENTS, MAX_ATTEMPTS = 3, 3, 18, 2
RUN_SECONDS, PI_RUN_SECONDS = 2 * 3600 + 45 * 60, 2 * 3600 + 35 * 60
ROOT = Path(os.environ.get("ZEDFLOW_SOURCE", "/home/zedium/workspaces/zedflow")).resolve()
DATA = Path(os.environ.get("XDG_DATA_HOME", Path.home() / ".local/share")) / "zedflow-pi-port-swarm"
STATE = Path(os.environ.get("XDG_STATE_HOME", Path.home() / ".local/state")) / "zedflow-pi-port-swarm"


class DagError(ValueError):
    pass


def run(args, cwd=None, env=None, check=True, timeout=None):
    return subprocess.run(
        args,
        cwd=cwd,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=check,
        timeout=timeout,
    )


def git(repo, *args, env=None, check=True):
    return run(["git", *args], repo, env, check)


def sha(repo, ref="HEAD"):
    return git(repo, "rev-parse", ref).stdout.strip()


def load_json(path):
    with open(path, encoding="utf-8") as file:
        return json.load(file)


def atomic_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, temporary = tempfile.mkstemp(dir=path.parent, prefix=".tmp-")
    with os.fdopen(fd, "w", encoding="utf-8") as file:
        json.dump(value, file, indent=2, sort_keys=True)
        file.write("\n")
    os.replace(temporary, path)


def plan_hash(dag):
    return hashlib.sha256(json.dumps(dag, sort_keys=True).encode()).hexdigest()


def validate_dag(dag):
    units = dag.get("units")
    if not isinstance(units, list) or not units:
        raise DagError("units must be a non-empty list")
    ids, graph, owners = set(), {}, {}
    exclusive = {"Cargo.toml", "Cargo.lock", "lib.rs", "mod.rs"}
    for unit in units:
        uid = unit.get("id")
        if not isinstance(uid, str) or not uid or uid in ids:
            raise DagError(f"duplicate or invalid unit id: {uid!r}")
        if unit.get("model") not in MODELS:
            raise DagError(f"{uid}: forbidden model {unit.get('model')!r}")
        if unit.get("role") not in {"writer", "reviewer", "validator", "reconcile"}:
            raise DagError(f"{uid}: invalid role")
        if "ownership" not in unit or "depends_on" not in unit:
            raise DagError(f"{uid}: ownership and dependencies are required")
        if not isinstance(unit["ownership"], list) or not isinstance(unit["depends_on"], list):
            raise DagError(f"{uid}: ownership and dependencies must be lists")
        ids.add(uid)
        graph[uid] = unit
        for path in unit["ownership"]:
            if Path(path).name in exclusive:
                if path in owners:
                    raise DagError(f"{path}: exclusive ownership by {owners[path]} and {uid}")
                owners[path] = uid
    for uid, unit in graph.items():
        for dependency in unit["depends_on"]:
            if dependency not in ids:
                raise DagError(f"{uid}: unknown dependency {dependency}")
    visiting, visited = set(), set()

    def walk(uid):
        if uid in visiting:
            raise DagError(f"cycle at {uid}")
        if uid not in visited:
            visiting.add(uid)
            for dependency in graph[uid]["depends_on"]:
                walk(dependency)
            visiting.remove(uid)
            visited.add(uid)

    for uid in graph:
        walk(uid)
    return graph


def ready_units(dag, state):
    """Return ready units, allowing only one mutating integration at a time."""
    graph, records, selected = validate_dag(dag), state.get("units", {}), []
    mutation_selected = False
    for uid, unit in graph.items():
        record = records.get(uid, {})
        if record.get("status") in TERMINAL or record.get("status") in {"CLAIMED", "IMPLEMENTED", "REVIEWED", "VALIDATED", "INTEGRATED"}:
            continue
        if record.get("attempts", 0) >= MAX_ATTEMPTS:
            continue
        if not all(records.get(dep, {}).get("status") == "CLOSED" for dep in unit["depends_on"]):
            continue
        if unit["role"] in MUTATING_ROLES and mutation_selected:
            continue
        ownership = set(unit["ownership"])
        if any(ownership & set(other["ownership"]) for other in selected):
            continue
        selected.append(unit)
        mutation_selected |= unit["role"] in MUTATING_ROLES
    return selected


def snapshot(repo):
    """Create a snapshot commit through a disposable index without source mutation."""
    repo = Path(repo)
    before = (sha(repo), git(repo, "diff", "--cached", "--binary").stdout)
    fd, index = tempfile.mkstemp(prefix="pi-port-index-")
    os.close(fd)
    os.unlink(index)
    try:
        environment = os.environ.copy()
        environment["GIT_INDEX_FILE"] = index
        git(repo, "read-tree", "HEAD", env=environment)
        git(repo, "add", "-A", env=environment)
        tree = git(repo, "write-tree", env=environment).stdout.strip()
        commit = git(repo, "commit-tree", tree, "-p", before[0], "-m", "pi-port-swarm snapshot", env=environment).stdout.strip()
    finally:
        with contextlib.suppress(FileNotFoundError):
            os.unlink(index)
    after = (sha(repo), git(repo, "diff", "--cached", "--binary").stdout)
    if before != after:
        raise RuntimeError("snapshot changed source HEAD or index")
    return commit


def pinned_pi(dag):
    try:
        _, pin = dag["source_gitlink"].split("@", 1)
    except (KeyError, ValueError) as error:
        raise DagError("source_gitlink must be references/pi@<sha>") from error
    return pin


def verify_pi_gitlink(repo, commit, pin):
    found = git(repo, "ls-tree", commit, "references/pi").stdout.split()[2]
    if found != pin:
        raise DagError(f"references/pi gitlink {found} differs from frozen {pin}")


def init_pi(repo, source, pin):
    """Initialize only Pi from the local source; LangGraph is intentionally untouched."""
    git(repo, "config", "submodule.references/pi.url", str(Path(source) / "references/pi"))
    git(repo, "-c", "protocol.file.allow=always", "submodule", "update", "--init", "--no-fetch", "references/pi")
    verify_pi_gitlink(repo, "HEAD", pin)


def bootstrap(repo, dag):
    """Create the automation clone once; never rewind an existing integration ref."""
    repo, clone, pin = Path(repo).resolve(), DATA / "repo", pinned_pi(dag)
    if not clone.exists():
        source_sha = snapshot(repo)
        verify_pi_gitlink(repo, source_sha, pin)
        clone.parent.mkdir(parents=True, exist_ok=True)
        run(["git", "clone", "--no-hardlinks", str(repo), str(clone)])
        git(clone, "fetch", str(repo), source_sha)
        git(clone, "branch", "-f", "automation/pi-port", source_sha)
    elif git(clone, "show-ref", "--verify", "--quiet", "refs/heads/automation/pi-port", check=False).returncode == 0:
        source_sha = sha(clone, "automation/pi-port")
    else:
        source_sha = snapshot(repo)
        verify_pi_gitlink(repo, source_sha, pin)
        git(clone, "fetch", str(repo), source_sha)
        git(clone, "branch", "automation/pi-port", source_sha)
    verify_pi_gitlink(clone, source_sha, pin)
    if git(clone, "status", "--porcelain").stdout:
        raise DagError("automation clone is dirty; retained for recovery")
    git(clone, "switch", "--detach", source_sha)
    init_pi(clone, repo, pin)
    for number in range(1, MAX_SLOTS + 1):
        slot = DATA / "worktrees" / f"slot-{number}"
        branch = f"automation/pi-port-slot-{number}"
        if slot.exists():
            continue  # recovery evidence is never deleted or cleaned
        slot.parent.mkdir(parents=True, exist_ok=True)
        git(clone, "worktree", "add", "-B", branch, str(slot), "automation/pi-port")
        init_pi(slot, repo, pin)
    return {"snapshot": source_sha, "clone": str(clone), "pi_gitlink": pin}


def runtime_dag(path):
    """Read the DAG from the integration ref after bootstrap, otherwise from source."""
    clone = DATA / "repo"
    if clone.exists() and git(clone, "show-ref", "--verify", "--quiet", "refs/heads/automation/pi-port", check=False).returncode == 0:
        raw = git(clone, "show", "automation/pi-port:tools/pi-port-swarm/dag.json").stdout
        return json.loads(raw)
    return load_json(path)


def slot_is_clean(slot):
    return git(slot, "status", "--porcelain").stdout == ""


def prepare_slot(unit, expected_head, attempt, source, pin, state, reserved=None):
    """Use a clean persistent slot; retain dirty slots and provision bounded replacements."""
    reserved = reserved if reserved is not None else set()
    clone = DATA / "repo"
    for number in range(1, MAX_SLOTS + MAX_RECOVERY_SLOTS + 1):
        slot = DATA / "worktrees" / f"slot-{number}"
        if slot in reserved:
            continue
        if not slot.exists():
            slot.parent.mkdir(parents=True, exist_ok=True)
            git(clone, "worktree", "add", "--detach", str(slot), expected_head)
            init_pi(slot, source, pin)
        if not slot_is_clean(slot):
            state.setdefault("recovery", {})[str(slot)] = "dirty; retained without reset, clean, or removal"
            continue
        branch = f"automation/pi-port-run-{unit['id'].lower()}-{attempt}-{time.time_ns()}-{number}"
        git(slot, "switch", "--detach", expected_head)
        git(slot, "switch", "-c", branch)
        init_pi(slot, source, pin)
        reserved.add(slot)
        return slot
    return None


def allowed_files(unit, changed):
    return all(any(path == prefix or path.startswith(prefix.rstrip("/") + "/") for prefix in unit["ownership"]) for path in changed)


def require_done(result):
    if result.get("status") != "DONE":
        raise DagError("only a structured DONE result may close a unit")
    orchestration = result.get("orchestration", {})
    if orchestration.get("listed_agents") is not True or orchestration.get("waited_for_all") is not True:
        raise DagError("result lacks subagent list/wait evidence")


def require_writer_evidence(result, commit):
    reviews = result.get("reviews", [])
    by_kind = {review.get("kind"): review for review in reviews if isinstance(review, dict)}
    for kind in ("fidelity", "rust"):
        review = by_kind.get(kind, {})
        if review.get("status") != "PASS" or review.get("sha") != commit or not review.get("run_id"):
            raise DagError(f"writer result lacks independent {kind} PASS evidence")
    validation = result.get("validation", {})
    if validation.get("status") != "PASS" or validation.get("sha") != commit or not validation.get("run_id"):
        raise DagError("writer result lacks independent exact-SHA validation evidence")
    return by_kind, validation


def session_file(session_dir, name):
    matches = []
    for path in session_dir.rglob("*.jsonl"):
        try:
            for line in path.read_text(encoding="utf-8").splitlines():
                entry = json.loads(line)
                if entry.get("type") == "session_info" and entry.get("name") == name:
                    matches.append(path)
                    break
        except (OSError, json.JSONDecodeError):
            continue
    if not matches:
        raise DagError(f"persisted parent session not found for {name}")
    return max(matches, key=lambda path: path.stat().st_mtime_ns)


def child_artifact(run_id, expected_sha):
    if not isinstance(run_id, str) or not re.fullmatch(r"[0-9a-f-]{8,36}", run_id):
        raise DagError(f"invalid subagent run id: {run_id!r}")
    candidates = list(Path("/tmp").glob(f"pi-subagents-uid-*/async-subagent-runs/{run_id}*"))
    if len(candidates) != 1:
        raise DagError(f"subagent artifact not uniquely found for {run_id}")
    status = load_json(candidates[0] / "status.json")
    if status.get("state") != "complete":
        raise DagError(f"subagent {run_id} did not complete successfully")
    output = candidates[0] / "output-0.log"
    child = parse_result(output.read_text(encoding="utf-8"))
    if child.get("status") != "PASS" or child.get("sha") != expected_sha:
        raise DagError(f"subagent {run_id} lacks PASS on {expected_sha}")


def verify_session_evidence(path, result, commit):
    """Bind claimed reviewer IDs to actual parent tool calls and completed child artifacts."""
    calls, results = {}, {}
    for line in Path(path).read_text(encoding="utf-8").splitlines():
        entry = json.loads(line)
        if entry.get("type") != "message":
            continue
        message = entry.get("message", {})
        if message.get("role") == "assistant":
            for item in message.get("content", []):
                if isinstance(item, dict) and item.get("type") == "toolCall":
                    calls[item.get("id")] = (item.get("name"), item.get("arguments", {}))
        elif message.get("role") == "toolResult":
            results[message.get("toolCallId")] = "\n".join(
                item.get("text", "") for item in message.get("content", []) if isinstance(item, dict)
            )
    if not any(name == "subagent" and arguments.get("action") == "list" for name, arguments in calls.values()):
        raise DagError("parent session lacks subagent list call")
    if not any(name == "wait" and arguments.get("all") is True for name, arguments in calls.values()):
        raise DagError("parent session lacks wait(all=true)")
    launched = {}
    evidence_agents = {"pi-fidelity-reviewer", "pi-rust-reviewer", "pi-port-validator"}
    for call_id, (name, arguments) in calls.items():
        if name != "subagent" or arguments.get("action"):
            continue
        task_agents = {task.get("agent") for task in arguments.get("tasks", [])}
        if task_agents & evidence_agents:
            raise DagError("review and validator evidence agents require separate subagent calls")
        agent = arguments.get("agent")
        if agent:
            launched.setdefault(agent, []).append(results.get(call_id, ""))
    by_kind, validation = require_writer_evidence(result, commit)
    expected = {
        "pi-fidelity-reviewer": by_kind["fidelity"]["run_id"],
        "pi-rust-reviewer": by_kind["rust"]["run_id"],
        "pi-port-validator": validation["run_id"],
    }
    if len(set(expected.values())) != len(expected):
        raise DagError("review and validator run IDs must be distinct")
    for agent, run_id in expected.items():
        if not any(run_id in text for text in launched.get(agent, [])):
            raise DagError(f"parent session does not bind {run_id} to {agent}")
        child_artifact(run_id, commit)


def accept_result(repo, unit, result, expected_head, parent_session=None):
    """Validate a role-specific result before closing a node or moving the port ref."""
    require_done(result)
    role, commit = unit["role"], result.get("commit")
    if role in MUTATING_ROLES:
        if not commit or result.get("sha") not in {None, commit}:
            raise DagError("writer result requires its commit SHA")
        require_writer_evidence(result, commit)
        if parent_session is None:
            raise DagError("writer result requires persisted parent-session evidence")
        verify_session_evidence(parent_session, result, commit)
        if git(repo, "merge-base", "--is-ancestor", expected_head, commit, check=False).returncode != 0:
            raise DagError("writer commit is not a descendant of expected_head")
        if git(repo, "diff", "--quiet", expected_head, commit, check=False).returncode == 0:
            raise DagError("writer result has an empty commit range")
        changed = git(repo, "diff", "--name-only", expected_head, commit).stdout.splitlines()
        if not allowed_files(unit, changed):
            raise DagError("commit changes files outside ownership")
        if sha(repo, "refs/heads/automation/pi-port") != expected_head:
            raise DagError("expected_head CAS failed")
        git(repo, "update-ref", "refs/heads/automation/pi-port", commit, expected_head)
    elif role == "reviewer":
        if commit or result.get("sha") != expected_head or result.get("review") != "PASS":
            raise DagError("reviewer result requires PASS on expected_head and no commit")
    else:  # validator
        validation = result.get("validation", {})
        if commit or result.get("sha") != expected_head or validation.get("status") != "PASS" or validation.get("sha") != expected_head:
            raise DagError("validator result requires PASS validation on expected_head and no commit")
    return True


def invocation(unit, session_dir, worktree):
    """Use Terra normally and Sol only for checkpoint reconciliation."""
    prompt = worktree / ".pi/prompts/pi-port-swarm.md"
    name = f"pi-port-{unit['id'].lower()}-{time.time_ns()}"
    supervisor_model = "openai-codex/gpt-5.6-sol" if unit["id"] == "RECONCILE-CHECKPOINT" else "openai-codex/gpt-5.6-terra"
    message = f"Execute unit {unit['id']} only in {worktree}. The DAG assigns {unit['model']} to the unit; apply the role routing policy and return one JSON result line."
    return ["pi", "-p", "--session-dir", str(session_dir), "--name", name, "--model", supervisor_model, "--thinking", "high", "--approve", f"@{prompt}", message], name


def run_environment():
    environment = os.environ.copy()
    environment.update({
        "PI_SKIP_VERSION_CHECK": "1", "PI_TELEMETRY": "0", "PI_SUBAGENT_MAX_SPAWNS_PER_SESSION": str(MAX_SUBAGENTS),
        "PI_SUBAGENT_WAIT_TOOL_ENABLED": "true", "CARGO_TARGET_DIR": "/tmp/zedflow-pi-port-swarm-target", "TMPDIR": "/tmp/zedflow-pi-port-swarm-tmp",
    })
    Path(environment["CARGO_TARGET_DIR"]).mkdir(parents=True, exist_ok=True)
    Path(environment["TMPDIR"]).mkdir(parents=True, exist_ok=True)
    return environment


def parse_result(output):
    for line in reversed(output.splitlines()):
        try:
            value = json.loads(line)
            if isinstance(value, dict):
                return value
        except json.JSONDecodeError:
            pass
    raise DagError("pi output has no JSON result")


def mark_failure(record, error):
    record.update(status="FAILED" if record.get("attempts", 0) < MAX_ATTEMPTS else "BLOCKED", error=str(error))


def recover_claims(state):
    """A held flock means any inherited CLAIMED record came from an interrupted tick."""
    for record in state.get("units", {}).values():
        if record.get("status") == "CLAIMED":
            mark_failure(record, "previous tick ended before producing an integration result")


def reconcile_pending(state, state_path):
    """Finish a CAS transaction that may have been interrupted after ref movement."""
    pending = state.get("pending_integration")
    if not pending:
        return
    repo = DATA / "repo"
    current = sha(repo, "automation/pi-port")
    unit, result = pending["unit"], pending["result"]
    record = state["units"].setdefault(unit["id"], {})
    try:
        if current == pending["expected_head"]:
            accept_result(repo, unit, result, pending["expected_head"], pending.get("parent_session"))
        elif current != result["commit"]:
            raise DagError(f"integration ref diverged during recovery: {current}")
        record.update(result, status="CLOSED")
        state.pop("pending_integration", None)
    except (DagError, KeyError) as error:
        mark_failure(record, error)
        state.pop("pending_integration", None)
    atomic_json(state_path, state)


def execute_pi(unit, session_dir, slot):
    command, name = invocation(unit, session_dir, slot)
    try:
        completed = run(command, cwd=slot, env=run_environment(), check=False, timeout=PI_RUN_SECONDS)
    except subprocess.TimeoutExpired as error:
        completed = subprocess.CompletedProcess(error.cmd, 124, error.stdout or "", (error.stderr or "") + "\npi run timed out\n")
    except OSError as error:
        completed = subprocess.CompletedProcess(command, 127, "", f"failed to start pi: {error}\n")
    try:
        persisted = session_file(session_dir, name)
    except DagError:
        persisted = None
    return completed, persisted


def tick(args):
    dag = runtime_dag(args.dag)
    validate_dag(dag)
    STATE.mkdir(parents=True, exist_ok=True)
    state_path = STATE / "state.json"
    state = load_json(state_path) if state_path.exists() else {"units": {}, "runs": []}
    if not state.get("bootstrap"):
        state["bootstrap"] = bootstrap(args.source, dag)
        atomic_json(state_path, state)
        dag = runtime_dag(args.dag)
        validate_dag(dag)
    reconcile_pending(state, state_path)
    recover_claims(state)
    atomic_json(state_path, state)
    started, launched, pin, jobs, reserved_slots = time.monotonic(), 0, pinned_pi(dag), [], set()
    for unit in ready_units(dag, state):
        if time.monotonic() - started >= RUN_SECONDS or launched >= MAX_SUBAGENTS:
            break
        uid = unit["id"]
        record = state["units"].setdefault(uid, {})
        expected_head = sha(DATA / "repo", "automation/pi-port")
        attempt = record.get("attempts", 0) + 1
        slot = prepare_slot(unit, expected_head, attempt, args.source, pin, state, reserved_slots)
        if slot is None:
            record.update(attempts=attempt)
            mark_failure(record, "no clean persistent worktree slot")
            atomic_json(state_path, state)
            continue
        record.update(status="CLAIMED", attempts=attempt, expected_head=expected_head, idempotence=f"{uid}+{expected_head}+{plan_hash(dag)}")
        jobs.append((unit, record, expected_head, slot))
        launched += 1
        atomic_json(state_path, state)
    session_dir = STATE / "sessions"
    session_dir.mkdir(parents=True, exist_ok=True)
    parallel_reviews = len(jobs) == 2 and {job[0]["id"] for job in jobs} == {"RV-FID", "RV-RUST"} and all(job[0]["role"] == "reviewer" for job in jobs)
    if parallel_reviews:
        with concurrent.futures.ThreadPoolExecutor(max_workers=2) as executor:
            completed_runs = list(executor.map(lambda job: execute_pi(job[0], session_dir, job[3]), jobs))
    else:
        completed_runs = [execute_pi(unit, session_dir, slot) for unit, _, _, slot in jobs]
    for (unit, record, expected_head, _), (completed, parent_session) in zip(jobs, completed_runs):
        uid = unit["id"]
        log = STATE / "logs" / f"{uid}-{int(time.time())}.log"
        log.parent.mkdir(parents=True, exist_ok=True)
        log.write_text(completed.stdout + completed.stderr)
        try:
            if completed.returncode != 0:
                raise DagError(f"pi exited {completed.returncode}")
            result = parse_result(completed.stdout)
            if unit["role"] in MUTATING_ROLES:
                state["pending_integration"] = {"unit": unit, "result": result, "expected_head": expected_head, "parent_session": str(parent_session) if parent_session else None}
                atomic_json(state_path, state)
            accept_result(DATA / "repo", unit, result, expected_head, parent_session)
            record.update(result, status="CLOSED")
            state.pop("pending_integration", None)
        except (DagError, KeyError) as error:
            mark_failure(record, error)
            if sha(DATA / "repo", "automation/pi-port") == expected_head:
                state.pop("pending_integration", None)
        state["runs"].append({"unit": uid, "exit": completed.returncode, "log": str(log)})
        atomic_json(state_path, state)
    print(json.dumps({"ready": [unit["id"] for unit in ready_units(runtime_dag(args.dag), state)], "state": str(state_path)}))


def paseo_connection():
    home = Path(os.environ.get("PASEO_HOME", Path.home() / ".paseo"))
    config = load_json(home / "config.json")
    host = config.get("daemon", {}).get("listen")
    if not host:
        raise DagError("Paseo daemon.listen is not configured")
    environment = os.environ.copy()
    password_file = home / "credentials" / "daemon-password"
    if password_file.exists():
        environment["PASEO_PASSWORD"] = password_file.read_text(encoding="utf-8").strip()
    return host, environment


def install(args):
    """Create or update the remotely visible Paseo hourly schedule."""
    host, environment = paseo_connection()
    name = "zedflow-pi-port-swarm"
    prompt = (ROOT / ".pi/prompts/pi-port-paseo-schedule.md").read_text(encoding="utf-8").strip()
    listed = run(["paseo", "--json", "schedule", "ls", "--host", host], env=environment)
    schedules = json.loads(listed.stdout)
    existing = next((schedule for schedule in schedules if schedule.get("name") == name), None)
    common = [
        "--cron", "0 * * * *", "--timezone", "Europe/Paris", "--name", name,
        "--provider", "pi/openai-codex/gpt-5.6-luna", "--cwd", str(Path(args.source).resolve()),
        "--json", "--host", host,
    ]
    if existing:
        completed = run(["paseo", "schedule", "update", existing["id"], "--prompt", prompt, *common], env=environment)
    else:
        completed = run(["paseo", "schedule", "create", prompt, "--target", "new-agent", *common], env=environment)
    print(completed.stdout.strip())


def parser():
    result = argparse.ArgumentParser()
    result.add_argument("--source", type=Path, default=ROOT)
    result.add_argument("--dag", type=Path, default=ROOT / "tools/pi-port-swarm/dag.json")
    subcommands = result.add_subparsers(dest="command", required=True)
    for command in ("validate-dag", "status", "tick", "install"):
        subcommands.add_parser(command)
    return result


def main():
    args = parser().parse_args()
    if args.command == "validate-dag":
        validate_dag(runtime_dag(args.dag))
        print("DAG valid")
        return 0
    if args.command == "status":
        print(json.dumps(load_json(STATE / "state.json") if (STATE / "state.json").exists() else {}, indent=2))
        return 0
    lock = STATE / "swarm.lock"
    lock.parent.mkdir(parents=True, exist_ok=True)
    with open(lock, "w") as file:
        try:
            fcntl.flock(file, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            print("swarm already running", file=sys.stderr)
            return 75
        if args.command == "tick":
            tick(args)
        else:
            install(args)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (DagError, subprocess.CalledProcessError, OSError) as error:
        print(f"swarm: {error}", file=sys.stderr)
        raise SystemExit(1)
