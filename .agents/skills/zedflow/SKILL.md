---
name: zedflow
description: >-
  Product-intent and repo-context skill for Zedflow. Use when you need to understand
  the repo's identity, canonical LangGraph baseline, and graph-native replacement seams before planning or editing.
disable-model-invocation: true
---

# Zedflow

## Use This Skill When

- You are working in this repo and need the product-intent context before making changes.
- You are planning or reviewing work that may affect orchestration, flows, interrupts, subgraphs, or repo identity.
- You need the canonical reference baseline for this repo.

## Read First

1. `AGENTS.md`
2. `docs/planning/ZEDFLOW_MIGRATION_INTENT.md`
3. This skill

## Product Identity

Zedflow is a standalone, graph-native coding-agent harness. It is not Pi Rust, not a drop-in Pi replacement, and not a fork-maintenance project.

Canonical reference baseline:
- `langgraph v1.2.6` — <https://github.com/langchain-ai/langgraph/tree/1.2.6>

## Default Mental Model

Prefer small changes that strengthen Zedflow's own flow/runtime graph model.

## Surface Map

| Surface | Default stance |
| --- | --- |
| Providers and auth | Build only what Zedflow needs |
| Model registry and config loading | Keep minimal until flows require more |
| Built-in tools and execution guards | Prefer explicit runtime boundaries |
| Sessions and persistence | Align with graph-runtime durability needs |
| CLI / TUI / RPC substrate | Add seams only when flow UX needs them |
| Agent loop and orchestration | Target-kernel zone |
| Subgraphs / interrupts / flow composition | Target-kernel zone |
