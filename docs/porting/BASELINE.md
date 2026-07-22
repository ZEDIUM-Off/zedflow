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

- the crate-based Zedflow lineage became the canonical `main` lineage;
- local uncommitted Zedflow work was preserved under `recovery/pre-main-realignment-20260720` before reconstruction;
- the unrelated inherited repository history was not merged and its remote branches were deleted.

New port work continues only on the independent crate-based Zedflow history.

## Port controller status

The V2 controller runs fresh, bounded Pi contexts per DAG unit and stores operational state outside the repository. It advances a dedicated automation integration ref only after ownership, exact-SHA, frozen-gitlink, validation, and compare-and-swap checks. `--continuous` chains accepted units immediately; no cron, Paseo, systemd service, or daemon schedules execution.

An optional external timer may invoke the read-only `monitor` command for progress reporting. The second bounded `coding-agent` wave has passed mechanical validation and frozen-Pi fidelity review. The next wave ports only dependency-light static runtime primitives and stdout guarding; session runtime, model-driven compaction, configuration, extensions, and TUI work remain deferred to later bounded assignments.
