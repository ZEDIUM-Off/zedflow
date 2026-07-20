---
name: zedflow
description: >-
  Product and repository context for Zedflow. Use before planning or editing
  Pi-port, orchestration, flow, runtime graph, or LangGraph surfaces.
disable-model-invocation: true
---

# Zedflow

## Required development order

1. **Current:** port the frozen Pi TypeScript packages completely and faithfully into matching `crates/zedflow-*` crates.
2. **Deferred:** after stage-1 fidelity is accepted on one SHA, implement the graph-native Zedflow product and LangGraph integration.

Do not introduce stage-2 product behavior while completing stage 1.

## Read first

1. `AGENTS.md`
2. `docs/planning/ZEDFLOW_MIGRATION_INTENT.md`
3. `docs/planning/PI_RUST_PORTING_RULES.md`
4. `docs/porting/BASELINE.md`

## Stage-1 mental model

The frozen `references/pi` gitlink is the semantic authority. Preserve package boundaries, runtime behavior, public contracts, errors, streaming, cancellation, sessions, tools, TUI behavior, and deterministic tests. Use explicit documented placeholders only where the porting rules permit them.

## Stage-2 product target

Zedflow remains a standalone graph-native coding-agent harness, not a maintained fork of the inherited Rust port. `CONTEXT.md` and `ZEDFLOW_MVP_PRD.md` describe this deferred target. The selected orchestration reference is LangGraph v1.2.6.
