# Pi TypeScript → Rust baseline

## Current stage

Zedflow is in **Stage 1: semantic Pi TypeScript → Rust port awaiting explicit human promotion approval**. The replacement final-gate validator accepted the immutable product SHA `7ae6374063da1f60cc5767d0a4e51d907cfc61d6`; the frozen source remains `references/pi@2b00dade7cec918aefb025c8b7a4fa304a30acdd`. This later documentation commit records evidence only and was not the product SHA tested. Promotion to `main` requires explicit human approval, and Stage-2 Flow/Runtime Graph/LangGraph work remains forbidden.

This is the sole current human status document. Operational facts come from the CAS integration ref, frozen gitlink, committed DAG, and external controller state:

```bash
python3 tools/pi-port-swarm/controller.py status
python3 tools/pi-port-swarm/manifest.py status
```

## Stage-1 package map

| Package | Mechanical inventory | Semantic state |
|---|---:|---|
| AI | 247/247 rows accounted for | accepted on product SHA `7ae63740`; promoted-`main` revalidation pending |
| Agent | 45/45 rows accounted for | accepted on product SHA `7ae63740`; promoted-`main` revalidation pending |
| TUI | 61/61 rows accounted for | replacement acceptance on product SHA `7ae63740`; promoted-`main` revalidation pending |
| Coding-agent | 332 rows accounted for | replacement acceptance on product SHA `7ae63740`; promoted-`main` revalidation pending |
| Orchestrator | 13/13 rows accounted for | accepted on product SHA `7ae63740`; promoted-`main` revalidation pending |

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

The user subsequently approved exact target-specific `windows-sys = "=0.61.2"` (`Win32_Foundation`, `Win32_System_Console`) and `objc2-core-graphics = "=0.3.2"` (`default-features = false`, `CGEventSource`, `CGEventTypes`). One documented Windows-only `unsafe` boundary may enable `ENABLE_VIRTUAL_TERMINAL_INPUT`; unsafe remains denied everywhere else. Rust PTY/ConPTY dev-dependencies remain arbitration-gated. Frozen Node/@xterm-headless is the differential visual oracle until separately replaced.

The canonical extension decision is `docs/porting/RUST_EXTENSION_ARCHITECTURE.md`: maximum Pi `ExtensionAPI` capability alignment through separately compiled Rust `cdylib` extensions loaded in-process, custom C ABI v1, exact `libloading = "=0.9.0"`, reused `sha2 = "0.10"`, one audited unsafe loader module, process-lifetime library retention, and source-only crates.io/GitHub/local installation with mandatory local Cargo build and provenance receipt. TS/jiti compatibility is deferred and must not be claimed by Stage 1.

The controller now permits a reviewed CAS upgrade from a fully completed DAG to a fresh, non-reused, reachable DAG while preserving all external runtime history. The semantic controller candidate must descend from `f83a96fe`, preserve the Pi gitlink, pass control tests, atomically upgrade `automation/pi-port`, and only then restart dispatch.

## Final-gate evidence

The controller accepted replacement final-gate unit `P8.T1-V1` on immutable product SHA `7ae6374063da1f60cc5767d0a4e51d907cfc61d6`. Its fmt, workspace check, executed workspace tests, strict manifest, and controller validation all returned 0. This one-SHA acceptance supersedes the earlier split-SHA workspace/fidelity/Rust-quality/end-user table and the invalid TUI/end-user completion evidence associated with it.

Stage 1 is **not complete**: the current commit is a later evidence-only documentation commit, not the tested product SHA. A human must explicitly approve promotion of the tested product SHA to `main`, after which a fresh read-only validator must repeat every final gate on promoted `main`. Exact command records and evidence limitations are retained in `.agents/state/stage-1-final-gate-evidence.md`.

## Human promotion checkpoint

The tested product SHA is `7ae6374063da1f60cc5767d0a4e51d907cfc61d6`; this later documentation commit must not be substituted for it or described as product-tested. **Human approval is requested to promote that exact product SHA to `main`.** Until approval, promotion is forbidden. After promotion, all final gates must pass again on `main`; Stage 2 remains forbidden until then.

## Stage-1 exit gate

Stage 1 is complete only when all of the following hold on one recorded commit SHA:

1. Every frozen source/test has a reachable semantic implementation/executable test or exact approved disposition; markers and empty tests do not count.
2. No dead mapped module, runtime placeholder/no-op, unwired CLI mode, unexplained ignore, or pending dependency arbitration remains.
3. Default TUI, print/text/json, RPC, sessions, tools, Rust extensions, skills, themes, package management and Orchestrator run end to end. Extension capability behavior is compared with Pi while the approved TS/jiti source-compatibility adapter remains deferred.
4. Provider transport, errors, streaming, cancellation, compaction and persistence pass independent fidelity review.
5. Workspace fmt/check/executed tests and strict semantic manifest pass.
6. Independent end-user, fidelity and Rust-quality reviews accept the same immutable SHA.
7. That SHA is explicitly promoted to `main`, and all gates pass again there.

Only then may Stage 2 begin.

## History

The completed mechanical DAG and its runtime records are preserved, not reset. Semantic execution advances only the CAS-managed `automation/pi-port` ref through fresh IDs and validated worktrees.
