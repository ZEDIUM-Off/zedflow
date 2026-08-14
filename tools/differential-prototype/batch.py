#!/usr/bin/env python3
"""Prototype differential tool for one-shot print/JSON CLI scopes."""

from __future__ import annotations

import argparse
import subprocess
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
                command = target.command + common_args() + ["--print", "Reply once."]
                result = subprocess.run(
                    command,
                    cwd=ROOT,
                    env=environment(target),
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                    timeout=30,
                )
                write_result(
                    artifacts / name,
                    command=command,
                    stdout=result.stdout,
                    stderr=result.stderr,
                    status=result.returncode,
                    requests=requests,
                    sandbox=target.sandbox,
                )

    if not compare(artifacts, ("stdout.raw", "stderr.raw", "status.txt", "requests.json", "sessions.json")):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
