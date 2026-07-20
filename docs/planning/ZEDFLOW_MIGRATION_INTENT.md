# Zedflow development intent

## Product identity

Zedflow is a standalone, graph-native coding-agent harness. It does not track the inherited Rust port as an upstream and is not maintained as a fork of it.

## Required development sequence

### Stage 1 — faithful Pi TypeScript port (current)

Port the frozen Pi TypeScript packages under `references/pi/packages/` into the matching `crates/zedflow-*` Rust crates.

Stage 1 preserves Pi runtime behavior, package boundaries, public contracts, error semantics, streaming, sessions, tools, TUI behavior, and deterministic tests as closely as Rust permits. The rules in `PI_RUST_PORTING_RULES.md` and the frozen Pi gitlink are authoritative.

No Zedflow-specific Flow, Runtime Graph, or LangGraph behavior is introduced during this stage.

### Stage 2 — Zedflow product (deferred)

Only after stage 1 is complete and fidelity is validated on one recorded SHA, build the graph-native Zedflow product described by `ZEDFLOW_MVP_PRD.md` and `CONTEXT.md`.

The current orchestration reference for that future stage is:

- `langgraph v1.2.6`: https://github.com/langchain-ai/langgraph/tree/1.2.6

## Why the stages are linked

The faithful Rust port supplies the provider, agent, coding-agent, tool, session, and TUI substrate that the later Zedflow runtime will compose. Stage 1 is therefore product foundation, not obsolete migration work; stage 2 must not redesign that substrate before fidelity is proven.
