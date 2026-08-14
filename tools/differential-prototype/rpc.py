#!/usr/bin/env python3
"""Prototype differential tool for the persistent JSONL RPC scope."""

from __future__ import annotations

import argparse
import json
import select
import subprocess
import time
from pathlib import Path

from common import (
    ROOT,
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
    process.stdin.write(b'{"id":"prototype","type":"prompt","message":"Reply once."}\n')
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
        for name, executable in executables.items():
            with replay_server() as (url, requests):
                target = make_target(name, executable, root, url)
                command = target.command + common_args() + ["--mode", "rpc"]
                stdout, stderr, status = exchange(command, environment(target))
                write_result(
                    artifacts / name,
                    command=command,
                    stdout=stdout,
                    stderr=stderr,
                    status=status,
                    requests=requests,
                    sandbox=target.sandbox,
                )

    if not compare(artifacts, ("stdout.raw", "stderr.raw", "status.txt", "requests.json", "sessions.json")):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
