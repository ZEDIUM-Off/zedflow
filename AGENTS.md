# AGENTS.md — Zedflow

Guidelines for AI coding agents working in this repository.

## Product identity and development order

Zedflow is a standalone, graph-native coding-agent harness developed in two strict stages:

1. **Current — Pi fidelity port:** port `references/pi/packages/` completely and faithfully into the matching `crates/zedflow-*` Rust crates.
2. **Deferred — Zedflow product:** only after the Pi port is complete and validated, implement Flow/Runtime Graph composition and LangGraph integration.

During stage 1, preserve Pi TypeScript runtime semantics. Do not introduce stage-2 behavior into ported crates.

Canonical references:

- Pi TypeScript reference submodule: `references/pi`
- LangGraph reference submodule: `references/langgraph`
- Product context: `CONTEXT.md`
- Current planning docs: `docs/planning/ZEDFLOW_MIGRATION_INTENT.md`, `docs/planning/ZEDFLOW_MVP_PRD.md`, `docs/planning/ZEDFLOW_WORKSPACE_ARCHITECTURE.md`

## Ground rules

- Distinguish the current Pi-to-Rust implementation stage from the final Zedflow product identity.
- Prefer small, verified changes.
- Do not delete files unless the user explicitly approves deletion.
- Never run destructive git commands such as `git reset --hard`, `git clean -fd`, or broad filesystem removal unless the user gives exact, explicit approval.
- Default branch is `main`; never use `master` in code or docs.

## Rust workflow

Use Cargo only.

After substantive code changes, run the smallest useful gate first, then widen:

```bash
cargo fmt --all --check
cargo check --workspace --all-targets
cargo test --workspace --all-targets --no-run
```

For expensive builds, prefer an external/offloaded builder when available. If local, set a temporary target dir to avoid polluting the repo:

```bash
export CARGO_TARGET_DIR="/tmp/zedflow-target"
export TMPDIR="/tmp/zedflow-tmp"
mkdir -p "$CARGO_TARGET_DIR" "$TMPDIR"
```

## Pi port coordination

`tools/pi-port-swarm/controller.py` is the stage-1 controller. Each unit runs in a fresh `pi -p` session and short-lived worktree; runtime state lives under `$XDG_STATE_HOME/zedflow-pi-port`. The controller alone selects units, verifies ownership/ancestry/gitlink/validations, and advances `refs/heads/automation/pi-port` by compare-and-swap. Workers never edit plan state; only an evidenced `PLAN_CHANGE` launches a fresh control-plane coordinator.

There is no scheduled port execution. `controller.py monitor` is deterministic and read-only; a separately managed timer may invoke it, but it must never dispatch work.

Port each Pi package into its matching crate, following the package split in `references/pi/packages/`:

- `zedflow-core`
- `zedflow-ai`
- `zedflow-agent`
- `zedflow-coding-agent`
- `zedflow-orchestrator`
- `zedflow-tui`
- `zedflow-tools`
- `zedflow-session`
- `zedflow-langgraph`

## CocoIndex

A local CocoIndex inventory utility lives under `tools/zedflow-index/`.

Run:

```bash
cd tools/zedflow-index
uv run cocoindex update main.py
```

Generated local DBs/venvs are ignored. The checked-in inventory output is `tools/zedflow-index/out/inventory.md`.
