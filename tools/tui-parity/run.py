#!/usr/bin/env python3
"""Run the tracked frozen-Pi TUI oracle and an optional Rust peer."""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PI = ROOT / "references" / "pi"
ORACLE = Path(__file__).with_name("frozen-pi-oracle.mjs")


def require(program: str) -> str:
    path = shutil.which(program)
    if path is None:
        raise SystemExit(f"tui parity requires {program}; install Node.js/npm from references/pi/package.json")
    return path


def run(command: list[str], *, data: bytes | None = None, cwd: Path | None = None) -> bytes:
    try:
        return subprocess.run(command, cwd=cwd, input=data, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout
    except subprocess.CalledProcessError as error:
        detail = error.stderr.decode(errors="replace").strip()
        raise SystemExit(f"{' '.join(command)} failed{': ' + detail if detail else ''}") from error


def frozen_workspace() -> tempfile.TemporaryDirectory[str]:
    temporary = tempfile.TemporaryDirectory(prefix="zedflow-tui-parity-")
    destination = Path(temporary.name) / "pi"
    shutil.copytree(PI, destination, ignore=shutil.ignore_patterns(".git", "node_modules"))
    shutil.copy2(ORACLE, destination / ORACLE.name)
    run([require("npm"), "ci", "--ignore-scripts"], cwd=destination)
    return temporary


def pi_oracle(fixture: bytes) -> bytes:
    temporary = frozen_workspace()
    try:
        workspace = Path(temporary.name) / "pi"
        return run([require("node"), ORACLE.name], data=fixture, cwd=workspace)
    finally:
        temporary.cleanup()


def self_check() -> None:
    result = json.loads(run([require("node"), str(ORACLE), "--self-check"]))
    assert result == {"version": 1, "protocol": "ok"}
    schema = json.loads((ORACLE.parent / "fixtures" / "schema.json").read_text())
    assert schema["properties"]["version"]["const"] == 1
    print("tui parity Python/oracle protocol self-check: ok")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("fixture", nargs="?", type=Path, help="JSON fixture to send to the frozen Pi oracle")
    parser.add_argument("--rust-command", nargs="+", help="optional Rust oracle command; stdout must equal normalized Pi JSON")
    parser.add_argument("--prepare", action="store_true", help="verify npm ci from the frozen package-lock in a disposable workspace")
    parser.add_argument("--self-check", action="store_true", help="check the dependency-free protocol scaffold")
    args = parser.parse_args()

    if args.self_check:
        self_check()
        return
    if args.prepare:
        temporary = frozen_workspace()
        temporary.cleanup()
        print("frozen Pi npm ci: ok")
        return
    if args.fixture is None:
        parser.error("fixture is required unless --self-check or --prepare is used")

    fixture = args.fixture.read_bytes()
    pi_output = pi_oracle(fixture)
    if args.rust_command:
        rust_output = run(args.rust_command, data=fixture, cwd=ROOT)
        if json.loads(pi_output) != json.loads(rust_output):
            raise SystemExit("normalized Pi and Rust frames differ")
    sys.stdout.buffer.write(pi_output)


if __name__ == "__main__":
    main()
