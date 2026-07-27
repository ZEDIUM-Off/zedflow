# Pi TypeScript → Rust baseline

## Current stage

Zedflow is in **Stage 1: incomplete semantic Pi TypeScript → Rust port**. The frozen source is the `references/pi` gitlink recorded by `tools/pi-port-swarm/dag.json`. The completed automation DAG proved file presence and selected tests, but a 2026-07-27 end-user audit invalidated its semantic completion claim. Stage-2 Flow/Runtime Graph/LangGraph work remains forbidden.

This is the sole current human status document. Operational facts come from the integration ref, frozen gitlink, committed DAG, and external controller state exposed by:

```bash
python3 tools/pi-port-swarm/controller.py status
python3 tools/pi-port-swarm/manifest.py status
```

Historical plans, trackers, and fidelity decisions are evidence, not current status.

## Stage-1 package map

| Package | Mechanical inventory | Semantic state |
|---|---:|---|
| AI | 247/247 rows accounted for | open: explicit transport placeholder, placeholder error path, and 24 ignored tests require disposition/implementation |
| Agent | 45/45 rows accounted for | open: four ignored behaviors require implementation or exact justified disposition |
| TUI | 61/61 rows accounted for | **invalid**: 23 source files are dead/unwired; terminal/TUI/components are skeletal; no full-screen runtime |
| Coding-agent | 332 rows accounted for | **invalid**: 70/170 source files are marker-only stubs; 164/184 test targets contain no executable test |
| Orchestrator | 13/13 rows accounted for | **invalid**: all 12 runtime modules are marker-only stubs; no Rust tests |

Mechanical mapping is not completion evidence. Audit artifacts: `.pi-subagents/artifacts/44c53eb4_pi-port-scout_{0,1,2}_output.md`.

## Current control-plane recovery

`automation/pi-port@f83a96fe` is retained as mechanical evidence, not an accepted Stage-1 product. Its completed DAG must not trigger promotion or Stage 2. A local test candidate `69dc24a8` wires a minimal canonical-input terminal chat so the Rust runtime can be exercised, but explicitly does not satisfy Pi TUI parity.

The recovery must first harden closure checks against marker-only modules/tests, dead mapped modules, ignored implementation gaps, and unwired CLI modes. It must then replace every stub with the frozen Pi behavior in the required order: TUI/runtime surface → Coding-agent interactive/runtime surface → Orchestrator → AI/Agent residuals → full differential and end-user gates. The approved exact TUI dependency pins remain `markdown 1.0.0`, `icu_properties 2.2.0`, `icu_segmenter 2.2.0`, and `emojis 0.9.0`.

## Stage-1 exit gate

Stage 1 is complete only when all of the following hold on one recorded commit SHA:

1. Every frozen Pi source/test has an existing one-to-one Rust implementation/test or an approved explicit disposition; marker constants and empty test targets do not count.
2. No `dependency-arbitration`, dead mapped module, convenience placeholder, explicit runtime no-op, unwired CLI mode, or implementation-gap ignore remains.
3. Default interactive TUI, print/text/json, RPC, sessions, tools, extensions, skills, themes, package management, and Orchestrator execute end to end with differential Pi tests.
4. Provider transport, errors, streaming, cancellation, compaction, and persistence pass independent Pi-fidelity review, including every formerly ignored non-live test.
5. `cargo fmt --all --check`, `cargo check --workspace --all-targets`, and executed `cargo test --workspace --all-targets` pass with no unexplained ignore.
6. Independent end-user, fidelity, and Rust-quality reviews accept the same immutable SHA.
7. The accepted integration SHA is promoted to `main`, and every gate passes again on the promoted SHA.

Only then may Stage 2 begin.

## History

The inherited monolithic Rust history is not part of this lineage. During the 2026-07-20 stabilization, the package-aligned Zedflow lineage became canonical and earlier local work was preserved under recovery refs. Port execution now advances only the dedicated automation integration ref through validated compare-and-swap updates.
