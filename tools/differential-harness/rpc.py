#!/usr/bin/env python3
"""Capture exact Pi and Zedflow evidence at the real JSONL RPC boundary."""

from __future__ import annotations

import argparse
import json
import select
import subprocess
import time
from pathlib import Path

INPUT = b'{"id":"differential","type":"prompt","message":"Reply once."}\n'

from common import (
    ROOT,
    PIN,
    build_zedflow,
    common_args,
    compare,
    environment,
    make_target,
    pi_executable,
    prepare_pi,
    replay_server,
    temporary_root,
    write_result,
)


def exchange(command: list[str], env: dict[str, str]) -> tuple[bytes, bytes, int]:
    process = subprocess.Popen(
        command,
        cwd=ROOT,
        env=env,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    assert process.stdin and process.stdout and process.stderr
    process.stdin.write(INPUT)
    process.stdin.flush()
    output = bytearray()
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        ready, _, _ = select.select([process.stdout], [], [], 0.1)
        if not ready:
            if process.poll() is not None:
                break
            continue
        line = process.stdout.readline()
        if not line:
            break
        output.extend(line)
        try:
            if json.loads(line).get("type") == "agent_end":
                break
        except json.JSONDecodeError:
            pass
    else:
        process.kill()
        raise SystemExit("RPC probe timed out before agent_end")

    process.stdin.close()
    try:
        status = process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.terminate()
        status = process.wait(timeout=5)
    return bytes(output), process.stderr.read(), status


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifacts", type=Path, required=True)
    args = parser.parse_args()
    artifacts = args.artifacts.resolve()
    if artifacts.exists():
        raise SystemExit(f"refusing to mix evidence in existing directory: {artifacts}")

    with temporary_root() as temporary:
        root = Path(temporary)
        pi = prepare_pi(root / "frozen-pi")
        executables = {"pi": pi_executable(pi), "zedflow": [str(build_zedflow())]}
        with replay_server() as (url, requests):
            for name, executable in executables.items():
                requests.clear()
                target = make_target(name, executable, root, url)
                command = target.command + common_args() + ["--mode", "rpc"]
                env = environment(target)
                stdout, stderr, status = exchange(command, env)
                write_result(
                    artifacts / name,
                    boundary="rpc",
                    scenario="prompt-agent-end",
                    target=name,
                    target_sha=PIN if name == "pi" else subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
                    command=command,
                    cwd=ROOT,
                    env=env,
                    stdin=INPUT,
                    stdout=stdout,
                    stderr=stderr,
                    status=status,
                    requests=requests,
                    sandbox=target.sandbox,
                )

    if not compare(artifacts, ("stdin.raw", "stdout.raw", "stderr.raw", "status.txt", "requests.json", "sessions.json", "persistent.json")):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
