# Zedflow Rust workspace architecture

Zedflow is rebuilding Pi TypeScript as a clean Rust workspace instead of continuing the inherited monolithic Rust port.

## Stage 1 mapping — current

The package split in `references/pi/packages/` is the source of truth:

| Pi reference package | Zedflow crate | Stage-1 responsibility |
| --- | --- | --- |
| `packages/ai` | `crates/zedflow-ai` | Providers, model registry, messages, streaming, auth |
| `packages/agent` | `crates/zedflow-agent` | Agent loop, harness, session semantics used by the agent package |
| `packages/coding-agent` | `crates/zedflow-coding-agent` | Coding-agent assembly and CLI behavior |
| `packages/orchestrator` | `crates/zedflow-orchestrator` | Faithful port of the Pi orchestrator package |
| `packages/tui` | `crates/zedflow-tui` | Terminal UI primitives and behavior |
| shared substrate | `crates/zedflow-core` | Shared errors, IDs, config primitives, common types |
| coding-agent tools | `crates/zedflow-tools` | Built-in tool definitions and execution behavior |
| coding-agent sessions | `crates/zedflow-session` | Session persistence and tree behavior |

Current status:

- `zedflow-ai` and `zedflow-agent` contain substantial ports.
- `zedflow-coding-agent`, `zedflow-orchestrator`, `zedflow-tui`, `zedflow-tools`, and `zedflow-session` are workspace skeletons awaiting their package ports.
- New stage-1 code belongs in the matching crate. Do not recreate a monolithic root crate.

## Stage 2 mapping — deferred

`crates/zedflow-langgraph` and Zedflow-specific orchestration, Runtime Graph, Flow, checkpoint binding, and sidecar behavior are stage 2. They remain skeletons until the complete Pi port passes the stage-1 exit gate.
