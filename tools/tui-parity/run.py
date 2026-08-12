#!/usr/bin/env python3
"""Compare deterministic terminal frames from frozen Pi and Rust."""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PI = ROOT / "references" / "pi"
ORACLE = Path(__file__).with_name("frozen-pi-oracle.mjs")
FIXTURES = ORACLE.parent / "fixtures"
FIXTURE_NAMES = (
    "input-editing.json",
    "streaming.json",
    "tools-compaction.json",
    "commands.json",
    "overlays.json",
    "unicode-resize.json",
    "abort-error.json",
)
MARKER = b"ZEDFLOW_TUI_ORACLE:"


def require(program: str) -> str:
    path = shutil.which(program)
    if path is None:
        raise SystemExit(f"tui parity requires {program}")
    return path


def run(
    command: list[str],
    *,
    data: bytes | None = None,
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
) -> bytes:
    try:
        return subprocess.run(
            command,
            cwd=cwd,
            input=data,
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=env,
        ).stdout
    except subprocess.CalledProcessError as error:
        detail = error.stderr.decode(errors="replace").strip()
        raise SystemExit(f"{' '.join(command)} failed{': ' + detail if detail else ''}") from error


def frozen_workspace() -> tempfile.TemporaryDirectory[str]:
    temporary = tempfile.TemporaryDirectory(prefix="zedflow-tui-parity-")
    destination = Path(temporary.name) / "pi"
    shutil.copytree(PI, destination, ignore=shutil.ignore_patterns(".git", "node_modules"))
    shutil.copy2(ORACLE, destination / ORACLE.name)
    # Offline is intentional: parity may use only the tracked lockfile and the npm cache.
    run([require("npm"), "ci", "--offline", "--ignore-scripts"], cwd=destination)
    return temporary


def pi_oracle(workspace: Path, fixture: bytes) -> bytes:
    env = os.environ.copy()
    env["ZEDFLOW_ORACLE_CWD"] = str(ROOT)
    return run(
        [str(workspace / "node_modules/.bin/tsx"), ORACLE.name],
        data=fixture,
        cwd=workspace,
        env=env,
    )


def rust_oracle_binary() -> Path:
    output = run(
        [
            require("cargo"),
            "test",
            "-p",
            "zedflow-coding-agent",
            "--test",
            "tui-parity-rust",
            "--no-run",
            "--message-format=json",
        ],
        cwd=ROOT,
    )
    for line in output.splitlines():
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            continue
        target = message.get("target", {})
        if target.get("name") == "tui-parity-rust" and message.get("executable"):
            return Path(message["executable"])
    raise SystemExit("cargo did not report the tui-parity-rust test executable")


def rust_oracle(binary: Path, fixture: bytes) -> bytes:
    output = run(
        [str(binary), "rust_oracle_subprocess", "--ignored", "--nocapture", "--exact"],
        data=fixture,
        cwd=ROOT,
    )
    for line in output.splitlines():
        if MARKER in line:
            return line.split(MARKER, 1)[1]
    raise SystemExit("Rust oracle did not emit a normalized frame")


def compare(name: str, pi_output: bytes, rust_output: bytes) -> dict:
    pi_value = json.loads(pi_output)
    rust_value = json.loads(rust_output)
    if pi_value != rust_value:
        raise SystemExit(
            f"{name}: normalized Pi and Rust frames differ\n"
            f"Pi: {json.dumps(pi_value, ensure_ascii=False)}\n"
            f"Rust: {json.dumps(rust_value, ensure_ascii=False)}"
        )
    return pi_value


def self_check() -> None:
    with frozen_workspace() as temporary:
        workspace = Path(temporary) / "pi"
        result = json.loads(run([str(workspace / "node_modules/.bin/tsx"), ORACLE.name, "--self-check"], cwd=workspace))
    assert result == {"version": 2, "protocol": "component-oracle"}
    schema = json.loads((FIXTURES / "schema.json").read_text())
    assert schema["properties"]["version"]["const"] == 2
    print("tui parity Python/oracle protocol self-check: ok")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("fixture", nargs="?", type=Path)
    parser.add_argument("--all", action="store_true", help="compare every tracked acceptance fixture")
    parser.add_argument("--artifacts", type=Path, help="write equal normalized JSON frames here")
    parser.add_argument("--prepare", action="store_true", help="verify offline npm ci from package-lock.json")
    parser.add_argument("--self-check", action="store_true")
    args = parser.parse_args()

    if args.self_check:
        self_check()
        return
    if args.prepare:
        temporary = frozen_workspace()
        temporary.cleanup()
        print("frozen Pi offline npm ci: ok")
        return
    if args.all == (args.fixture is not None):
        parser.error("choose exactly one fixture or --all")

    paths = [FIXTURES / name for name in FIXTURE_NAMES] if args.all else [args.fixture]
    assert all(path is not None for path in paths)
    binary = rust_oracle_binary()
    temporary = frozen_workspace()
    try:
        workspace = Path(temporary.name) / "pi"
        for path in paths:
            assert path is not None
            fixture = path.read_bytes()
            equal = compare(path.name, pi_oracle(workspace, fixture), rust_oracle(binary, fixture))
            if args.artifacts:
                args.artifacts.mkdir(parents=True, exist_ok=True)
                (args.artifacts / path.name).write_text(
                    json.dumps(equal, ensure_ascii=False, indent=2) + "\n"
                )
            print(f"{path.name}: equal")
    finally:
        temporary.cleanup()


if __name__ == "__main__":
    main()
