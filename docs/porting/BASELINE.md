# Pi TypeScript → Rust baseline

## Current stage

Zedflow is in **Stage 1: incomplete semantic Pi TypeScript → Rust port**. The frozen source remains `references/pi@2b00dade7cec918aefb025c8b7a4fa304a30acdd`. The completed mechanical DAG proved file presence, not product fidelity. Stage-2 Flow/Runtime Graph/LangGraph work and promotion to `main` remain forbidden.

This is the sole current human status document. Operational facts come from the CAS integration ref, frozen gitlink, committed DAG, and external controller state:

```bash
python3 tools/pi-port-swarm/controller.py status
python3 tools/pi-port-swarm/manifest.py status
```

## Stage-1 package map

| Package | Mechanical inventory | Semantic state |
|---|---:|---|
| AI | 247/247 rows accounted for | open: placeholder error/transport paths and deterministic ignored tests |
| Agent | 45/45 rows accounted for | open: four ignored behaviors |
| TUI | 61/61 rows accounted for | invalid: dead components and skeletal terminal/runtime |
| Coding-agent | 332 rows accounted for | invalid: 70 marker-only sources, 164 vacuous tests, unwired modes |
| Orchestrator | 13/13 rows accounted for | invalid: marker-only runtime and no executable Rust tests |

Audit evidence is retained in `.pi-subagents/artifacts/44c53eb4_pi-port-scout_{0,1,2}_output.md`. `automation/pi-port@f83a96fe` remains mechanical evidence only.

## Semantic recovery

The approved replacement blueprint is `.agents/plans/pi-stage-1-port-recovery.md`. Its fresh `SEM-*` DAG starts with a strict semantic closure guard, then executes:

```text
TUI terminal/runtime/components/native gates
→ Coding-agent core/interactive/CLI/executable tests
→ Orchestrator
→ AI and Agent residuals
→ workspace, fidelity, Rust-quality and end-user gates
```

The user approved exact `crossterm = "=0.29.0"` for safe raw mode, size/restoration and portable console services only. Pi's byte framing, Kitty parser, renderer, Markdown ANSI/OSC-8, Unicode policy and component model remain local Rust ports. The existing exact pins remain `markdown = "=1.0.0"`, `icu_properties = "=2.2.0"`, `icu_segmenter = "=2.2.0"`, and `emojis = "=0.9.0"`; workspace `base64 = "0.22"` may be reused. No Ratatui or alternate-screen redesign is authorized.

Additional native macOS/Windows dependencies, unsafe boundaries, and Rust PTY/ConPTY dev-dependencies remain arbitration-gated. Frozen Node/@xterm-headless is the differential visual oracle until separately replaced.

The controller now permits a reviewed CAS upgrade from a fully completed DAG to a fresh, non-reused, reachable DAG while preserving all external runtime history. The semantic controller candidate must descend from `f83a96fe`, preserve the Pi gitlink, pass control tests, atomically upgrade `automation/pi-port`, and only then restart dispatch.

## Stage-1 exit gate

Stage 1 is complete only when all of the following hold on one recorded commit SHA:

1. Every frozen source/test has a reachable semantic implementation/executable test or exact approved disposition; markers and empty tests do not count.
2. No dead mapped module, runtime placeholder/no-op, unwired CLI mode, unexplained ignore, or pending dependency arbitration remains.
3. Default TUI, print/text/json, RPC, sessions, tools, extensions, skills, themes, package management and Orchestrator run end to end with differential Pi tests.
4. Provider transport, errors, streaming, cancellation, compaction and persistence pass independent fidelity review.
5. Workspace fmt/check/executed tests and strict semantic manifest pass.
6. Independent end-user, fidelity and Rust-quality reviews accept the same immutable SHA.
7. That SHA is explicitly promoted to `main`, and all gates pass again there.

Only then may Stage 2 begin.

## History

The completed mechanical DAG and its runtime records are preserved, not reset. Semantic execution advances only the CAS-managed `automation/pi-port` ref through fresh IDs and validated worktrees.
