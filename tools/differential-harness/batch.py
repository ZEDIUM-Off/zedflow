#!/usr/bin/env python3
"""Capture exact Pi and Zedflow evidence at the real batch CLI boundary."""

from __future__ import annotations

import argparse
import subprocess
from pathlib import Path

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
                command = target.command + common_args() + ["--print", "Reply once."]
                env = environment(target)
                result = subprocess.run(command, cwd=ROOT, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30)
                write_result(artifacts / name, boundary="batch", scenario="deterministic-reply", target=name, target_sha=PIN if name == "pi" else subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(), command=command, cwd=ROOT, env=env, stdin=b"", stdout=result.stdout, stderr=result.stderr, status=result.returncode, requests=requests, sandbox=target.sandbox)

    if not compare(artifacts, ("stdin.raw", "stdout.raw", "stderr.raw", "status.txt", "requests.json", "sessions.json", "persistent.json")):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
