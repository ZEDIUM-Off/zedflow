#!/usr/bin/env python3
"""Shared primitives for exact real-boundary Pi ↔ Zedflow evidence."""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import subprocess
import tarfile
import tempfile
import threading
from contextlib import contextmanager
from dataclasses import dataclass
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Iterator

ROOT = Path(__file__).resolve().parents[2]
PI = ROOT / "references" / "pi"
PIN = "2b00dade7cec918aefb025c8b7a4fa304a30acdd"
MODEL = "fixture-model"
PROVIDER = "fixture"
API_KEY = "fixture-key"


@dataclass
class Target:
    name: str
    command: list[str]
    sandbox: Path


class _ReplayHandler(BaseHTTPRequestHandler):
    def do_POST(self) -> None:  # noqa: N802 - stdlib hook
        length = int(self.headers.get("content-length", "0"))
        body = self.rfile.read(length)
        if self.headers.get("authorization") != f"Bearer {API_KEY}":
            self.send_error(401)
            return
        if API_KEY.encode() in body:
            self.send_error(400, "credential leaked into request body")
            return
        self.server.requests.append(json.loads(body))  # type: ignore[attr-defined]
        chunks = [
            {"id": "fixture-response", "model": MODEL, "choices": [{"index": 0, "delta": {"content": "deterministic "}}]},
            {"id": "fixture-response", "model": MODEL, "choices": [{"index": 0, "delta": {"content": "reply"}, "finish_reason": "stop"}], "usage": {"prompt_tokens": 1, "completion_tokens": 2, "total_tokens": 3}},
        ]
        payload = "".join(f"data: {json.dumps(chunk, separators=(',', ':'))}\n\n" for chunk in chunks) + "data: [DONE]\n\n"
        encoded = payload.encode()
        self.send_response(200)
        self.send_header("content-type", "text/event-stream")
        self.send_header("content-length", str(len(encoded)))
        self.end_headers()
        self.wfile.write(encoded)

    def log_message(self, _format: str, *_args: object) -> None:
        pass


@contextmanager
def replay_server() -> Iterator[tuple[str, list[object]]]:
    server = ThreadingHTTPServer(("127.0.0.1", 0), _ReplayHandler)
    server.requests = []  # type: ignore[attr-defined]
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield f"http://127.0.0.1:{server.server_port}/v1", server.requests  # type: ignore[attr-defined]
    finally:
        server.shutdown()
        thread.join()
        server.server_close()


def require(name: str) -> str:
    executable = shutil.which(name)
    if not executable:
        raise SystemExit(f"differential prototype requires {name}")
    return executable


def prepare_pi(destination: Path) -> Path:
    if subprocess.check_output(["git", "-C", PI, "rev-parse", "HEAD"], text=True).strip() != PIN:
        raise SystemExit(f"frozen Pi must be {PIN}")
    destination.mkdir()
    process = subprocess.Popen(["git", "-C", PI, "archive", PIN], stdout=subprocess.PIPE)
    assert process.stdout
    with tarfile.open(fileobj=process.stdout, mode="r|") as archive:
        archive.extractall(destination, filter="data")
    if process.wait() != 0:
        raise SystemExit("git archive failed")
    subprocess.run([require("npm"), "ci", "--offline", "--ignore-scripts"], cwd=destination, check=True)
    return destination


def build_zedflow() -> Path:
    target_dir = Path(os.environ.get("CARGO_TARGET_DIR", "/tmp/zedflow-prototype-target"))
    env = {**os.environ, "CARGO_TARGET_DIR": str(target_dir), "CARGO_NET_OFFLINE": "true"}
    subprocess.run([require("cargo"), "build", "-p", "zedflow-coding-agent"], cwd=ROOT, env=env, check=True)
    return target_dir / "debug" / "zedflow-coding-agent"


def make_target(name: str, executable: list[str], root: Path, base_url: str) -> Target:
    sandbox = root / name
    agent = sandbox / "agent"
    sessions = sandbox / "sessions"
    agent.mkdir(parents=True)
    sessions.mkdir()
    (agent / "settings.json").write_text(json.dumps({"defaultProjectTrust": "always", "quietStartup": True}))
    (agent / "models.json").write_text(json.dumps({
        "providers": {
            PROVIDER: {
                "baseUrl": base_url,
                "api": "openai-completions",
                "apiKey": API_KEY,
                "models": [{"id": MODEL, "reasoning": False, "contextWindow": 128000, "maxTokens": 1024}],
            }
        }
    }))
    return Target(name, executable, sandbox)


def environment(target: Target) -> dict[str, str]:
    return {
        "PATH": os.environ["PATH"],
        "HOME": str(target.sandbox / "home"),
        "PI_CODING_AGENT_DIR": str(target.sandbox / "agent"),
        "PI_CODING_AGENT_SESSION_DIR": str(target.sandbox / "sessions"),
        "TERM": "xterm-256color",
        "LANG": "C.UTF-8",
        "CI": "1",
        "NO_PROXY": "*",
        "npm_config_offline": "true",
        "CARGO_NET_OFFLINE": "true",
    }


def common_args() -> list[str]:
    return [
        "--provider", PROVIDER,
        "--model", MODEL,
        "--thinking", "off",
        "--no-tools",
        "--no-extensions",
        "--no-skills",
        "--no-prompt-templates",
        "--no-context-files",
        "--session-id", "differential-prototype",
    ]


def pi_executable(pi: Path) -> list[str]:
    return [
        str(pi / "node_modules" / ".bin" / "tsx"),
        "--tsconfig", str(pi / "tsconfig.json"),
        str(pi / "packages" / "coding-agent" / "src" / "cli.ts"),
    ]


def write_result(directory: Path, *, boundary: str, scenario: str, target: str, target_sha: str, command: list[str], cwd: Path, env: dict[str, str], stdin: bytes, stdout: bytes, stderr: bytes, status: int, requests: list[object], sandbox: Path) -> None:
    if boundary not in {"batch", "rpc", "tui"}:
        raise ValueError("unknown observable boundary")
    if len(target_sha) != 40 or any(character not in "0123456789abcdef" for character in target_sha):
        raise ValueError("target SHA must be full lowercase hex")
    evidence = stdout + stderr + json.dumps(requests).encode()
    lowered = evidence.lower()
    for marker in (b"authorization:", b"bearer ", b"sk-"):
        if marker in lowered:
            raise SystemExit(f"refusing to save evidence containing credential marker {marker!r}")
    directory.mkdir(parents=True, exist_ok=True)
    (directory / "command.json").write_text(json.dumps(command, indent=2) + "\n")
    (directory / "stdin.raw").write_bytes(stdin)
    (directory / "stdout.raw").write_bytes(stdout)
    (directory / "stderr.raw").write_bytes(stderr)
    (directory / "status.txt").write_text(f"{status}\n")
    (directory / "requests.json").write_text(json.dumps(requests, indent=2, sort_keys=True) + "\n")
    (directory / "sessions.json").write_text(json.dumps(manifest(sandbox / "sessions"), indent=2, sort_keys=True) + "\n")
    (directory / "persistent.json").write_text(json.dumps(manifest(sandbox), indent=2, sort_keys=True) + "\n")
    artifact_names = ("stdin.raw", "stdout.raw", "stderr.raw", "status.txt", "requests.json", "sessions.json", "persistent.json")
    record = {
        "schema": 1,
        "boundary": boundary,
        "scenario": scenario,
        "target": target,
        "target_sha": target_sha,
        "pi_sha": PIN,
        "command": {"argv": command, "cwd": str(cwd), "env": dict(sorted(env.items()))},
        "returncode": status,
        "artifacts": {name: hashlib.sha256((directory / name).read_bytes()).hexdigest() for name in artifact_names},
    }
    (directory / "manifest.json").write_text(json.dumps(record, indent=2, sort_keys=True) + "\n")


def manifest(root: Path) -> dict[str, str]:
    return {
        str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in sorted(root.rglob("*"))
        if path.is_file()
    }


def verify_target(directory: Path) -> None:
    record = json.loads((directory / "manifest.json").read_text())
    if record.get("schema") != 1 or record.get("boundary") not in {"batch", "rpc", "tui"}:
        raise ValueError(f"invalid evidence manifest: {directory}")
    for name, expected in record.get("artifacts", {}).items():
        if hashlib.sha256((directory / name).read_bytes()).hexdigest() != expected:
            raise ValueError(f"artifact hash mismatch: {directory / name}")


def compare(artifacts: Path, channels: tuple[str, ...]) -> bool:
    verify_target(artifacts / "pi")
    verify_target(artifacts / "zedflow")
    differences = []
    for channel in channels:
        pi = (artifacts / "pi" / channel).read_bytes()
        zedflow = (artifacts / "zedflow" / channel).read_bytes()
        if pi != zedflow:
            differences.append(channel)
    verdict = {"equal": not differences, "differences": differences}
    (artifacts / "verdict.json").write_text(json.dumps(verdict, indent=2, sort_keys=True) + "\n")
    run_manifest = {
        "schema": 1,
        "pi_manifest_sha256": hashlib.sha256((artifacts / "pi/manifest.json").read_bytes()).hexdigest(),
        "zedflow_manifest_sha256": hashlib.sha256((artifacts / "zedflow/manifest.json").read_bytes()).hexdigest(),
        "verdict_sha256": hashlib.sha256((artifacts / "verdict.json").read_bytes()).hexdigest(),
        "channels": list(channels),
    }
    (artifacts / "manifest.json").write_text(json.dumps(run_manifest, indent=2, sort_keys=True) + "\n")
    print("equal" if not differences else f"DIFFER: {', '.join(differences)}")
    return not differences


def temporary_root() -> tempfile.TemporaryDirectory[str]:
    return tempfile.TemporaryDirectory(prefix="zedflow-differential-prototype-")


def self_check() -> None:
    with replay_server() as (url, requests):
        import urllib.request
        request = urllib.request.Request(
            url + "/chat/completions",
            data=b'{"model":"fixture-model"}',
            headers={"Authorization": f"Bearer {API_KEY}", "Content-Type": "application/json"},
        )
        assert b"deterministic" in urllib.request.urlopen(request).read()
        assert requests == [{"model": MODEL}]
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        for target, sha in (("pi", PIN), ("zedflow", "b" * 40)):
            sandbox = root / f"{target}-sandbox"
            (sandbox / "sessions").mkdir(parents=True)
            write_result(root / target, boundary="batch", scenario="self-check", target=target, target_sha=sha, command=[target], cwd=ROOT, env={"CI": "1"}, stdin=b"", stdout=b"same", stderr=b"", status=0, requests=[], sandbox=sandbox)
        assert compare(root, ("stdin.raw", "stdout.raw", "stderr.raw", "status.txt", "requests.json", "sessions.json", "persistent.json"))
        assert json.loads((root / "manifest.json").read_text())["schema"] == 1
        (root / "pi/stdout.raw").write_bytes(b"tampered")
        try:
            verify_target(root / "pi")
        except ValueError:
            pass
        else:
            raise AssertionError("tampered evidence was accepted")
    print("shared differential primitives: ok")


if __name__ == "__main__":
    self_check()
