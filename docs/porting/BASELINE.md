# Pi TypeScript → Rust baseline

## Current stage

Zedflow is in **Stage 1: faithful one-to-one Pi TypeScript port**. The frozen source is the `references/pi` gitlink recorded by `tools/pi-port-swarm/dag.json`. Stage-2 Flow/Runtime Graph/LangGraph work is deferred.

This is the sole current human status document. Operational facts come from the integration ref, frozen gitlink, committed DAG, and external controller state exposed by:

```bash
python3 tools/pi-port-swarm/controller.py status
python3 tools/pi-port-swarm/manifest.py status
```

Historical plans, trackers, and fidelity decisions are evidence, not current status.

## Stage-1 package map

| Package | Mechanical closure at `automation/pi-port@c9cb37dd` | State |
|---|---:|---|
| AI | 247/247 inventoried rows, no missing target | mechanically closed; fidelity evidence retained |
| Agent | 45/45 inventoried rows, no missing target | mechanically closed; fidelity evidence retained |
| TUI | 1/61 exact targets present | incomplete; last review found Kitty CSI-u Unicode/Shift drift |
| Coding-agent | 237 mapped target paths still missing; 3 explicit disposition rows | incomplete; accepted code is preserved |
| Orchestrator | 0/13 targets present | not started |

These are target/disposition facts, not semantic-completion claims. Run the controller/manifest commands for the live projection after the integration ref advances.

## Current control-plane recovery

The Stage-1 recovery blueprint is `.agents/plans/pi-stage-1-port-recovery.md`. Recovery work reconciles the latest controller fixes with the accepted integration history, enforces manifest closure, restores bounded repair/replan/resume behavior, and removes non-Pi Stage-1 crates. The CAS-managed `automation/pi-port` ref remains the runtime integration authority and must not be checked out in the controller worktree.

The approved tail plan uses deterministic manifest gaps: 59 TUI, 237 Coding-agent, and 13 Orchestrator missing targets. The required order is TUI closure → Coding-agent closure → Orchestrator → final Stage-1 gate. The first TUI batch stopped because its component modules depend on foundation modules and Cargo dependencies outside its ownership.

The user approved the TUI dependency mapping on 2026-07-27: exact pins `markdown 1.0.0`, `icu_properties 2.2.0`, `icu_segmenter 2.2.0`, and `emojis 0.9.0`, while retaining Pi's custom terminal Markdown rendering and grapheme-width policy. Dispatch resumes only after the control protocol, dependency-first TUI DAG, and external runtime identities are migrated together.

## Stage-1 exit gate

Stage 1 is complete only when all of the following hold on one recorded commit SHA:

1. Every frozen Pi source/test has an existing one-to-one Rust target or an approved explicit disposition.
2. No `dependency-arbitration`, unexplained missing row, convenience placeholder, or implementation-gap ignore remains.
3. Public contracts, errors, streaming, cancellation, sessions, tools, CLI, and TUI behavior pass independent Pi-fidelity review.
4. `cargo fmt --all --check`, `cargo check --workspace --all-targets`, and executed `cargo test --workspace --all-targets` pass.
5. Independent fidelity and Rust-quality reviews accept the same SHA.
6. The accepted integration SHA is promoted to `main`, and the gate passes again on the promoted SHA.

Only then may Stage 2 begin.

## History

The inherited monolithic Rust history is not part of this lineage. During the 2026-07-20 stabilization, the package-aligned Zedflow lineage became canonical and earlier local work was preserved under recovery refs. Port execution now advances only the dedicated automation integration ref through validated compare-and-swap updates.
