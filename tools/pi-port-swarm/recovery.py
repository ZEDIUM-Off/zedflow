#!/usr/bin/env python3
"""Analyze a stopped port flow and apply one bounded automatic recovery action."""

from __future__ import annotations

import fcntl
import hashlib
import json
import os
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Any

HOME = Path.home()
SOURCE = HOME / ".local/share/zedflow-worktrees/zedflow-main"
STATE_DIR = Path(os.environ.get("XDG_STATE_HOME", HOME / ".local/state")) / "zedflow-pi-port"
STATE_PATH = STATE_DIR / "state.json"
CONTROLLER = SOURCE / "tools/pi-port-swarm/controller.py"
PROMPT = SOURCE / ".pi/prompts/pi-port-recovery.md"
NOTIFY = HOME / ".local/bin/workspace-notify"
RECOVERY_DIR = STATE_DIR / "recovery"


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
    completed = subprocess.run(
        ["python3", str(CONTROLLER), "monitor"],
        cwd=SOURCE,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if completed.returncode:
        return {"error": completed.stderr.strip(), "ready": [], "current": None}
    return json.loads(completed.stdout)


def active_failures(state: dict[str, Any], snapshot: dict[str, Any]) -> dict[str, Any]:
    blockers = snapshot.get("dag_progress", {}).get("blockers")
    if not isinstance(blockers, dict):
        return {}
    active = set(blockers)
    return {
        unit: record
        for unit, record in state.get("units", {}).items()
        if unit in active and record.get("status") in {"FAILED", "BLOCKED"}
    }


def final_result(stdout: str) -> dict[str, Any]:
    results = []
    for line in stdout.splitlines():
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict) and value.get("action") in {"restart", "retry", "replan", "human"}:
            results.append(value)
    if len(results) != 1:
        raise RuntimeError("recovery analyst did not emit exactly one action")
    return results[0]


def start_controller() -> bool:
    subprocess.run(["systemctl", "--user", "reset-failed", "zedflow-pi-port.service"], check=False)
    return subprocess.run(["systemctl", "--user", "start", "--no-block", "zedflow-pi-port.service"], check=False).returncode == 0


def bounded_action(action: str, integration: str, unit: str) -> bool:
    ledger_path = RECOVERY_DIR / f"auto-{action}s.json"
    ledger = json.loads(ledger_path.read_text()) if ledger_path.exists() else {}
    key = f"{integration}:{unit}"
    if ledger.get(key, 0) >= 1:
        return False
    ledger[key] = 1
    atomic_json(ledger_path, ledger)
    return True


def main() -> int:
    RECOVERY_DIR.mkdir(parents=True, exist_ok=True)
    with (STATE_DIR / "recovery.lock").open("w") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            return 0

        state = json.loads(STATE_PATH.read_text(encoding="utf-8"))
        if any(record.get("status") in {"RUNNING", "ACCEPTING"} for record in state.get("units", {}).values()):
            return 0
        snapshot = monitor()
        if snapshot.get("error"):
            notify("Intervention requise", f"Le monitor recovery a échoué : {snapshot['error'][:500]}", "high", "question")
            return 0
        failed = active_failures(state, snapshot)
        integration = subprocess.check_output(["git", "rev-parse", "automation/pi-port"], cwd=SOURCE, text=True).strip()
        if not failed:
            ready = snapshot.get("ready")
            unit = ready[0] if isinstance(ready, list) and ready and isinstance(ready[0], str) else None
            if not unit:
                return 0
            if not bounded_action("restart", integration, unit):
                notify("Intervention requise", f"La reprise automatique de {unit} a déjà été tentée.", "high", "question")
            elif start_controller():
                notify("Recovery automatique · reprise", f"Aucun blocage actif. Reprise : {unit}", "default", "arrows_counterclockwise")
            else:
                notify("Intervention requise", f"Le redémarrage automatique de {unit} a échoué.", "high", "question")
            return 0

        fingerprint_payload = json.dumps({"integration": integration, "failed": failed}, sort_keys=True, separators=(",", ":"))
        fingerprint = hashlib.sha256(fingerprint_payload.encode()).hexdigest()[:16]
        result_path = RECOVERY_DIR / f"{fingerprint}.json"
        if result_path.exists():
            return 0

        session_dir = RECOVERY_DIR / f"session-{int(time.time())}-{fingerprint}"
        capsule = {
            "fingerprint": fingerprint,
            "integration_sha": integration,
            "failed": failed,
            "current": snapshot.get("current"),
            "ready": snapshot.get("ready", []),
            "dag_progress": snapshot.get("dag_progress"),
            "monitor_error": snapshot.get("error"),
            "state_path": str(STATE_PATH),
            "source": str(SOURCE),
        }
        completed = subprocess.run(
            [
                "pi", "-p", "--approve",
                "--no-extensions", "--no-skills", "--no-prompt-templates",
                "--tools", "read,grep,find,ls",
                "--session-dir", str(session_dir),
                "--name", f"zedflow-port-recovery-{fingerprint}",
                f"@{PROMPT}",
                json.dumps(capsule, separators=(",", ":")),
            ],
            cwd=SOURCE,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=900,
        )
        try:
            result = final_result(completed.stdout) if completed.returncode == 0 else {
                "action": "human",
                "summary": f"Recovery Pi failed with exit {completed.returncode}: {completed.stderr.strip()[:300]}",
                "question": "Veux-tu que j'inspecte et relance manuellement la boucle ?",
            }
        except RuntimeError as error:
            result = {"action": "human", "summary": str(error), "question": "Veux-tu que j'inspecte et relance manuellement la boucle ?"}

        action = result["action"]
        summary = " ".join(str(result.get("summary", "")).split())[:500]
        if action == "restart":
            if snapshot.get("ready") and start_controller():
                notify("Recovery automatique · reprise", f"{summary}\nReprise : {snapshot['ready'][0]}\nID : {fingerprint}", "default", "arrows_counterclockwise")
            elif snapshot.get("ready"):
                action = "human"
                result["question"] = "Le redémarrage automatique du contrôleur a échoué. Intervenir manuellement ?"
            else:
                action = "human"
                result["question"] = "Aucune unité n'est prête; quelle mutation du plan faut-il appliquer ?"
        elif action in {"retry", "replan"}:
            unit = str(result.get("unit", ""))
            if unit not in failed or not bounded_action(action, integration, unit):
                action = "human"
                result["question"] = f"Le {action} automatique est invalide ou déjà consommé. Intervenir manuellement ?"
            else:
                command = ["python3", str(CONTROLLER), action, "--unit", unit]
                if action == "replan":
                    command += ["--reason", str(result.get("reason") or summary)]
                recovered = subprocess.run(command, cwd=SOURCE, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
                if recovered.returncode:
                    attempted = action
                    action = "human"
                    result["question"] = f"Le {attempted} automatique a échoué: {recovered.stderr.strip()[:300]}"
                elif start_controller():
                    notify(
                        f"Recovery automatique · {action} {unit}",
                        f"{summary}\nID : {fingerprint}",
                        "default",
                        "arrows_counterclockwise",
                    )
                else:
                    action = "human"
                    result["question"] = "Le plan a été corrigé, mais le redémarrage automatique du contrôleur a échoué."

        if action == "human":
            question = " ".join(str(result.get("question", "Quelle action dois-je appliquer ?")).split())[:500]
            notify("Intervention requise", f"{summary}\nQuestion : {question}\nRéponds dans Pi avec l’ID {fingerprint}.", "high", "question")

        result.update(action=action, fingerprint=fingerprint, integration_sha=integration, handled_at=int(time.time()))
        atomic_json(result_path, result)
        return 0


if __name__ == "__main__":
    raise SystemExit(main())
