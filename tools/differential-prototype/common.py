#!/usr/bin/env python3
"""Throwaway shared primitives for real Pi ↔ Zedflow differential probes."""

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


def write_result(directory: Path, *, command: list[str], stdout: bytes, stderr: bytes, status: int, requests: list[object], sandbox: Path) -> None:
    evidence = stdout + stderr + json.dumps(requests).encode()
    lowered = evidence.lower()
    for marker in (b"authorization:", b"bearer ", b"sk-"):
        if marker in lowered:
            raise SystemExit(f"refusing to save evidence containing credential marker {marker!r}")
    directory.mkdir(parents=True, exist_ok=True)
    (directory / "command.json").write_text(json.dumps(command, indent=2) + "\n")
    (directory / "stdout.raw").write_bytes(stdout)
    (directory / "stderr.raw").write_bytes(stderr)
    (directory / "status.txt").write_text(f"{status}\n")
    (directory / "requests.json").write_text(json.dumps(requests, indent=2, sort_keys=True) + "\n")
    (directory / "sessions.json").write_text(json.dumps(manifest(sandbox / "sessions"), indent=2, sort_keys=True) + "\n")


def manifest(root: Path) -> dict[str, str]:
    return {
        str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in sorted(root.rglob("*"))
        if path.is_file()
    }


def compare(artifacts: Path, channels: tuple[str, ...]) -> bool:
    differences = []
    for channel in channels:
        pi = (artifacts / "pi" / channel).read_bytes()
        zedflow = (artifacts / "zedflow" / channel).read_bytes()
        if pi != zedflow:
            differences.append(channel)
    (artifacts / "verdict.json").write_text(json.dumps({"equal": not differences, "differences": differences}, indent=2) + "\n")
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
    print("shared differential primitives: ok")


if __name__ == "__main__":
    self_check()
