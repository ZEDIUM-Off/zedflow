#!/usr/bin/env python3
"""Reproduce confirmed Pi↔Zedflow divergences from immutable source revisions.

This deliberately checks the frozen Pi and product baseline instead of the
worktree, so a later port change cannot silently bless a known red result.
"""
from __future__ import annotations

import argparse
import subprocess
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PI = ROOT / "references" / "pi"
PIN = "2b00dade7cec918aefb025c8b7a4fa304a30acdd"
BASELINE = "e91b44be9c897aef63c84c34b4e14b387a8141a7"


@dataclass(frozen=True)
class Fixture:
    name: str
    pi_path: str
    pi_text: str
    rust_path: str
    rust_text: str


def show(repository: Path, revision: str, path: str) -> str:
    return subprocess.check_output(
        ["git", "-C", str(repository), "show", f"{revision}:{path}"], text=True
    )


FIXTURES = (
    Fixture("interactive-user-update", "packages/coding-agent/src/modes/interactive/interactive-mode.ts", 'this.streamingComponent && event.message.role === "assistant"', "crates/zedflow-coding-agent/src/modes/interactive/interactive-mode.rs", "transcript.update_assistant(&message);"),
    Fixture("cli-pi-command", "packages/coding-agent/package.json", '"pi": "dist/cli.js"', "crates/zedflow-coding-agent/Cargo.toml", 'name = "zedflow-coding-agent"'),
    Fixture("cli-version", "packages/coding-agent/package.json", '"version": "0.80.3"', "crates/zedflow-coding-agent/Cargo.toml", 'version = "0.1.0"'),
    Fixture("rpc-bundled-entry", "packages/orchestrator/src/rpc-process.ts", 'require.resolve("@earendil-works/pi-coding-agent/rpc-entry")', "crates/zedflow-orchestrator/src/rpc-process.rs", 'unwrap_or_else(|_| "pi".into())'),
    Fixture("serve-signal-cleanup", "packages/orchestrator/src/serve.ts", 'process.on("SIGTERM"', "crates/zedflow-orchestrator/src/serve.rs", "start_radius().await?"),
    Fixture("ipc-untyped-json", "packages/orchestrator/src/ipc/protocol.ts", "JSON.parse(line) as OrchestratorRequest", "crates/zedflow-orchestrator/src/ipc/protocol.rs", "serde_json::from_str(line)"),
    Fixture("ipc-client-socket-path", "packages/orchestrator/src/ipc/client.ts", "before a response was received: ${socketPath}", "crates/zedflow-orchestrator/src/ipc/client.rs", '"Orchestrator socket closed before a response was received",'),
    Fixture("ipc-stale-socket-errors", "packages/orchestrator/src/ipc/server.ts", "const isLive = await isSocketLive(socketPath);", "crates/zedflow-orchestrator/src/ipc/server.rs", "Err(_) => std::fs::remove_file(path),"),
    Fixture("radius-retry-jitter", "packages/orchestrator/src/radius.ts", "Math.random()", "crates/zedflow-orchestrator/src/radius.rs", "2u64.saturating_pow(failures.saturating_sub(1))"),
    Fixture("radius-url-resolution", "packages/orchestrator/src/radius.ts", "new URL(DEFAULT_ORCHESTRATOR_BASE_PATH, getRadiusUrl()).toString()", "crates/zedflow-orchestrator/src/radius.rs", 'format!("{}/v1/", radius_url().trim_end_matches(\'/\'))'),
    Fixture("supervisor-stop-finally-removal", "packages/orchestrator/src/supervisor.ts", "finally {", "crates/zedflow-orchestrator/src/supervisor.rs", "storage::remove_instance(id)?;\n        cleanup?;"),
)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--case", choices=("all", *(fixture.name for fixture in FIXTURES)), default="all")
    args = parser.parse_args()
    selected = FIXTURES if args.case == "all" else tuple(fixture for fixture in FIXTURES if fixture.name == args.case)
    for fixture in selected:
        pi = show(PI, PIN, fixture.pi_path)
        rust = show(ROOT, BASELINE, fixture.rust_path)
        if fixture.pi_text not in pi or fixture.rust_text not in rust:
            raise SystemExit(f"fixture no longer reproduces: {fixture.name}")
        print(f"confirmed red: {fixture.name}")


if __name__ == "__main__":
    main()
