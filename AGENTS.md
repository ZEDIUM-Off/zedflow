# AGENTS.md — Zedflow

Guidelines for AI coding agents working in this repository.

## Product identity

Zedflow is a standalone, graph-native coding-agent harness. It moves orchestration toward explicit Flow/Runtime Graph composition with LangGraph as the reference runtime.

Canonical references:

- Pi TypeScript reference submodule: `references/pi`
- LangGraph reference submodule: `references/langgraph`
- Product context: `CONTEXT.md`
- Current planning docs: `docs/planning/ZEDFLOW_MIGRATION_INTENT.md`, `docs/planning/ZEDFLOW_MVP_PRD.md`, `docs/planning/ZEDFLOW_WORKSPACE_ARCHITECTURE.md`

## Ground rules

- Do not describe this repo as Pi Rust or as a drop-in Pi replacement.
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

## Cleanup direction

This repo is being cleaned from an inherited Pi Rust port workspace into a focused Zedflow base. Treat old drop-in certification, parity ledgers, swarm/operator automation, extension-corpus artifacts, and port-management docs as removable unless they are directly needed by the runtime or current tests.

The root crate is now a temporary quarry. New product code should land in `crates/`, following the package split in `references/pi/packages/`:

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
