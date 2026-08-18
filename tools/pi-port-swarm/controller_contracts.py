#!/usr/bin/env python3
"""External leases and immutable scope approvals for parallel port units."""
from __future__ import annotations

import fcntl
import hashlib
import json
import os
import tempfile
import time
import urllib.request
import uuid
from pathlib import Path
from typing import Any, Callable


def atomic_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile("w", dir=path.parent, prefix=".tmp-", delete=False, encoding="utf-8") as file:
        json.dump(value, file, indent=2, sort_keys=True)
        file.write("\n")
        temporary = Path(file.name)
    temporary.replace(path)


def full_sha(value: Any) -> bool:
    return isinstance(value, str) and len(value) == 40 and all(character in "0123456789abcdef" for character in value)


def safe_paths(paths: list[str]) -> list[str]:
    result = sorted(set(paths))
    if not result:
        raise ValueError("a lease needs at least one path")
    if any(not path or Path(path).is_absolute() or ".." in Path(path).parts for path in result):
        raise ValueError("lease paths must be repository-relative")
    return result


def overlaps(left: str, right: str) -> bool:
    return left == right or left.startswith(right.rstrip("/") + "/") or right.startswith(left.rstrip("/") + "/")


class LeaseRegistry:
    """A process-external, atomic, expiring lease registry with an audit trail."""

    def __init__(self, directory: Path, *, clock: Callable[[], float] = time.time):
        directory.mkdir(parents=True, exist_ok=True)
        self.path = directory / "leases.json"
        self.lock_path = directory / "leases.lock"
        self.clock = clock
        if not self.path.exists():
            with self.lock_path.open("a+") as lock:
                fcntl.flock(lock, fcntl.LOCK_EX)
                if not self.path.exists():
                    atomic_json(self.path, {"version": 1, "leases": {}, "extensions": {}, "audit": []})

    def _update(self, operation: Callable[[dict[str, Any], int], Any]) -> Any:
        with self.lock_path.open("a+") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX)
            state = json.loads(self.path.read_text(encoding="utf-8"))
            now = int(self.clock())
            self._expire(state, now)
            result = operation(state, now)
            atomic_json(self.path, state)
            return result

    @staticmethod
    def _event(state: dict[str, Any], event: str, now: int, **fields: Any) -> None:
        state.setdefault("audit", []).append({"event": event, "at": now, **fields})

    def _expire(self, state: dict[str, Any], now: int) -> None:
        for token, lease in list(state.get("leases", {}).items()):
            if lease["expires_at"] <= now:
                del state["leases"][token]
                self._event(state, "EXPIRED", now, token=token, unit=lease["unit"], paths=lease["paths"])

    def snapshot(self) -> dict[str, Any]:
        return self._update(lambda state, _now: json.loads(json.dumps(state)))

    def extension(self, request_id: str) -> dict[str, Any] | None:
        return self._update(lambda state, _now: json.loads(json.dumps(state["extensions"].get(request_id))))

    def paths(self, token: str, unit: str) -> list[str]:
        def operation(state: dict[str, Any], _now: int) -> list[str]:
            lease = state["leases"].get(token)
            if not lease or lease["unit"] != unit:
                raise ValueError("unit lease is absent or expired")
            return list(lease["paths"])
        return self._update(operation)

    def acquire(self, unit: str, paths: list[str], *, ttl: int = 3600) -> str | None:
        paths = safe_paths(paths)
        if ttl <= 0:
            raise ValueError("lease ttl must be positive")

        def operation(state: dict[str, Any], now: int) -> str | None:
            conflict = next((lease for lease in state["leases"].values() if any(overlaps(path, held) for path in paths for held in lease["paths"])), None)
            if conflict:
                self._event(state, "CONFLICT", now, unit=unit, paths=paths, held_by=conflict["unit"])
                return None
            token = uuid.uuid4().hex
            state["leases"][token] = {"unit": unit, "paths": paths, "acquired_at": now, "expires_at": now + ttl, "pid": os.getpid()}
            self._event(state, "ACQUIRED", now, token=token, unit=unit, paths=paths, expires_at=now + ttl)
            return token

        return self._update(operation)

    def renew(self, token: str, *, ttl: int = 3600) -> bool:
        if ttl <= 0:
            raise ValueError("lease ttl must be positive")

        def operation(state: dict[str, Any], now: int) -> bool:
            lease = state["leases"].get(token)
            if not lease:
                self._event(state, "RENEW_MISSED", now, token=token)
                return False
            lease["expires_at"] = now + ttl
            self._event(state, "RENEWED", now, token=token, unit=lease["unit"], expires_at=now + ttl)
            return True

        return self._update(operation)

    def release(self, token: str, *, outcome: str) -> bool:
        def operation(state: dict[str, Any], now: int) -> bool:
            lease = state["leases"].pop(token, None)
            if not lease:
                self._event(state, "RELEASE_MISSED", now, token=token, outcome=outcome)
                return False
            self._event(state, "RELEASED", now, token=token, unit=lease["unit"], paths=lease["paths"], outcome=outcome)
            return True

        return self._update(operation)

    def request_extension(self, token: str, unit: str, paths: list[str], evidence: Path) -> str:
        paths = safe_paths(paths)
        evidence_sha256 = hashlib.sha256(evidence.read_bytes()).hexdigest()

        def operation(state: dict[str, Any], now: int) -> str:
            lease = state["leases"].get(token)
            if not lease or lease["unit"] != unit:
                raise ValueError("scope extension requires the unit's active lease")
            request_id = uuid.uuid4().hex
            request = {"request_id": request_id, "token": token, "unit": unit, "paths": paths, "evidence": str(evidence), "evidence_sha256": evidence_sha256, "status": "PENDING_GITHUB", "requested_at": now}
            state["extensions"][request_id] = request
            self._event(state, "SCOPE_REQUESTED", now, request_id=request_id, unit=unit, paths=paths, evidence_sha256=evidence_sha256)
            return request_id

        return self._update(operation)

    def approve_extension(self, request_id: str, approval: dict[str, Any]) -> bool:
        """Grant only an APPROVED GitHub PR review whose JSON body binds the request."""
        body = approval.get("body")
        try:
            binding = json.loads(body) if isinstance(body, str) else None
        except json.JSONDecodeError:
            binding = None
        if approval.get("state") != "APPROVED" or not isinstance(binding, dict) or not full_sha(approval.get("commit_id")):
            raise ValueError("approval must be an APPROVED GitHub PR review at an immutable commit")
        immutable = {key: approval.get(key) for key in ("id", "node_id", "html_url", "commit_id", "submitted_at")}
        if not immutable["id"] or not immutable["node_id"] or not immutable["html_url"] or not immutable["submitted_at"]:
            raise ValueError("approval lacks immutable GitHub review identity")

        def operation(state: dict[str, Any], now: int) -> bool:
            request = state["extensions"].get(request_id)
            if not request or request["status"] != "PENDING_GITHUB":
                return False
            expected = {"request_id": request_id, "unit": request["unit"], "evidence_sha256": request["evidence_sha256"], "paths": request["paths"]}
            if binding != expected:
                raise ValueError("GitHub approval is not bound to this unit, evidence, and exact scope")
            lease = state["leases"].get(request["token"])
            if not lease or lease["unit"] != request["unit"]:
                raise ValueError("scope approval arrived after the originating lease expired")
            conflict = next((held for token, held in state["leases"].items() if token != request["token"] and any(overlaps(path, existing) for path in request["paths"] for existing in held["paths"])), None)
            if conflict:
                self._event(state, "SCOPE_APPROVAL_CONFLICT", now, request_id=request_id, held_by=conflict["unit"])
                return False
            lease["paths"] = sorted(set(lease["paths"] + request["paths"]))
            payload_sha256 = hashlib.sha256(json.dumps(approval, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
            request.update(status="APPROVED", approved_at=now, approval={**immutable, "payload_sha256": payload_sha256})
            self._event(state, "SCOPE_APPROVED", now, request_id=request_id, unit=request["unit"], paths=request["paths"], approval=immutable, payload_sha256=payload_sha256)
            return True

        return self._update(operation)


def fetch_github_review(api_url: str) -> dict[str, Any]:
    if not api_url.startswith("https://api.github.com/repos/ZEDIUM-Off/zedflow/pulls/") or "/reviews/" not in api_url:
        raise ValueError("approval URL must identify a ZEDIUM-Off/zedflow pull-request review API resource")
    request = urllib.request.Request(api_url, headers={"Accept": "application/vnd.github+json", "User-Agent": "zedflow-pi-port-controller"})
    with urllib.request.urlopen(request, timeout=15) as response:
        value = json.load(response)
    if not isinstance(value, dict):
        raise ValueError("GitHub review response must be an object")
    return value
