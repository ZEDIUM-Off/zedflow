# Pi TypeScript → Rust baseline

## Current stage

Zedflow is in **stage 1: faithful Pi TypeScript port**. The frozen source is the `references/pi` gitlink recorded by `tools/pi-port-swarm/dag.json`. Stage-2 Flow/Runtime Graph/LangGraph work is deferred.

The current committed baseline contains substantial `zedflow-ai` and `zedflow-agent` ports. The remaining Pi package crates are not yet complete.

## Stage-1 exit gate

Stage 1 is complete only when all of the following hold on one recorded commit SHA:

1. Every frozen Pi package source and deterministic test is ported to its matching crate, or has an explicit blocker allowed by `docs/planning/PI_RUST_PORTING_RULES.md`.
2. Public contracts, errors, streaming, cancellation, sessions, tools, and TUI behavior pass independent Pi-fidelity review.
3. `cargo fmt --all --check`, `cargo check --workspace --all-targets`, and `cargo test --workspace --all-targets` pass.
4. Ignored tests and `PORT PLACEHOLDER` sites have an explicit, current disposition; no convenience placeholder counts as completion.
5. Independent fidelity and Rust-quality reviews accept the same SHA.

Only then may stage-2 implementation begin.

## GitHub history realignment

The repository originally published an inherited monolithic Rust port. Zedflow later restarted the port as package-aligned crates with an independent Git history.

During the 2026-07-20 stabilization:

- the former GitHub `main` at `052ed642659202538b10d02d718158d1c642d50f` was preserved as `archive/pi-rust-main-052ed642`;
- the crate-based Zedflow lineage was selected as the canonical `main` lineage;
- local uncommitted work was preserved under `recovery/pre-main-realignment-20260720` before reconstruction;
- unrelated histories were not merged.

The archive is historical evidence, not an upstream. New port work continues only on the crate-based lineage.

## Swarm status

Automated scheduling is paused while the clean baseline and DAG state are reconciled. The persistent coordinator/worker model remains the intended port mechanism. Scheduling may resume only after a manual pilot proves task selection, one-writer ownership, exact-SHA validation, review, and compare-and-swap integration.
