#!/usr/bin/env python3
"""Classify stopped port failures and perform one bounded recovery action."""
from __future__ import annotations

import fcntl
import hashlib
import json
import os
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Any, Callable

HOME = Path.home()
SOURCE = Path(__file__).resolve().parents[2]
STATE_DIR = Path(os.environ.get("XDG_STATE_HOME", HOME / ".local/state")) / "zedflow-pi-port"
STATE_PATH = STATE_DIR / "state.json"
CONTROLLER = SOURCE / "tools/pi-port-swarm/controller.py"
PROMPT = SOURCE / ".pi/prompts/pi-port-recovery.md"
NOTIFY = HOME / ".local/bin/workspace-notify"
RECOVERY_DIR = STATE_DIR / "recovery"
OUTCOMES = {"REPAIRABLE", "PLAN_CHANGE_REQUIRED", "ARBITRATION_REQUIRED", "TRANSIENT"}


def atomic_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile("w", dir=path.parent, delete=False, encoding="utf-8") as file:
        json.dump(value, file, indent=2, sort_keys=True)
        file.write("\n")
        temporary = Path(file.name)
    temporary.replace(path)


def notify(title: str, message: str, priority: str = "high", tags: str = "warning") -> None:
    subprocess.run([str(NOTIFY), "-P", "zedflow", "-t", title, "-p", priority, "-g", tags, message], check=True)


def monitor() -> dict[str, Any]:
    completed = subprocess.run(["python3", str(CONTROLLER), "monitor"], cwd=SOURCE, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    return json.loads(completed.stdout) if completed.returncode == 0 else {"error": completed.stderr.strip(), "ready": []}


def active_failures(state: dict[str, Any], snapshot: dict[str, Any]) -> dict[str, Any]:
    blockers = snapshot.get("dag_progress", {}).get("blockers")
    return {unit: record for unit, record in state.get("units", {}).items() if isinstance(blockers, dict) and unit in blockers and record.get("status") in {"FAILED", "BLOCKED"}}


def final_result(stdout: str) -> dict[str, Any]:
    results = [value for line in stdout.splitlines() if isinstance((value := _json(line)), dict) and value.get("classification") in OUTCOMES]
    if len(results) != 1:
        raise RuntimeError("recovery analyst did not emit exactly one classification")
    return results[0]


def _json(line: str) -> Any:
    try:
        return json.loads(line)
    except json.JSONDecodeError:
        return None


def bounded_action(action: str, integration: str, unit: str) -> bool:
    path = RECOVERY_DIR / "actions.json"
    ledger = json.loads(path.read_text()) if path.exists() else {}
    key = f"{integration}:{unit}:{action}"
    if ledger.get(key, 0):
        return False
    ledger[key] = 1
    atomic_json(path, ledger)
    return True


def start_controller(runner: Callable[..., Any] = subprocess.run) -> bool:
    runner(["systemctl", "--user", "reset-failed", "zedflow-pi-port.service"], check=False)
    return runner(["systemctl", "--user", "start", "--no-block", "zedflow-pi-port.service"], check=False).returncode == 0


def resume(classification: str, unit: str, reason: str, runner: Callable[..., Any] = subprocess.run, starter: Callable[[], bool] = start_controller) -> bool:
    command = ["python3", str(CONTROLLER), "recover", "--unit", unit, "--classification", classification, "--reason", reason]
    completed = runner(command, cwd=SOURCE, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    return completed.returncode == 0 and starter()


def recovery_command(fingerprint: str, capsule: dict[str, Any]) -> list[str]:
    capsule_path = RECOVERY_DIR / f"{fingerprint}.capsule.json"
    atomic_json(capsule_path, capsule)
    return ["pi", "-p", "--approve", "--no-extensions", "--no-skills", "--no-prompt-templates", "--tools", "read,grep,find,ls", "--session-dir", str(RECOVERY_DIR / f"session-{int(time.time())}-{fingerprint}"), "--name", f"zedflow-port-recovery-{fingerprint}", f"@{PROMPT}", f"@{capsule_path}"]


def main() -> int:
    RECOVERY_DIR.mkdir(parents=True, exist_ok=True)
    with (STATE_DIR / "recovery.lock").open("w") as lock:
        try: fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError: return 0
        state, snapshot = json.loads(STATE_PATH.read_text()), monitor()
        failed = active_failures(state, snapshot)
        if not failed: return 0
        integration = snapshot.get("integration_sha", "unknown")
        fingerprint = hashlib.sha256(json.dumps({"integration": integration, "failed": failed}, sort_keys=True).encode()).hexdigest()[:16]
        result_path = RECOVERY_DIR / f"{fingerprint}.json"
        if result_path.exists(): return 0
        capsule = {"fingerprint": fingerprint, "integration_sha": integration, "failed": failed, "ready": snapshot.get("ready", []), "dag_progress": snapshot.get("dag_progress"), "source": str(SOURCE)}
        if len(failed) == 1:
            unit, record = next(iter(failed.items()))
            classification = record.get("classification")
            if classification in OUTCOMES:
                reason = str(record.get("blocker") or "classified controller blocker")
                result = {"unit": unit, "classification": classification, "summary": reason, "reason": reason}
            else:
                result = {}
        else:
            result = {}
        if not result:
            completed = subprocess.run(recovery_command(fingerprint, capsule), cwd=SOURCE, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=900)
            try: result = final_result(completed.stdout) if completed.returncode == 0 else {"classification": "TRANSIENT", "summary": completed.stderr[:300]}
            except RuntimeError as error: result = {"classification": "ARBITRATION_REQUIRED", "summary": str(error)}
        unit, classification = str(result.get("unit", "")), result["classification"]
        if classification != "ARBITRATION_REQUIRED" and unit in failed and bounded_action(classification, integration, unit):
            result["resumed"] = resume(classification, unit, str(result.get("reason") or result.get("summary", "")))
        else:
            result["resumed"] = False
        if not result["resumed"]:
            notify("Intervention requise", f"{classification}: {result.get('summary', '')[:500]}\nID: {fingerprint}", "high", "question")
        result.update(fingerprint=fingerprint, integration_sha=integration, handled_at=int(time.time()))
        atomic_json(result_path, result)
        return 0

if __name__ == "__main__":
    raise SystemExit(main())
