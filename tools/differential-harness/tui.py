#!/usr/bin/env python3
"""Capture exact Pi and Zedflow evidence at the real PTY/TUI boundary."""
from __future__ import annotations

import argparse
import fcntl
import os
import pty
import select
import signal
import struct
import subprocess
import termios
import time
from pathlib import Path

from common import (
    PIN,
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

INPUT = b"Reply once.\r"
ROWS, COLUMNS = 24, 80


def exchange(command: list[str], env: dict[str, str]) -> tuple[bytes, int]:
    pid, descriptor = pty.fork()
    if pid == 0:
        os.chdir(ROOT)
        os.execvpe(command[0], command, env)
    fcntl.ioctl(descriptor, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLUMNS, 0, 0))
    output = bytearray()
    sent = interrupted = False
    deadline = time.monotonic() + 30
    status: int | None = None
    try:
        while time.monotonic() < deadline:
            ready, _, _ = select.select([descriptor], [], [], 0.1)
            if ready:
                try:
                    chunk = os.read(descriptor, 65536)
                except OSError:
                    chunk = b""
                if not chunk:
                    break
                output.extend(chunk)
            if not sent and time.monotonic() > deadline - 29:
                os.write(descriptor, INPUT)
                sent = True
            if not interrupted and b"deterministic reply" in output:
                os.write(descriptor, b"\x03")
                interrupted = True
            waited, raw = os.waitpid(pid, os.WNOHANG)
            if waited:
                status = os.waitstatus_to_exitcode(raw)
                break
        if status is None:
            os.kill(pid, signal.SIGTERM)
            _, raw = os.waitpid(pid, 0)
            status = os.waitstatus_to_exitcode(raw)
    finally:
        os.close(descriptor)
    return bytes(output), status


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
                command = target.command + common_args()
                env = environment(target)
                stdout, status = exchange(command, env)
                write_result(
                    artifacts / name,
                    boundary="tui",
                    scenario=f"pty-{ROWS}x{COLUMNS}-deterministic-reply",
                    target=name,
                    target_sha=PIN if name == "pi" else subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
                    command=command,
                    cwd=ROOT,
                    env=env,
                    stdin=INPUT + b"\x03",
                    stdout=stdout,
                    stderr=b"",
                    status=status,
                    requests=requests,
                    sandbox=target.sandbox,
                )

    if not compare(artifacts, ("stdin.raw", "stdout.raw", "status.txt", "requests.json", "sessions.json", "persistent.json")):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
