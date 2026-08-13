#!/usr/bin/env python3
"""Real frozen-Pi versus Zedflow interactive TUI fidelity runner.

Each semantic fixture sends raw PTY input to both complete CLIs.  Captures are
fed to one xterm-headless decoder; no components, handlers, transcript events,
or expected render strings are fabricated by this suite.
"""
from __future__ import annotations
import argparse, json, os, pty, select, shutil, signal, subprocess, sys, tempfile, time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PI = ROOT / "references/pi"
PIN = "2b00dade7cec918aefb025c8b7a4fa304a30acdd"
HERE = Path(__file__).parent
FIXTURES = HERE / "fixtures"


def checked(command, **kwargs): return subprocess.run(command, check=True, **kwargs)
def git(*args): return subprocess.check_output(["git", "-C", PI, *args], text=True).strip()
def require(name):
    if not shutil.which(name): raise SystemExit(f"tui fidelity requires {name}")

def frozen_pi_workspace(temp: Path) -> Path:
    if git("rev-parse", "HEAD") != PIN: raise SystemExit(f"frozen Pi must be {PIN}")
    # archive reads the exact git object, so the known dirty docs file cannot affect Pi.
    destination = temp / "pi"; destination.mkdir()
    with (destination / "archive.tar").open("wb") as target:
        checked(["git", "-C", PI, "archive", PIN], stdout=target)
    checked(["tar", "-xf", "archive.tar"], cwd=destination)
    checked(["npm", "ci", "--offline", "--ignore-scripts"], cwd=destination, stdout=subprocess.DEVNULL)
    return destination

def environment(home: Path) -> dict[str, str]:
    keep = {"PATH": os.environ["PATH"], "TERM": "xterm-256color", "COLORTERM": "truecolor", "HOME": str(home), "PI_CODING_AGENT_DIR": str(home / "agent"), "PI_CODING_AGENT_SESSION_DIR": str(home / "sessions"), "CI": "1", "NO_PROXY": "*", "npm_config_offline": "true", "CARGO_NET_OFFLINE": "true"}
    return keep

def capture(command: list[str], fixture: dict, cwd: Path, home: Path, output: Path) -> None:
    import fcntl, struct, termios

    env = environment(home); home.mkdir(parents=True, exist_ok=True)
    agent_dir = home / "agent"
    agent_dir.mkdir(parents=True, exist_ok=True)
    (agent_dir / "settings.json").write_text(json.dumps({
        "defaultProjectTrust": "always",
        "quietStartup": True,
    }))
    pid, fd = pty.fork()
    if pid == 0:
        os.chdir(cwd); os.environ.clear(); os.environ.update(env); os.execvpe(command[0], command, os.environ)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", fixture["rows"], fixture["columns"], 0, 0))
    data = bytearray(); deadline = time.monotonic() + fixture.get("timeout", 8)
    try:
        for event in fixture["events"]:
            time.sleep(event.get("wait", .25))
            if event["type"] == "input":
                os.write(fd, event["data"].encode())
                # Interactive applications may emit startup queries immediately
                # after fork; always drain before the next raw action.
                while select.select([fd], [], [], 0)[0]:
                    try: data.extend(os.read(fd, 65536))
                    except OSError: break
            elif event["type"] == "resize":
                fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", event["rows"], event["columns"], 0, 0))
            if event["type"] == "checkpoint":
                end = time.monotonic() + event.get("settle", .5)
                while time.monotonic() < end:
                    ready, _, _ = select.select([fd], [], [], .05)
                    if ready:
                        try: data.extend(os.read(fd, 65536))
                        except OSError: break
        while time.monotonic() < deadline:
            ready, _, _ = select.select([fd], [], [], .05)
            if ready:
                try: data.extend(os.read(fd, 65536))
                except OSError: break
            else: break
    finally:
        try: os.kill(pid, signal.SIGTERM)
        except ProcessLookupError: pass
        try: os.waitpid(pid, 0)
        except ChildProcessError: pass
    output.write_bytes(data)

def decode(raw: Path, fixture: dict, node_modules: Path) -> object:
    # The decoder is copied beside the frozen dependency tree, guaranteeing both
    # captures use this exact one xterm-headless implementation.
    decoder = node_modules.parent / "zedflow-fidelity-decoder.mjs"
    shutil.copy2(HERE / "decoder.mjs", decoder)
    result = subprocess.check_output(["node", decoder, str(fixture["columns"]), str(fixture["rows"]), raw], cwd=node_modules.parent, text=True)
    return json.loads(result)

def main():
    parser = argparse.ArgumentParser(description=__doc__); parser.add_argument("--all", action="store_true"); parser.add_argument("fixture", nargs="?"); parser.add_argument("--artifacts", type=Path); args = parser.parse_args()
    if args.all == bool(args.fixture): parser.error("choose one fixture or --all")
    require("node"); require("npm"); require("cargo")
    paths = sorted(FIXTURES.glob("*.json")) if args.all else [FIXTURES / args.fixture]
    differences = 0
    with tempfile.TemporaryDirectory(prefix="zedflow-tui-fidelity-") as temp_s:
        temp = Path(temp_s); pi = frozen_pi_workspace(temp)
        target = Path(os.environ.get("CARGO_TARGET_DIR", "/tmp/zedflow-target"))
        checked(["cargo", "build", "-p", "zedflow-coding-agent"], cwd=ROOT, env={**os.environ, "CARGO_NET_OFFLINE":"true", "CARGO_TARGET_DIR":str(target)}, stdout=subprocess.DEVNULL)
        rust = target / "debug" / "zedflow-coding-agent"
        for path in paths:
            fixture = json.loads(path.read_text()); base = args.artifacts / path.stem if args.artifacts else temp / path.stem; base.mkdir(parents=True, exist_ok=True)
            capture([
                str(pi / "node_modules/.bin/tsx"), "--tsconfig", str(pi / "tsconfig.json"),
                str(pi / "packages/coding-agent/src/cli.ts"),
            ], fixture, ROOT, base / "pi-home", base / "pi.raw")
            capture([str(rust)], fixture, ROOT, base / "rust-home", base / "rust.raw")
            pi_frame = decode(base / "pi.raw", fixture, pi / "node_modules")
            rust_frame = decode(base / "rust.raw", fixture, pi / "node_modules")
            (base / "pi.json").write_text(json.dumps(pi_frame, indent=2) + "\n"); (base / "zedflow.json").write_text(json.dumps(rust_frame, indent=2) + "\n")
            if pi_frame != rust_frame:
                differences += 1
                (base / "diff.txt").write_text("Pi and Zedflow terminal cells differ; inspect pi.json and zedflow.json.\n")
                print(f"{path.name}: DIFFER ({base})")
            else: print(f"{path.name}: equal")
    if differences:
        raise SystemExit(f"{differences} real TUI fidelity fixture(s) differ")
if __name__ == "__main__": main()
