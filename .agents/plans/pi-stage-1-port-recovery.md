# Pi Stage-1 Semantic Fidelity Closure

<a id="how-to-use"></a>
## How to use this plan

This approved revision replaces the completed mechanical DAG. The controller runs only the fresh IDs below, one writer at a time, from `automation/pi-port`. Every implementation agent starts fresh and must read its unit, the frozen Pi files it replaces, and dependency outputs. A blocker must be classified; no placeholder or disposition may be invented to advance the graph.

<a id="legend"></a>
## Legend

- `writer`: one scoped implementation commit; local validation only.
- `validator`: read-only declared package/workspace gates.
- `reviewer`: independent semantic, Rust-quality, or end-user review.
- `checkpoint`: records completion evidence only.
- All units are `Assignable: yes`, `Context: fresh`; execution is sequential except the disjoint AI/Agent residual writers.

<a id="goal"></a>
## Goal

Produce a behaviorally faithful, executable one-to-one Rust port of frozen Pi AI, Agent, TUI, Coding-agent, and Orchestrator. Replace marker-only files, dead modules, vacuous tests, runtime no-ops and unwired modes, then pass differential, native-terminal, package, workspace and end-user gates on one SHA.

<a id="non-goals"></a>
## Non-goals

No Stage 2/LangGraph work, Ratatui rewrite, direct `unicode-width` substitution, alternate-screen redesign, frozen gitlink mutation, credential fabrication, promotion to `main`, prebuilt native extension installation, dynamic-library unload in ABI v1, or TypeScript/jiti extension compatibility in the current closure. TS compatibility is a later adapter.

<a id="review-flags"></a>
## Review Flags

| ID | Type | Severity | Summary | Affects | Follow-up / accepted risk |
|---|---|---:|---|---|---|
| RF-S1 | R | High | Mechanical mappings previously admitted marker-only/dead/vacuous targets. | all | `SEM-V1-CLOSURE-GUARD` must fail these before implementation proceeds. |
| RF-S2 | BQ resolved | High | Portable TTY boundary required. | TUI | User approved exact `crossterm = "=0.29.0"`; use only raw mode/size/restoration, not its parser as a Pi replacement. |
| RF-S3 | BQ | High | Apple Terminal modifier polling and exact Windows VT input may require another native dependency. | SEM-TUI-V5 | Try approved Crossterm path; otherwise stop with exact `ARBITRATION_REQUIRED`. |
| RF-S4 | R | Medium | Live-provider/OAuth tests require external capabilities. | AI/CA | Keep narrowly capability-gated with explicit reason; all deterministic behavior must execute. |
| RF-S5 | R | Medium | PTY/ConPTY automation may justify a test-only dependency. | final native gate | Use current Node/xterm oracle first; any Rust PTY dependency requires arbitration. |
| RF-S6 | BQ resolved | High | Pi executes TS extensions in-process through jiti; Zedflow requires compiled Rust extensions. | Coding-agent V3 | User approved maximum ExtensionAPI capability alignment through separately compiled Rust cdylibs, custom ABI v1, source-only crate/GitHub install and local build; TS compatibility is deferred. |

<a id="global-acceptance"></a>
## Global Acceptance Criteria

1. Every mapped Rust source is semantically implemented and reachable; every mapped test contains executable behavior or an approved exact disposition. The approved extension substitution preserves Pi ExtensionAPI capabilities through Rust cdylibs while explicitly deferring TS/jiti source compatibility.
2. No `MODULE_PATH`, `TEST_PATH`, `PortPlaceholder`, runtime no-op, `not wired yet`, unexplained `#[ignore]`, dead mapped module, or advertised unreachable CLI mode remains.
3. TUI preserves Pi terminal lifecycle, raw input/Kitty parser, differential renderer, components, Markdown ANSI/OSC-8, Unicode 17 composite width, images and restoration behavior.
4. Default interactive, print/text/json, RPC, sessions, tools, extensions, skills, themes, packages and Orchestrator run end to end.
5. Package gates, strict manifest, workspace fmt/check/tests, independent fidelity/Rust/end-user reviews all accept one immutable SHA.
6. Stage 2 remains forbidden until that SHA is explicitly promoted to `main` and all gates repeat there.

<a id="legacy-policy"></a>
## Legacy / workaround policy

No aliases, shims, type weakening, marker constants, empty tests, compile-only assertions, blanket ignores, direct source-path assertions, or broad exception entries may stand in for behavior. Fix root causes in shared code. Do not delete files unless separately approved; disposition duplicate legacy paths explicitly. Approved extension dependencies are exactly `libloading = "=0.9.0"` and reused `sha2 = "0.10"`; all other replacement dependencies remain `ARBITRATION_REQUIRED`.

<a id="breaking-changes"></a>
## Planned Breaking Changes and Propagation Map

| Change | Introduced by | Expected temporary breakage | Fixed by | Forbidden workaround |
|---|---|---|---|---|
| Strict semantic manifest gate | SEM-V1 | Existing crates intentionally fail closure | package semantic units | exceptions for markers/tests |
| Crossterm terminal lifecycle | SEM-TUI-V1 | terminal contracts become fallible/native | TUI V2-V6 and Coding-agent V4-V5 | line REPL or Crossterm key-parser substitution |
| Rust extension ABI replaces jiti execution | SEM-CA-V3A–V3G | TS source compatibility is deferred; extension packages become Rust cdylibs | ABI/SDK/install/resource validators and fidelity review | Bun/Deno shortcut, Rust trait objects across ABI, prebuilt artifact install, or capability loss |
| Real Coding-agent runtime wiring | SEM-CA-V4/V5 | placeholder APIs become executable/fallible | CA test batches and validation | keep `not wired yet` branches |
| Orchestrator runtime replaces constants | SEM-ORCH-V1 | real IPC/process errors surface | ORCH validators/review | marker modules |

<a id="orchestration"></a>
## Subagent Orchestration Plan

- W0: `SEM-V1-CLOSURE-GUARD`.
- W1: `SEM-TUI-V1` through `SEM-TUI-V7` in order.
- W2: `SEM-CA-V1`, `SEM-CA-V2`, Rust extension sequence `SEM-CA-V3A` through `SEM-CA-V3G`, then `SEM-CA-V4` through `SEM-CA-V12`.
- W3: `SEM-ORCH-V1` through `SEM-ORCH-V3`.
- W4: `SEM-AI-V1-RESIDUALS` and `SEM-AG-V1-RESIDUALS` are disjoint; controller still enforces one writer. Then residual validation.
- W5: workspace, fidelity, Rust-quality, end-user, docs, checkpoint.

<a id="parallelization-constraints"></a>
## Parallelization Constraints

| Constraint | Reason | Affected units |
|---|---|---|
| One active writer | shared module roots/Cargo lock and deterministic CAS | all writers |
| TUI before Coding-agent | Coding-agent interactive UI consumes TUI contracts | W1/W2 |
| Coding-agent before Orchestrator | Orchestrator supervises Coding-agent runtime | W2/W3 |
| Validators/reviewers use exact predecessor SHA | prevent mixed evidence | all gates |
| Native dependency decisions stop rather than improvise | accepted arbitration policy | SEM-TUI-V5 |

<a id="canonical-line-references"></a>
## Canonical Line References

<!-- CANONICAL_LINE_REFERENCES_START -->
<!-- This block is generated by finalize-plan-lines.mjs. Do not edit manually. -->
| ID | Anchor | Lines | Description |
|---|---|---|---|
| how-to-use | #how-to-use | L3-L6 | How to use this plan |
| legend | #legend | L8-L15 | Legend |
| goal | #goal | L17-L20 | Goal |
| non-goals | #non-goals | L22-L25 | Non-goals |
| review-flags | #review-flags | L27-L37 | Review Flags |
| global-acceptance | #global-acceptance | L39-L47 | Global Acceptance Criteria |
| legacy-policy | #legacy-policy | L49-L52 | Legacy / workaround policy |
| breaking-changes | #breaking-changes | L54-L63 | Planned Breaking Changes and Propagation Map |
| orchestration | #orchestration | L65-L73 | Subagent Orchestration Plan |
| parallelization-constraints | #parallelization-constraints | L75-L84 | Parallelization Constraints |
| canonical-line-references | #canonical-line-references | L86-L144 | Canonical Line References |
| phases-and-tasks | #phases-and-tasks | L146-L2641 | Phases and Tasks |
| sem-v1-closure-guard | #sem-v1-closure-guard | L149-L205 | SEM-V1-CLOSURE-GUARD |
| sem-tui-v1-portable-terminal | #sem-tui-v1-portable-terminal | L207-L270 | SEM-TUI-V1-PORTABLE-TERMINAL |
| sem-tui-v2-renderer | #sem-tui-v2-renderer | L272-L339 | SEM-TUI-V2-RENDERER |
| sem-tui-v3-input-editing | #sem-tui-v3-input-editing | L341-L415 | SEM-TUI-V3-INPUT-EDITING |
| sem-tui-v4-components | #sem-tui-v4-components | L417-L486 | SEM-TUI-V4-COMPONENTS |
| sem-tui-v5-native-backends | #sem-tui-v5-native-backends | L488-L548 | SEM-TUI-V5-NATIVE-BACKENDS |
| sem-tui-v6-validate | #sem-tui-v6-validate | L550-L605 | SEM-TUI-V6-VALIDATE |
| sem-tui-v7-fidelity | #sem-tui-v7-fidelity | L607-L662 | SEM-TUI-V7-FIDELITY |
| sem-ca-v1-core-tools | #sem-ca-v1-core-tools | L664-L725 | SEM-CA-V1-CORE-TOOLS |
| sem-ca-v2-session-config | #sem-ca-v2-session-config | L727-L798 | SEM-CA-V2-SESSION-CONFIG |
| sem-ca-v3a-rust-extension-api-r1 | #sem-ca-v3a-rust-extension-api-r1 | L800-L865 | SEM-CA-V3A-RUST-EXTENSION-API-R1 |
| sem-ca-v3b-rust-extension-abi-loader-r1 | #sem-ca-v3b-rust-extension-abi-loader-r1 | L867-L931 | SEM-CA-V3B-RUST-EXTENSION-ABI-LOADER-R1 |
| sem-ca-v3c-rust-extension-sdk-fixture-r1 | #sem-ca-v3c-rust-extension-sdk-fixture-r1 | L933-L993 | SEM-CA-V3C-RUST-EXTENSION-SDK-FIXTURE-R1 |
| sem-ca-v3d-source-install-provenance-r1 | #sem-ca-v3d-source-install-provenance-r1 | L995-L1064 | SEM-CA-V3D-SOURCE-INSTALL-PROVENANCE-R1 |
| sem-ca-v3e-remaining-resources-r1 | #sem-ca-v3e-remaining-resources-r1 | L1066-L1146 | SEM-CA-V3E-REMAINING-RESOURCES-R1 |
| sem-ca-v3f-rust-extensions-validate-r1 | #sem-ca-v3f-rust-extensions-validate-r1 | L1148-L1204 | SEM-CA-V3F-RUST-EXTENSIONS-VALIDATE-R1 |
| sem-ca-v3g-rust-extensions-fidelity-r1 | #sem-ca-v3g-rust-extensions-fidelity-r1 | L1206-L1262 | SEM-CA-V3G-RUST-EXTENSIONS-FIDELITY-R1 |
| sem-ca-v4-interactive | #sem-ca-v4-interactive | L1264-L1319 | SEM-CA-V4-INTERACTIVE |
| sem-ca-v5-cli-modes | #sem-ca-v5-cli-modes | L1321-L1385 | SEM-CA-V5-CLI-MODES |
| sem-ca-v6-test-batch-1 | #sem-ca-v6-test-batch-1 | L1387-L1477 | SEM-CA-V6-TEST-BATCH-1 |
| sem-ca-v7-test-batch-2 | #sem-ca-v7-test-batch-2 | L1479-L1569 | SEM-CA-V7-TEST-BATCH-2 |
| sem-ca-v8-test-batch-3 | #sem-ca-v8-test-batch-3 | L1571-L1661 | SEM-CA-V8-TEST-BATCH-3 |
| sem-ca-v9-test-batch-4 | #sem-ca-v9-test-batch-4 | L1663-L1753 | SEM-CA-V9-TEST-BATCH-4 |
| sem-ca-v10-test-batch-5 | #sem-ca-v10-test-batch-5 | L1755-L1834 | SEM-CA-V10-TEST-BATCH-5 |
| sem-ca-v11-validate | #sem-ca-v11-validate | L1836-L1891 | SEM-CA-V11-VALIDATE |
| sem-ca-v12-fidelity | #sem-ca-v12-fidelity | L1893-L1948 | SEM-CA-V12-FIDELITY |
| sem-orch-v1-runtime | #sem-orch-v1-runtime | L1950-L2008 | SEM-ORCH-V1-RUNTIME |
| sem-orch-v2-validate | #sem-orch-v2-validate | L2010-L2065 | SEM-ORCH-V2-VALIDATE |
| sem-orch-v3-fidelity | #sem-orch-v3-fidelity | L2067-L2122 | SEM-ORCH-V3-FIDELITY |
| sem-ai-v1-residuals | #sem-ai-v1-residuals | L2124-L2181 | SEM-AI-V1-RESIDUALS |
| sem-ag-v1-residuals | #sem-ag-v1-residuals | L2183-L2239 | SEM-AG-V1-RESIDUALS |
| sem-residuals-v2-validate | #sem-residuals-v2-validate | L2241-L2296 | SEM-RESIDUALS-V2-VALIDATE |
| sem-final-v1-workspace | #sem-final-v1-workspace | L2298-L2353 | SEM-FINAL-V1-WORKSPACE |
| sem-final-v2-fidelity | #sem-final-v2-fidelity | L2355-L2410 | SEM-FINAL-V2-FIDELITY |
| sem-final-v3-rust-quality | #sem-final-v3-rust-quality | L2412-L2467 | SEM-FINAL-V3-RUST-QUALITY |
| sem-final-v4-enduser | #sem-final-v4-enduser | L2469-L2524 | SEM-FINAL-V4-ENDUSER |
| sem-final-v5-docs | #sem-final-v5-docs | L2526-L2583 | SEM-FINAL-V5-DOCS |
| sem-final-v6-checkpoint | #sem-final-v6-checkpoint | L2585-L2641 | SEM-FINAL-V6-CHECKPOINT |
| pre-finalization-review | #pre-finalization-review | L2643-L2650 | Pre-finalization Review Summary |
<!-- CANONICAL_LINE_REFERENCES_END -->

<a id="phases-and-tasks"></a>
## Phases and Tasks

<a id="sem-v1-closure-guard"></a>
### SEM-V1-CLOSURE-GUARD

Assignable: yes

Execution metadata:
- Wave: W0
- Context: fresh
- Depends on: none
- Can run in parallel with: only disjoint AI/Agent residual peer where applicable
- Must not run in parallel with: any unit sharing an ownership prefix

Scope boundaries:
- Goal: Replace mechanical-only closure with revision-aware semantic guards: reject marker-only sources, empty/non-executable tests, dead mapped modules, unexplained ignores, narrow runtime placeholders, and unwired Coding-agent CLI modes. Do not disposition existing failures away.
- Non-goals: neighboring units, Stage 2, promotion, unapproved dependencies.
- Forbidden work: placeholders, vacuous tests, broad exceptions, frozen Pi mutation, out-of-scope cleanup.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify/create as required | `tools/pi-port-swarm/manifest.py` | exclusive controller ownership for this unit |
| modify/create as required | `tools/pi-port-swarm/test_manifest.py` | exclusive controller ownership for this unit |

Required context package:
- Plan references: this unit, Goal, Global Acceptance, Legacy policy, RF-S1 through RF-S6 as applicable.
- Required files: owned Rust paths and their one-to-one files/tests under `references/pi/packages/` at frozen SHA `2b00dade7cec918aefb025c8b7a4fa304a30acdd`.
- Dependency outputs: predecessor result, candidate SHA, declared validation logs.
- Neighboring out-of-scope units: all other IDs in this plan.

Implementation outline:
1. Read the complete frozen Pi counterpart and all Rust callers/exports before editing.
2. Implement observable behavior at the shared root cause; wire modules and real tests.
3. Run only declared/local checks; return a structured result with evidence and remaining blocker classification.

Validation responsibility:
- Type: locally-validating
- Must run: python3 tools/pi-port-swarm/test_manifest.py
- Must NOT run: workspace-wide gates or mutate files outside ownership.

Output contract:
- Structured PASS/BLOCKED result, exact base/candidate SHA, changed files or reviewed scope, commands/evidence, residual risks, and one valid blocker classification when not accepted.

Acceptance criteria:
- Replace mechanical-only closure with revision-aware semantic guards: reject marker-only sources, empty/non-executable tests, dead mapped modules, unexplained ignores, narrow runtime placeholders, and unwired Coding-agent CLI modes. Do not disposition existing failures away.
- No forbidden workaround or out-of-scope edit; all declared commands pass.

Handoff to dependent units:
- Commit/result SHA and durable validation logs; name any intentionally deferred behavior and its owning downstream ID.

Subagent prompt:
```text
Implement only SEM-V1-CLOSURE-GUARD from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this entire unit, its listed frozen Pi counterparts, Rust callers/module roots, dependency outputs, and the global acceptance/legacy rules before editing.
Task: Replace mechanical-only closure with revision-aware semantic guards: reject marker-only sources, empty/non-executable tests, dead mapped modules, unexplained ignores, narrow runtime placeholders, and unwired Coding-agent CLI modes. Do not disposition existing failures away.
Stay inside ownership. Do not implement neighboring units or Stage 2. Do not use marker code, vacuous tests, broad exceptions, or unapproved dependency substitutions.
Run the stated validation responsibility. If blocked, stop and return exact evidence with REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="sem-tui-v1-portable-terminal"></a>
### SEM-TUI-V1-PORTABLE-TERMINAL

Assignable: yes

Execution metadata:
- Wave: W1
- Context: fresh
- Depends on: SEM-V1-CLOSURE-GUARD
- Can run in parallel with: only disjoint AI/Agent residual peer where applicable
- Must not run in parallel with: any unit sharing an ownership prefix

Scope boundaries:
- Goal: Port Pi ProcessTerminal lifecycle and stdin framing. Add exactly crossterm =0.29.0 for safe raw mode, size and restoration only; preserve Pi raw-byte/CSI/OSC/DCS/APC/bracketed-paste parsing and protocol negotiation locally.
- Non-goals: neighboring units, Stage 2, promotion, unapproved dependencies.
- Forbidden work: placeholders, vacuous tests, broad exceptions, frozen Pi mutation, out-of-scope cleanup.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify/create as required | `Cargo.lock` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/Cargo.toml` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/src/terminal.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/src/stdin-buffer.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/src/terminal-colors.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/tests/terminal.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/tests/stdin-buffer.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/tests/terminal-colors.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/tests/tui-cell-size-input.rs` | exclusive controller ownership for this unit |

Required context package:
- Plan references: this unit, Goal, Global Acceptance, Legacy policy, RF-S1 through RF-S6 as applicable.
- Required files: owned Rust paths and their one-to-one files/tests under `references/pi/packages/` at frozen SHA `2b00dade7cec918aefb025c8b7a4fa304a30acdd`.
- Dependency outputs: predecessor result, candidate SHA, declared validation logs.
- Neighboring out-of-scope units: all other IDs in this plan.

Implementation outline:
1. Read the complete frozen Pi counterpart and all Rust callers/exports before editing.
2. Implement observable behavior at the shared root cause; wire modules and real tests.
3. Run only declared/local checks; return a structured result with evidence and remaining blocker classification.

Validation responsibility:
- Type: locally-validating
- Must run: cargo fmt --package zedflow-tui --check; cargo test -p zedflow-tui --test terminal --test stdin-buffer --test terminal-colors --test tui-cell-size-input
- Must NOT run: workspace-wide gates or mutate files outside ownership.

Output contract:
- Structured PASS/BLOCKED result, exact base/candidate SHA, changed files or reviewed scope, commands/evidence, residual risks, and one valid blocker classification when not accepted.

Acceptance criteria:
- Port Pi ProcessTerminal lifecycle and stdin framing. Add exactly crossterm =0.29.0 for safe raw mode, size and restoration only; preserve Pi raw-byte/CSI/OSC/DCS/APC/bracketed-paste parsing and protocol negotiation locally.
- No forbidden workaround or out-of-scope edit; all declared commands pass.

Handoff to dependent units:
- Commit/result SHA and durable validation logs; name any intentionally deferred behavior and its owning downstream ID.

Subagent prompt:
```text
Implement only SEM-TUI-V1-PORTABLE-TERMINAL from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this entire unit, its listed frozen Pi counterparts, Rust callers/module roots, dependency outputs, and the global acceptance/legacy rules before editing.
Task: Port Pi ProcessTerminal lifecycle and stdin framing. Add exactly crossterm =0.29.0 for safe raw mode, size and restoration only; preserve Pi raw-byte/CSI/OSC/DCS/APC/bracketed-paste parsing and protocol negotiation locally.
Stay inside ownership. Do not implement neighboring units or Stage 2. Do not use marker code, vacuous tests, broad exceptions, or unapproved dependency substitutions.
Run the stated validation responsibility. If blocked, stop and return exact evidence with REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="sem-tui-v2-renderer"></a>
### SEM-TUI-V2-RENDERER

Assignable: yes

Execution metadata:
- Wave: W1
- Context: fresh
- Depends on: SEM-TUI-V1-PORTABLE-TERMINAL
- Can run in parallel with: only disjoint AI/Agent residual peer where applicable
- Must not run in parallel with: any unit sharing an ownership prefix

Scope boundaries:
- Goal: Port Pi TUI lifecycle, render scheduling, differential redraw, viewport overwrite, overlays, focus and logical cursor. Export the runtime from the crate root; do not replace it with Ratatui or a full-screen abstraction.
- Non-goals: neighboring units, Stage 2, promotion, unapproved dependencies.
- Forbidden work: placeholders, vacuous tests, broad exceptions, frozen Pi mutation, out-of-scope cleanup.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify/create as required | `crates/zedflow-tui/src/lib.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/src/index.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/src/tui.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/src/primitives.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/tests/primitives.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/tests/tui-render.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/tests/tui-shrink.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/tests/tui-overlay-style-leak.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/tests/overlay-non-capturing.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/tests/overlay-options.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/tests/overlay-short-content.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/tests/regression-overlay-cjk-boundary.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/tests/viewport-overwrite-repro.rs` | exclusive controller ownership for this unit |

Required context package:
- Plan references: this unit, Goal, Global Acceptance, Legacy policy, RF-S1 through RF-S6 as applicable.
- Required files: owned Rust paths and their one-to-one files/tests under `references/pi/packages/` at frozen SHA `2b00dade7cec918aefb025c8b7a4fa304a30acdd`.
- Dependency outputs: predecessor result, candidate SHA, declared validation logs.
- Neighboring out-of-scope units: all other IDs in this plan.

Implementation outline:
1. Read the complete frozen Pi counterpart and all Rust callers/exports before editing.
2. Implement observable behavior at the shared root cause; wire modules and real tests.
3. Run only declared/local checks; return a structured result with evidence and remaining blocker classification.

Validation responsibility:
- Type: locally-validating
- Must run: cargo fmt --package zedflow-tui --check
- Must NOT run: workspace-wide gates or mutate files outside ownership.

Output contract:
- Structured PASS/BLOCKED result, exact base/candidate SHA, changed files or reviewed scope, commands/evidence, residual risks, and one valid blocker classification when not accepted.

Acceptance criteria:
- Port Pi TUI lifecycle, render scheduling, differential redraw, viewport overwrite, overlays, focus and logical cursor. Export the runtime from the crate root; do not replace it with Ratatui or a full-screen abstraction.
- No forbidden workaround or out-of-scope edit; all declared commands pass.

Handoff to dependent units:
- Commit/result SHA and durable validation logs; name any intentionally deferred behavior and its owning downstream ID.

Subagent prompt:
```text
Implement only SEM-TUI-V2-RENDERER from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this entire unit, its listed frozen Pi counterparts, Rust callers/module roots, dependency outputs, and the global acceptance/legacy rules before editing.
Task: Port Pi TUI lifecycle, render scheduling, differential redraw, viewport overwrite, overlays, focus and logical cursor. Export the runtime from the crate root; do not replace it with Ratatui or a full-screen abstraction.
Stay inside ownership. Do not implement neighboring units or Stage 2. Do not use marker code, vacuous tests, broad exceptions, or unapproved dependency substitutions.
Run the stated validation responsibility. If blocked, stop and return exact evidence with REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="sem-tui-v3-input-editing"></a>
### SEM-TUI-V3-INPUT-EDITING

Assignable: yes

Execution metadata:
- Wave: W1
- Context: fresh
- Depends on: SEM-TUI-V2-RENDERER
- Can run in parallel with: only disjoint AI/Agent residual peer where applicable
- Must not run in parallel with: any unit sharing an ownership prefix

Scope boundaries:
- Goal: Complete Pi key parsing/keybindings, editor semantics, history/undo/kill ring, autocomplete/fuzzy matching, ANSI wrapping and Unicode-17 composite width behavior with executable differential tests.
- Non-goals: neighboring units, Stage 2, promotion, unapproved dependencies.
- Forbidden work: placeholders, vacuous tests, broad exceptions, frozen Pi mutation, out-of-scope cleanup.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify/create as required | `crates/zedflow-tui/src/keys.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/src/keybindings.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/src/autocomplete.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/src/editor-component.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/src/fuzzy.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/src/kill-ring.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/src/undo-stack.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/src/utils.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/src/word-navigation.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/tests/keys.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/tests/keybindings.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/tests/autocomplete.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/tests/editor.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/tests/fuzzy.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/tests/input.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/tests/tab-width.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/tests/truncate-to-width.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/tests/word-navigation.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/tests/wrap-ansi.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/tests/regression-regional-indicator-width.rs` | exclusive controller ownership for this unit |

Required context package:
- Plan references: this unit, Goal, Global Acceptance, Legacy policy, RF-S1 through RF-S6 as applicable.
- Required files: owned Rust paths and their one-to-one files/tests under `references/pi/packages/` at frozen SHA `2b00dade7cec918aefb025c8b7a4fa304a30acdd`.
- Dependency outputs: predecessor result, candidate SHA, declared validation logs.
- Neighboring out-of-scope units: all other IDs in this plan.

Implementation outline:
1. Read the complete frozen Pi counterpart and all Rust callers/exports before editing.
2. Implement observable behavior at the shared root cause; wire modules and real tests.
3. Run only declared/local checks; return a structured result with evidence and remaining blocker classification.

Validation responsibility:
- Type: locally-validating
- Must run: cargo fmt --package zedflow-tui --check
- Must NOT run: workspace-wide gates or mutate files outside ownership.

Output contract:
- Structured PASS/BLOCKED result, exact base/candidate SHA, changed files or reviewed scope, commands/evidence, residual risks, and one valid blocker classification when not accepted.

Acceptance criteria:
- Complete Pi key parsing/keybindings, editor semantics, history/undo/kill ring, autocomplete/fuzzy matching, ANSI wrapping and Unicode-17 composite width behavior with executable differential tests.
- No forbidden workaround or out-of-scope edit; all declared commands pass.

Handoff to dependent units:
- Commit/result SHA and durable validation logs; name any intentionally deferred behavior and its owning downstream ID.

Subagent prompt:
```text
Implement only SEM-TUI-V3-INPUT-EDITING from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this entire unit, its listed frozen Pi counterparts, Rust callers/module roots, dependency outputs, and the global acceptance/legacy rules before editing.
Task: Complete Pi key parsing/keybindings, editor semantics, history/undo/kill ring, autocomplete/fuzzy matching, ANSI wrapping and Unicode-17 composite width behavior with executable differential tests.
Stay inside ownership. Do not implement neighboring units or Stage 2. Do not use marker code, vacuous tests, broad exceptions, or unapproved dependency substitutions.
Run the stated validation responsibility. If blocked, stop and return exact evidence with REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="sem-tui-v4-components"></a>
### SEM-TUI-V4-COMPONENTS

Assignable: yes

Execution metadata:
- Wave: W1
- Context: fresh
- Depends on: SEM-TUI-V3-INPUT-EDITING
- Can run in parallel with: only disjoint AI/Agent residual peer where applicable
- Must not run in parallel with: any unit sharing an ownership prefix

Scope boundaries:
- Goal: Wire and port every Pi TUI component, local Markdown ANSI/OSC-8 renderer, image protocols and terminal-image helpers. Reuse workspace base64 0.22; retain approved exact markdown/ICU/emojis pins and xterm-headless as the frozen visual oracle.
- Non-goals: neighboring units, Stage 2, promotion, unapproved dependencies.
- Forbidden work: placeholders, vacuous tests, broad exceptions, frozen Pi mutation, out-of-scope cleanup.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify/create as required | `Cargo.lock` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/Cargo.toml` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/src/lib.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/src/index.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/src/components` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/src/terminal-image.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/tests/markdown.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/tests/terminal-image.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/tests/bug-regression-isimageline-startswith-bug.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/tests/select-list.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/tests/truncated-text.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/tests/image-test.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/tests/chat-simple.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/tests/test-themes.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/tests/virtual-terminal.rs` | exclusive controller ownership for this unit |

Required context package:
- Plan references: this unit, Goal, Global Acceptance, Legacy policy, RF-S1 through RF-S6 as applicable.
- Required files: owned Rust paths and their one-to-one files/tests under `references/pi/packages/` at frozen SHA `2b00dade7cec918aefb025c8b7a4fa304a30acdd`.
- Dependency outputs: predecessor result, candidate SHA, declared validation logs.
- Neighboring out-of-scope units: all other IDs in this plan.

Implementation outline:
1. Read the complete frozen Pi counterpart and all Rust callers/exports before editing.
2. Implement observable behavior at the shared root cause; wire modules and real tests.
3. Run only declared/local checks; return a structured result with evidence and remaining blocker classification.

Validation responsibility:
- Type: locally-validating
- Must run: cargo fmt --package zedflow-tui --check
- Must NOT run: workspace-wide gates or mutate files outside ownership.

Output contract:
- Structured PASS/BLOCKED result, exact base/candidate SHA, changed files or reviewed scope, commands/evidence, residual risks, and one valid blocker classification when not accepted.

Acceptance criteria:
- Wire and port every Pi TUI component, local Markdown ANSI/OSC-8 renderer, image protocols and terminal-image helpers. Reuse workspace base64 0.22; retain approved exact markdown/ICU/emojis pins and xterm-headless as the frozen visual oracle.
- No forbidden workaround or out-of-scope edit; all declared commands pass.

Handoff to dependent units:
- Commit/result SHA and durable validation logs; name any intentionally deferred behavior and its owning downstream ID.

Subagent prompt:
```text
Implement only SEM-TUI-V4-COMPONENTS from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this entire unit, its listed frozen Pi counterparts, Rust callers/module roots, dependency outputs, and the global acceptance/legacy rules before editing.
Task: Wire and port every Pi TUI component, local Markdown ANSI/OSC-8 renderer, image protocols and terminal-image helpers. Reuse workspace base64 0.22; retain approved exact markdown/ICU/emojis pins and xterm-headless as the frozen visual oracle.
Stay inside ownership. Do not implement neighboring units or Stage 2. Do not use marker code, vacuous tests, broad exceptions, or unapproved dependency substitutions.
Run the stated validation responsibility. If blocked, stop and return exact evidence with REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="sem-tui-v5-native-backends"></a>
### SEM-TUI-V5-NATIVE-BACKENDS

Assignable: yes

Execution metadata:
- Wave: W1
- Context: fresh
- Depends on: SEM-TUI-V4-COMPONENTS
- Can run in parallel with: only disjoint AI/Agent residual peer where applicable
- Must not run in parallel with: any unit sharing an ownership prefix

Scope boundaries:
- Goal: Close native parity: validate Windows Shift-Tab/Ctrl-Space/AltGr/keypad/Unicode/repeat semantics and Apple Terminal modifier polling. Use Crossterm first; if exact macOS or Windows behavior requires any additional dependency or unsafe boundary, return ARBITRATION_REQUIRED with exact version/features and evidence.
- Non-goals: neighboring units, Stage 2, promotion, unapproved dependencies.
- Forbidden work: placeholders, vacuous tests, broad exceptions, frozen Pi mutation, out-of-scope cleanup.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify/create as required | `Cargo.lock` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/Cargo.toml` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/src/native-modifiers.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/src/terminal.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/tests/terminal.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-tui/tests/keys.rs` | exclusive controller ownership for this unit |

Required context package:
- Plan references: this unit, Goal, Global Acceptance, Legacy policy, RF-S1 through RF-S6 as applicable.
- Required files: owned Rust paths and their one-to-one files/tests under `references/pi/packages/` at frozen SHA `2b00dade7cec918aefb025c8b7a4fa304a30acdd`.
- Dependency outputs: predecessor result, candidate SHA, declared validation logs.
- Neighboring out-of-scope units: all other IDs in this plan.

Implementation outline:
1. Read the complete frozen Pi counterpart and all Rust callers/exports before editing.
2. Implement observable behavior at the shared root cause; wire modules and real tests.
3. Run only declared/local checks; return a structured result with evidence and remaining blocker classification.

Validation responsibility:
- Type: locally-validating
- Must run: cargo fmt --package zedflow-tui --check
- Must NOT run: workspace-wide gates or mutate files outside ownership.

Output contract:
- Structured PASS/BLOCKED result, exact base/candidate SHA, changed files or reviewed scope, commands/evidence, residual risks, and one valid blocker classification when not accepted.

Acceptance criteria:
- Close native parity: validate Windows Shift-Tab/Ctrl-Space/AltGr/keypad/Unicode/repeat semantics and Apple Terminal modifier polling. Use Crossterm first; if exact macOS or Windows behavior requires any additional dependency or unsafe boundary, return ARBITRATION_REQUIRED with exact version/features and evidence.
- No forbidden workaround or out-of-scope edit; all declared commands pass.

Handoff to dependent units:
- Commit/result SHA and durable validation logs; name any intentionally deferred behavior and its owning downstream ID.

Subagent prompt:
```text
Implement only SEM-TUI-V5-NATIVE-BACKENDS from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this entire unit, its listed frozen Pi counterparts, Rust callers/module roots, dependency outputs, and the global acceptance/legacy rules before editing.
Task: Close native parity: validate Windows Shift-Tab/Ctrl-Space/AltGr/keypad/Unicode/repeat semantics and Apple Terminal modifier polling. Use Crossterm first; if exact macOS or Windows behavior requires any additional dependency or unsafe boundary, return ARBITRATION_REQUIRED with exact version/features and evidence.
Stay inside ownership. Do not implement neighboring units or Stage 2. Do not use marker code, vacuous tests, broad exceptions, or unapproved dependency substitutions.
Run the stated validation responsibility. If blocked, stop and return exact evidence with REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="sem-tui-v6-validate"></a>
### SEM-TUI-V6-VALIDATE

Assignable: yes

Execution metadata:
- Wave: W1
- Context: fresh
- Depends on: SEM-TUI-V5-NATIVE-BACKENDS
- Can run in parallel with: only disjoint AI/Agent residual peer where applicable
- Must not run in parallel with: any unit sharing an ownership prefix

Scope boundaries:
- Goal: Validate all executable TUI tests, strict semantic manifest closure, module reachability and package compilation. No source edits.
- Non-goals: neighboring units, Stage 2, promotion, unapproved dependencies.
- Forbidden work: placeholders, vacuous tests, broad exceptions, frozen Pi mutation, out-of-scope cleanup.

Files:
| Action | Path | Purpose |
|---|---|---|
| read only | exact predecessor candidate | validation/review only |

Required context package:
- Plan references: this unit, Goal, Global Acceptance, Legacy policy, RF-S1 through RF-S6 as applicable.
- Required files: owned Rust paths and their one-to-one files/tests under `references/pi/packages/` at frozen SHA `2b00dade7cec918aefb025c8b7a4fa304a30acdd`.
- Dependency outputs: predecessor result, candidate SHA, declared validation logs.
- Neighboring out-of-scope units: all other IDs in this plan.

Implementation outline:
1. Read the complete frozen Pi counterpart and all Rust callers/exports before editing.
2. Implement observable behavior at the shared root cause; wire modules and real tests.
3. Run only declared/local checks; return a structured result with evidence and remaining blocker classification.

Validation responsibility:
- Type: integration-validating
- Must run: cargo fmt --package zedflow-tui --check; cargo check -p zedflow-tui --all-targets; cargo test -p zedflow-tui --all-targets; python3 tools/pi-port-swarm/manifest.py check --package zedflow-tui
- Must NOT run: source edits.

Output contract:
- Structured PASS/BLOCKED result, exact base/candidate SHA, changed files or reviewed scope, commands/evidence, residual risks, and one valid blocker classification when not accepted.

Acceptance criteria:
- Validate all executable TUI tests, strict semantic manifest closure, module reachability and package compilation. No source edits.
- No forbidden workaround or out-of-scope edit; all declared commands pass.

Handoff to dependent units:
- Commit/result SHA and durable validation logs; name any intentionally deferred behavior and its owning downstream ID.

Subagent prompt:
```text
Implement only SEM-TUI-V6-VALIDATE from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this entire unit, its listed frozen Pi counterparts, Rust callers/module roots, dependency outputs, and the global acceptance/legacy rules before editing.
Task: Validate all executable TUI tests, strict semantic manifest closure, module reachability and package compilation. No source edits.
Stay inside ownership. Do not implement neighboring units or Stage 2. Do not use marker code, vacuous tests, broad exceptions, or unapproved dependency substitutions.
Run the stated validation responsibility. If blocked, stop and return exact evidence with REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="sem-tui-v7-fidelity"></a>
### SEM-TUI-V7-FIDELITY

Assignable: yes

Execution metadata:
- Wave: W1
- Context: fresh
- Depends on: SEM-TUI-V6-VALIDATE
- Can run in parallel with: only disjoint AI/Agent residual peer where applicable
- Must not run in parallel with: any unit sharing an ownership prefix

Scope boundaries:
- Goal: Independently compare frozen Pi TUI behavior against Rust: terminal lifecycle/restoration, raw input and Kitty, differential renderer, overlays, components, Markdown, Unicode 17, images and native-platform behavior. Reject dead modules, marker code or vacuous tests.
- Non-goals: neighboring units, Stage 2, promotion, unapproved dependencies.
- Forbidden work: placeholders, vacuous tests, broad exceptions, frozen Pi mutation, out-of-scope cleanup.

Files:
| Action | Path | Purpose |
|---|---|---|
| read only | exact predecessor candidate | validation/review only |

Required context package:
- Plan references: this unit, Goal, Global Acceptance, Legacy policy, RF-S1 through RF-S6 as applicable.
- Required files: owned Rust paths and their one-to-one files/tests under `references/pi/packages/` at frozen SHA `2b00dade7cec918aefb025c8b7a4fa304a30acdd`.
- Dependency outputs: predecessor result, candidate SHA, declared validation logs.
- Neighboring out-of-scope units: all other IDs in this plan.

Implementation outline:
1. Read the complete frozen Pi counterpart and all Rust callers/exports before editing.
2. Implement observable behavior at the shared root cause; wire modules and real tests.
3. Run only declared/local checks; return a structured result with evidence and remaining blocker classification.

Validation responsibility:
- Type: integration-validating
- Must run: independent commands necessary to establish the stated review intent
- Must NOT run: source edits.

Output contract:
- Structured PASS/BLOCKED result, exact base/candidate SHA, changed files or reviewed scope, commands/evidence, residual risks, and one valid blocker classification when not accepted.

Acceptance criteria:
- Independently compare frozen Pi TUI behavior against Rust: terminal lifecycle/restoration, raw input and Kitty, differential renderer, overlays, components, Markdown, Unicode 17, images and native-platform behavior. Reject dead modules, marker code or vacuous tests.
- No forbidden workaround or out-of-scope edit; all declared commands pass.

Handoff to dependent units:
- Commit/result SHA and durable validation logs; name any intentionally deferred behavior and its owning downstream ID.

Subagent prompt:
```text
Implement only SEM-TUI-V7-FIDELITY from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this entire unit, its listed frozen Pi counterparts, Rust callers/module roots, dependency outputs, and the global acceptance/legacy rules before editing.
Task: Independently compare frozen Pi TUI behavior against Rust: terminal lifecycle/restoration, raw input and Kitty, differential renderer, overlays, components, Markdown, Unicode 17, images and native-platform behavior. Reject dead modules, marker code or vacuous tests.
Stay inside ownership. Do not implement neighboring units or Stage 2. Do not use marker code, vacuous tests, broad exceptions, or unapproved dependency substitutions.
Run the stated validation responsibility. If blocked, stop and return exact evidence with REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="sem-ca-v1-core-tools"></a>
### SEM-CA-V1-CORE-TOOLS

Assignable: yes

Execution metadata:
- Wave: W2
- Context: fresh
- Depends on: SEM-TUI-V7-FIDELITY
- Can run in parallel with: only disjoint AI/Agent residual peer where applicable
- Must not run in parallel with: any unit sharing an ownership prefix

Scope boundaries:
- Goal: Replace marker-only Coding-agent core tool, exec, output, message, event and compaction modules with frozen Pi behavior. Preserve tool trust boundaries, truncation, cancellation and error semantics.
- Non-goals: neighboring units, Stage 2, promotion, unapproved dependencies.
- Forbidden work: placeholders, vacuous tests, broad exceptions, frozen Pi mutation, out-of-scope cleanup.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify/create as required | `crates/zedflow-coding-agent/src/core/tools` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/compaction` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/messages.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/event-bus.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/exec.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/bash-executor.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/output-guard.rs` | exclusive controller ownership for this unit |

Required context package:
- Plan references: this unit, Goal, Global Acceptance, Legacy policy, RF-S1 through RF-S6 as applicable.
- Required files: owned Rust paths and their one-to-one files/tests under `references/pi/packages/` at frozen SHA `2b00dade7cec918aefb025c8b7a4fa304a30acdd`.
- Dependency outputs: predecessor result, candidate SHA, declared validation logs.
- Neighboring out-of-scope units: all other IDs in this plan.

Implementation outline:
1. Read the complete frozen Pi counterpart and all Rust callers/exports before editing.
2. Implement observable behavior at the shared root cause; wire modules and real tests.
3. Run only declared/local checks; return a structured result with evidence and remaining blocker classification.

Validation responsibility:
- Type: locally-validating
- Must run: cargo fmt --package zedflow-coding-agent --check
- Must NOT run: workspace-wide gates or mutate files outside ownership.

Output contract:
- Structured PASS/BLOCKED result, exact base/candidate SHA, changed files or reviewed scope, commands/evidence, residual risks, and one valid blocker classification when not accepted.

Acceptance criteria:
- Replace marker-only Coding-agent core tool, exec, output, message, event and compaction modules with frozen Pi behavior. Preserve tool trust boundaries, truncation, cancellation and error semantics.
- No forbidden workaround or out-of-scope edit; all declared commands pass.

Handoff to dependent units:
- Commit/result SHA and durable validation logs; name any intentionally deferred behavior and its owning downstream ID.

Subagent prompt:
```text
Implement only SEM-CA-V1-CORE-TOOLS from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this entire unit, its listed frozen Pi counterparts, Rust callers/module roots, dependency outputs, and the global acceptance/legacy rules before editing.
Task: Replace marker-only Coding-agent core tool, exec, output, message, event and compaction modules with frozen Pi behavior. Preserve tool trust boundaries, truncation, cancellation and error semantics.
Stay inside ownership. Do not implement neighboring units or Stage 2. Do not use marker code, vacuous tests, broad exceptions, or unapproved dependency substitutions.
Run the stated validation responsibility. If blocked, stop and return exact evidence with REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="sem-ca-v2-session-config"></a>
### SEM-CA-V2-SESSION-CONFIG

Assignable: yes

Execution metadata:
- Wave: W2
- Context: fresh
- Depends on: SEM-CA-V1-CORE-TOOLS
- Can run in parallel with: only disjoint AI/Agent residual peer where applicable
- Must not run in parallel with: any unit sharing an ownership prefix

Scope boundaries:
- Goal: Port Pi sessions, persistence/tree operations, migrations, settings, auth, trust, model registry/resolution and configuration errors; --no-session only disables conversation persistence.
- Non-goals: neighboring units, Stage 2, promotion, unapproved dependencies.
- Forbidden work: placeholders, vacuous tests, broad exceptions, frozen Pi mutation, out-of-scope cleanup.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify/create as required | `crates/zedflow-coding-agent/src/config.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/migrations.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/agent-session.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/agent-session-runtime.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/agent-session-services.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/auth-guidance.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/auth-storage.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/defaults.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/diagnostics.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/model-registry.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/model-resolver.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/project-trust.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/resolve-config-value.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/session-cwd.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/session-manager.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/settings-manager.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/trust-manager.rs` | exclusive controller ownership for this unit |

Required context package:
- Plan references: this unit, Goal, Global Acceptance, Legacy policy, RF-S1 through RF-S6 as applicable.
- Required files: owned Rust paths and their one-to-one files/tests under `references/pi/packages/` at frozen SHA `2b00dade7cec918aefb025c8b7a4fa304a30acdd`.
- Dependency outputs: predecessor result, candidate SHA, declared validation logs.
- Neighboring out-of-scope units: all other IDs in this plan.

Implementation outline:
1. Read the complete frozen Pi counterpart and all Rust callers/exports before editing.
2. Implement observable behavior at the shared root cause; wire modules and real tests.
3. Run only declared/local checks; return a structured result with evidence and remaining blocker classification.

Validation responsibility:
- Type: locally-validating
- Must run: cargo fmt --package zedflow-coding-agent --check
- Must NOT run: workspace-wide gates or mutate files outside ownership.

Output contract:
- Structured PASS/BLOCKED result, exact base/candidate SHA, changed files or reviewed scope, commands/evidence, residual risks, and one valid blocker classification when not accepted.

Acceptance criteria:
- Port Pi sessions, persistence/tree operations, migrations, settings, auth, trust, model registry/resolution and configuration errors; --no-session only disables conversation persistence.
- No forbidden workaround or out-of-scope edit; all declared commands pass.

Handoff to dependent units:
- Commit/result SHA and durable validation logs; name any intentionally deferred behavior and its owning downstream ID.

Subagent prompt:
```text
Implement only SEM-CA-V2-SESSION-CONFIG from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this entire unit, its listed frozen Pi counterparts, Rust callers/module roots, dependency outputs, and the global acceptance/legacy rules before editing.
Task: Port Pi sessions, persistence/tree operations, migrations, settings, auth, trust, model registry/resolution and configuration errors; --no-session only disables conversation persistence.
Stay inside ownership. Do not implement neighboring units or Stage 2. Do not use marker code, vacuous tests, broad exceptions, or unapproved dependency substitutions.
Run the stated validation responsibility. If blocked, stop and return exact evidence with REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="sem-ca-v3a-rust-extension-api-r1"></a>
### SEM-CA-V3A-RUST-EXTENSION-API-R1

Assignable: yes

Execution metadata:
- Wave: W2
- Context: fresh
- Depends on: SEM-CA-V2-SESSION-CONFIG
- Can run in parallel with: none (ordered ABI/lifecycle contract)
- Must not run in parallel with: any Coding-agent writer

Scope boundaries:
- Goal: Port Pi ExtensionAPI capabilities to native Rust in-process contracts and executable handlers: tools, commands, keybindings/flags, lifecycle/input/session events, providers/models, compaction, UI/components, session actions, stale-context invalidation, errors and shutdown. Keep all Rust types inside one compilation boundary; no dynamic loading or source installation yet.
- Non-goals: TypeScript/jiti compatibility, Stage 2, additional workspace crates, hot unload, prebuilt extension installation, neighboring units.
- Forbidden work: Rust trait objects/String/Vec/futures across ABI, broad dispositions, unaudited unsafe, marker tests, capability loss presented as parity.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify/create as required | `docs/porting/RUST_EXTENSION_ARCHITECTURE.md` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/extensions/types.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/extensions/runner.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/extensions/wrapper.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/extensions/mod.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/extensions/index.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/index.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/extensions-runner.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/extensions-input-event.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/compaction-extensions.rs` | exclusive scope for this unit |

Required context package:
- Plan references: Goal, Global Acceptance, Legacy policy, RF-S1, RF-S4, RF-S6, and this unit.
- Required decision/spec: `docs/porting/RUST_EXTENSION_ARCHITECTURE.md` in full.
- Frozen Pi references: `src/core/extensions/{types,loader,runner,wrapper,index}.ts`, `src/core/sdk.ts`, `src/core/{package-manager,resource-loader,skills,prompt-templates}.ts`, and mapped tests as relevant.
- Dependency outputs: exact predecessor result, changed API/ABI contracts, candidate SHA and logs.
- Neighboring out-of-scope units: all other IDs.

Implementation outline:
1. Read complete frozen behavior and current Rust callers before editing.
2. Implement only the owned layer against the approved Rust substitution; preserve all representable Pi ExtensionAPI behavior.
3. Add executable behavior tests, run only declared validation, and return exact evidence or a valid blocker classification.

Validation responsibility:
- Type: locally-validating
- Must run: cargo fmt --package zedflow-coding-agent --check
- Must NOT run: workspace-wide gates or out-of-scope edits.

Output contract:
- Structured PASS/BLOCKED result with base/candidate SHA, changed/reviewed files, executable tests, command evidence, ABI/security residuals, and one valid blocker classification when blocked.

Acceptance criteria:
- Port Pi ExtensionAPI capabilities to native Rust in-process contracts and executable handlers: tools, commands, keybindings/flags, lifecycle/input/session events, providers/models, compaction, UI/components, session actions, stale-context invalidation, errors and shutdown. Keep all Rust types inside one compilation boundary; no dynamic loading or source installation yet.
- All ABI ownership/lifetime/trust rules in the architecture spec hold; no forbidden workaround or unapproved dependency.

Handoff to dependent units:
- Exact public contracts, ABI/message versions, artifact/receipt schema, candidate SHA and durable logs.

Subagent prompt:
```text
Implement only SEM-CA-V3A-RUST-EXTENSION-API-R1 from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this unit and docs/porting/RUST_EXTENSION_ARCHITECTURE.md completely, then read the listed frozen Pi counterparts, current Rust callers/module roots and predecessor outputs.
Preserve maximum Pi ExtensionAPI capability alignment through the approved Rust cdylib substitution. Stay inside ownership. Do not add Bun/Deno, another crate, hot-unload, prebuilt artifacts, broad dispositions, or unaudited unsafe.
Task: Port Pi ExtensionAPI capabilities to native Rust in-process contracts and executable handlers: tools, commands, keybindings/flags, lifecycle/input/session events, providers/models, compaction, UI/components, session actions, stale-context invalidation, errors and shutdown. Keep all Rust types inside one compilation boundary; no dynamic loading or source installation yet.
Run the stated validation responsibility. If blocked, stop with exact evidence and REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="sem-ca-v3b-rust-extension-abi-loader-r1"></a>
### SEM-CA-V3B-RUST-EXTENSION-ABI-LOADER-R1

Assignable: yes

Execution metadata:
- Wave: W2
- Context: fresh
- Depends on: SEM-CA-V3A-RUST-EXTENSION-API-R1
- Can run in parallel with: none (ordered ABI/lifecycle contract)
- Must not run in parallel with: any Coding-agent writer

Scope boundaries:
- Goal: Implement approved custom C ABI v1 and in-process cdylib loader with libloading =0.9.0. Use bounded versioned JSON envelopes, validated repr(C) tables/handles, trust+SHA-256 checks, no unwind across FFI, one scoped unsafe loader module, and process-lifetime Library retention with no unload v1.
- Non-goals: TypeScript/jiti compatibility, Stage 2, additional workspace crates, hot unload, prebuilt extension installation, neighboring units.
- Forbidden work: Rust trait objects/String/Vec/futures across ABI, broad dispositions, unaudited unsafe, marker tests, capability loss presented as parity.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify/create as required | `Cargo.lock` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/Cargo.toml` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/lib.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/extensions/abi.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/extensions/loader.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/extensions/mod.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/extensions-discovery.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/extensions-abi-v1.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/extensions-loader-cdylib.rs` | exclusive scope for this unit |

Required context package:
- Plan references: Goal, Global Acceptance, Legacy policy, RF-S1, RF-S4, RF-S6, and this unit.
- Required decision/spec: `docs/porting/RUST_EXTENSION_ARCHITECTURE.md` in full.
- Frozen Pi references: `src/core/extensions/{types,loader,runner,wrapper,index}.ts`, `src/core/sdk.ts`, `src/core/{package-manager,resource-loader,skills,prompt-templates}.ts`, and mapped tests as relevant.
- Dependency outputs: exact predecessor result, changed API/ABI contracts, candidate SHA and logs.
- Neighboring out-of-scope units: all other IDs.

Implementation outline:
1. Read complete frozen behavior and current Rust callers before editing.
2. Implement only the owned layer against the approved Rust substitution; preserve all representable Pi ExtensionAPI behavior.
3. Add executable behavior tests, run only declared validation, and return exact evidence or a valid blocker classification.

Validation responsibility:
- Type: locally-validating
- Must run: cargo fmt --package zedflow-coding-agent --check
- Must NOT run: workspace-wide gates or out-of-scope edits.

Output contract:
- Structured PASS/BLOCKED result with base/candidate SHA, changed/reviewed files, executable tests, command evidence, ABI/security residuals, and one valid blocker classification when blocked.

Acceptance criteria:
- Implement approved custom C ABI v1 and in-process cdylib loader with libloading =0.9.0. Use bounded versioned JSON envelopes, validated repr(C) tables/handles, trust+SHA-256 checks, no unwind across FFI, one scoped unsafe loader module, and process-lifetime Library retention with no unload v1.
- All ABI ownership/lifetime/trust rules in the architecture spec hold; no forbidden workaround or unapproved dependency.

Handoff to dependent units:
- Exact public contracts, ABI/message versions, artifact/receipt schema, candidate SHA and durable logs.

Subagent prompt:
```text
Implement only SEM-CA-V3B-RUST-EXTENSION-ABI-LOADER-R1 from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this unit and docs/porting/RUST_EXTENSION_ARCHITECTURE.md completely, then read the listed frozen Pi counterparts, current Rust callers/module roots and predecessor outputs.
Preserve maximum Pi ExtensionAPI capability alignment through the approved Rust cdylib substitution. Stay inside ownership. Do not add Bun/Deno, another crate, hot-unload, prebuilt artifacts, broad dispositions, or unaudited unsafe.
Task: Implement approved custom C ABI v1 and in-process cdylib loader with libloading =0.9.0. Use bounded versioned JSON envelopes, validated repr(C) tables/handles, trust+SHA-256 checks, no unwind across FFI, one scoped unsafe loader module, and process-lifetime Library retention with no unload v1.
Run the stated validation responsibility. If blocked, stop with exact evidence and REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="sem-ca-v3c-rust-extension-sdk-fixture-r1"></a>
### SEM-CA-V3C-RUST-EXTENSION-SDK-FIXTURE-R1

Assignable: yes

Execution metadata:
- Wave: W2
- Context: fresh
- Depends on: SEM-CA-V3B-RUST-EXTENSION-ABI-LOADER-R1
- Can run in parallel with: none (ordered ABI/lifecycle contract)
- Must not run in parallel with: any Coding-agent writer

Scope boundaries:
- Goal: Expose an ergonomic Rust Extension trait/ExtensionApi and export_extension! macro from the existing coding-agent crate, preserving the five-crate workspace. Compile a real fixture cdylib separately, load it, and exercise Pi-equivalent registration, invocation, UI/provider/event behavior and shutdown through ABI v1.
- Non-goals: TypeScript/jiti compatibility, Stage 2, additional workspace crates, hot unload, prebuilt extension installation, neighboring units.
- Forbidden work: Rust trait objects/String/Vec/futures across ABI, broad dispositions, unaudited unsafe, marker tests, capability loss presented as parity.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify/create as required | `crates/zedflow-coding-agent/src/core/sdk.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/extensions/abi.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/extensions/mod.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/extensions-sdk-build.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/fixtures/rust-extension` | exclusive scope for this unit |

Required context package:
- Plan references: Goal, Global Acceptance, Legacy policy, RF-S1, RF-S4, RF-S6, and this unit.
- Required decision/spec: `docs/porting/RUST_EXTENSION_ARCHITECTURE.md` in full.
- Frozen Pi references: `src/core/extensions/{types,loader,runner,wrapper,index}.ts`, `src/core/sdk.ts`, `src/core/{package-manager,resource-loader,skills,prompt-templates}.ts`, and mapped tests as relevant.
- Dependency outputs: exact predecessor result, changed API/ABI contracts, candidate SHA and logs.
- Neighboring out-of-scope units: all other IDs.

Implementation outline:
1. Read complete frozen behavior and current Rust callers before editing.
2. Implement only the owned layer against the approved Rust substitution; preserve all representable Pi ExtensionAPI behavior.
3. Add executable behavior tests, run only declared validation, and return exact evidence or a valid blocker classification.

Validation responsibility:
- Type: locally-validating
- Must run: cargo fmt --package zedflow-coding-agent --check
- Must NOT run: workspace-wide gates or out-of-scope edits.

Output contract:
- Structured PASS/BLOCKED result with base/candidate SHA, changed/reviewed files, executable tests, command evidence, ABI/security residuals, and one valid blocker classification when blocked.

Acceptance criteria:
- Expose an ergonomic Rust Extension trait/ExtensionApi and export_extension! macro from the existing coding-agent crate, preserving the five-crate workspace. Compile a real fixture cdylib separately, load it, and exercise Pi-equivalent registration, invocation, UI/provider/event behavior and shutdown through ABI v1.
- All ABI ownership/lifetime/trust rules in the architecture spec hold; no forbidden workaround or unapproved dependency.

Handoff to dependent units:
- Exact public contracts, ABI/message versions, artifact/receipt schema, candidate SHA and durable logs.

Subagent prompt:
```text
Implement only SEM-CA-V3C-RUST-EXTENSION-SDK-FIXTURE-R1 from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this unit and docs/porting/RUST_EXTENSION_ARCHITECTURE.md completely, then read the listed frozen Pi counterparts, current Rust callers/module roots and predecessor outputs.
Preserve maximum Pi ExtensionAPI capability alignment through the approved Rust cdylib substitution. Stay inside ownership. Do not add Bun/Deno, another crate, hot-unload, prebuilt artifacts, broad dispositions, or unaudited unsafe.
Task: Expose an ergonomic Rust Extension trait/ExtensionApi and export_extension! macro from the existing coding-agent crate, preserving the five-crate workspace. Compile a real fixture cdylib separately, load it, and exercise Pi-equivalent registration, invocation, UI/provider/event behavior and shutdown through ABI v1.
Run the stated validation responsibility. If blocked, stop with exact evidence and REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="sem-ca-v3d-source-install-provenance-r1"></a>
### SEM-CA-V3D-SOURCE-INSTALL-PROVENANCE-R1

Assignable: yes

Execution metadata:
- Wave: W2
- Context: fresh
- Depends on: SEM-CA-V3C-RUST-EXTENSION-SDK-FIXTURE-R1
- Can run in parallel with: none (ordered ABI/lifecycle contract)
- Must not run in parallel with: any Coding-agent writer

Scope boundaries:
- Goal: Implement the approved source-only extension installation norm: crate:<name>@<exact-version>, github:<owner>/<repo>@<resolved-commit>[#package], and path: development sources; mandatory local Cargo build, no prebuilt artifacts, locked/offline build after fetch, sanitized staging, sha2 0.10 provenance receipt, content-addressed artifact store, exact trust binding and rollback metadata.
- Non-goals: TypeScript/jiti compatibility, Stage 2, additional workspace crates, hot unload, prebuilt extension installation, neighboring units.
- Forbidden work: Rust trait objects/String/Vec/futures across ABI, broad dispositions, unaudited unsafe, marker tests, capability loss presented as parity.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify/create as required | `Cargo.lock` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/Cargo.toml` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/package-manager.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/source-info.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/extensions/install.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/extensions/provenance.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/extensions/mod.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/package-manager-cli.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/bun` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/package-manager.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/package-manager-ssh.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/package-command-paths.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/extension-package-manager.rs` | exclusive scope for this unit |
| modify/create as required | `.agents/port-manifests/exceptions.tsv` | exclusive scope for this unit |

Required context package:
- Plan references: Goal, Global Acceptance, Legacy policy, RF-S1, RF-S4, RF-S6, and this unit.
- Required decision/spec: `docs/porting/RUST_EXTENSION_ARCHITECTURE.md` in full.
- Frozen Pi references: `src/core/extensions/{types,loader,runner,wrapper,index}.ts`, `src/core/sdk.ts`, `src/core/{package-manager,resource-loader,skills,prompt-templates}.ts`, and mapped tests as relevant.
- Dependency outputs: exact predecessor result, changed API/ABI contracts, candidate SHA and logs.
- Neighboring out-of-scope units: all other IDs.

Implementation outline:
1. Read complete frozen behavior and current Rust callers before editing.
2. Implement only the owned layer against the approved Rust substitution; preserve all representable Pi ExtensionAPI behavior.
3. Add executable behavior tests, run only declared validation, and return exact evidence or a valid blocker classification.

Validation responsibility:
- Type: locally-validating
- Must run: cargo fmt --package zedflow-coding-agent --check
- Must NOT run: workspace-wide gates or out-of-scope edits.

Output contract:
- Structured PASS/BLOCKED result with base/candidate SHA, changed/reviewed files, executable tests, command evidence, ABI/security residuals, and one valid blocker classification when blocked.

Acceptance criteria:
- Implement the approved source-only extension installation norm: crate:<name>@<exact-version>, github:<owner>/<repo>@<resolved-commit>[#package], and path: development sources; mandatory local Cargo build, no prebuilt artifacts, locked/offline build after fetch, sanitized staging, sha2 0.10 provenance receipt, content-addressed artifact store, exact trust binding and rollback metadata.
- All ABI ownership/lifetime/trust rules in the architecture spec hold; no forbidden workaround or unapproved dependency.

Handoff to dependent units:
- Exact public contracts, ABI/message versions, artifact/receipt schema, candidate SHA and durable logs.

Subagent prompt:
```text
Implement only SEM-CA-V3D-SOURCE-INSTALL-PROVENANCE-R1 from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this unit and docs/porting/RUST_EXTENSION_ARCHITECTURE.md completely, then read the listed frozen Pi counterparts, current Rust callers/module roots and predecessor outputs.
Preserve maximum Pi ExtensionAPI capability alignment through the approved Rust cdylib substitution. Stay inside ownership. Do not add Bun/Deno, another crate, hot-unload, prebuilt artifacts, broad dispositions, or unaudited unsafe.
Task: Implement the approved source-only extension installation norm: crate:<name>@<exact-version>, github:<owner>/<repo>@<resolved-commit>[#package], and path: development sources; mandatory local Cargo build, no prebuilt artifacts, locked/offline build after fetch, sanitized staging, sha2 0.10 provenance receipt, content-addressed artifact store, exact trust binding and rollback metadata.
Run the stated validation responsibility. If blocked, stop with exact evidence and REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="sem-ca-v3e-remaining-resources-r1"></a>
### SEM-CA-V3E-REMAINING-RESOURCES-R1

Assignable: yes

Execution metadata:
- Wave: W2
- Context: fresh
- Depends on: SEM-CA-V3D-SOURCE-INSTALL-PROVENANCE-R1
- Can run in parallel with: none (ordered ABI/lifecycle contract)
- Must not run in parallel with: any Coding-agent writer

Scope boundaries:
- Goal: Port the remaining frozen Pi resources surface—atomic extension/resource reload, skills, prompt templates, keybindings, exports, footer/provider metadata, telemetry, timing and platform utilities—against the validated Rust extension runtime. Failed extension reload must preserve prior active state and surface diagnostics.
- Non-goals: TypeScript/jiti compatibility, Stage 2, additional workspace crates, hot unload, prebuilt extension installation, neighboring units.
- Forbidden work: Rust trait objects/String/Vec/futures across ABI, broad dispositions, unaudited unsafe, marker tests, capability loss presented as parity.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify/create as required | `crates/zedflow-coding-agent/src/core/export-html` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/footer-data-provider.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/http-dispatcher.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/index.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/keybindings.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/prompt-templates.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/provider-attribution.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/provider-display-names.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/resource-loader.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/skills.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/slash-commands.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/system-prompt.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/telemetry.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/timings.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/core/experimental.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/utils` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/resource-extension-runtime.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/resource-loader.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/skills.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/sdk-skills.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/prompt-templates.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/footer-data-provider.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/export-html-skill-block.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/export-html-whitespace.rs` | exclusive scope for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/export-html-xss.rs` | exclusive scope for this unit |

Required context package:
- Plan references: Goal, Global Acceptance, Legacy policy, RF-S1, RF-S4, RF-S6, and this unit.
- Required decision/spec: `docs/porting/RUST_EXTENSION_ARCHITECTURE.md` in full.
- Frozen Pi references: `src/core/extensions/{types,loader,runner,wrapper,index}.ts`, `src/core/sdk.ts`, `src/core/{package-manager,resource-loader,skills,prompt-templates}.ts`, and mapped tests as relevant.
- Dependency outputs: exact predecessor result, changed API/ABI contracts, candidate SHA and logs.
- Neighboring out-of-scope units: all other IDs.

Implementation outline:
1. Read complete frozen behavior and current Rust callers before editing.
2. Implement only the owned layer against the approved Rust substitution; preserve all representable Pi ExtensionAPI behavior.
3. Add executable behavior tests, run only declared validation, and return exact evidence or a valid blocker classification.

Validation responsibility:
- Type: locally-validating
- Must run: cargo fmt --package zedflow-coding-agent --check
- Must NOT run: workspace-wide gates or out-of-scope edits.

Output contract:
- Structured PASS/BLOCKED result with base/candidate SHA, changed/reviewed files, executable tests, command evidence, ABI/security residuals, and one valid blocker classification when blocked.

Acceptance criteria:
- Port the remaining frozen Pi resources surface—atomic extension/resource reload, skills, prompt templates, keybindings, exports, footer/provider metadata, telemetry, timing and platform utilities—against the validated Rust extension runtime. Failed extension reload must preserve prior active state and surface diagnostics.
- All ABI ownership/lifetime/trust rules in the architecture spec hold; no forbidden workaround or unapproved dependency.

Handoff to dependent units:
- Exact public contracts, ABI/message versions, artifact/receipt schema, candidate SHA and durable logs.

Subagent prompt:
```text
Implement only SEM-CA-V3E-REMAINING-RESOURCES-R1 from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this unit and docs/porting/RUST_EXTENSION_ARCHITECTURE.md completely, then read the listed frozen Pi counterparts, current Rust callers/module roots and predecessor outputs.
Preserve maximum Pi ExtensionAPI capability alignment through the approved Rust cdylib substitution. Stay inside ownership. Do not add Bun/Deno, another crate, hot-unload, prebuilt artifacts, broad dispositions, or unaudited unsafe.
Task: Port the remaining frozen Pi resources surface—atomic extension/resource reload, skills, prompt templates, keybindings, exports, footer/provider metadata, telemetry, timing and platform utilities—against the validated Rust extension runtime. Failed extension reload must preserve prior active state and surface diagnostics.
Run the stated validation responsibility. If blocked, stop with exact evidence and REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="sem-ca-v3f-rust-extensions-validate-r1"></a>
### SEM-CA-V3F-RUST-EXTENSIONS-VALIDATE-R1

Assignable: yes

Execution metadata:
- Wave: W2
- Context: fresh
- Depends on: SEM-CA-V3E-REMAINING-RESOURCES-R1
- Can run in parallel with: none (ordered ABI/lifecycle contract)
- Must not run in parallel with: any Coding-agent writer

Scope boundaries:
- Goal: Validate the approved Rust extension API, ABI/unsafe invariants, separately compiled fixture, source-only local build/provenance/trust, loader lifetime, executable handlers and resource integration. Do not accept compile-only fixtures, TS execution shortcuts, prebuilt artifacts or marker tests.
- Non-goals: TypeScript/jiti compatibility, Stage 2, additional workspace crates, hot unload, prebuilt extension installation, neighboring units.
- Forbidden work: Rust trait objects/String/Vec/futures across ABI, broad dispositions, unaudited unsafe, marker tests, capability loss presented as parity.

Files:
| Action | Path | Purpose |
|---|---|---|
| read only | exact predecessor candidate and frozen Pi extension sources/tests | validation/review only |

Required context package:
- Plan references: Goal, Global Acceptance, Legacy policy, RF-S1, RF-S4, RF-S6, and this unit.
- Required decision/spec: `docs/porting/RUST_EXTENSION_ARCHITECTURE.md` in full.
- Frozen Pi references: `src/core/extensions/{types,loader,runner,wrapper,index}.ts`, `src/core/sdk.ts`, `src/core/{package-manager,resource-loader,skills,prompt-templates}.ts`, and mapped tests as relevant.
- Dependency outputs: exact predecessor result, changed API/ABI contracts, candidate SHA and logs.
- Neighboring out-of-scope units: all other IDs.

Implementation outline:
1. Read complete frozen behavior and current Rust callers before editing.
2. Implement only the owned layer against the approved Rust substitution; preserve all representable Pi ExtensionAPI behavior.
3. Add executable behavior tests, run only declared validation, and return exact evidence or a valid blocker classification.

Validation responsibility:
- Type: integration-validating
- Must run: cargo fmt --package zedflow-coding-agent --check; cargo check -p zedflow-coding-agent --all-targets; cargo test -p zedflow-coding-agent --test extensions-abi-v1; cargo test -p zedflow-coding-agent --test extensions-loader-cdylib; cargo test -p zedflow-coding-agent --test extensions-sdk-build; cargo test -p zedflow-coding-agent --test extensions-runner; cargo test -p zedflow-coding-agent --test extensions-input-event; cargo test -p zedflow-coding-agent --test extension-package-manager; cargo test -p zedflow-coding-agent --test resource-extension-runtime
- Must NOT run: source edits.

Output contract:
- Structured PASS/BLOCKED result with base/candidate SHA, changed/reviewed files, executable tests, command evidence, ABI/security residuals, and one valid blocker classification when blocked.

Acceptance criteria:
- Validate the approved Rust extension API, ABI/unsafe invariants, separately compiled fixture, source-only local build/provenance/trust, loader lifetime, executable handlers and resource integration. Do not accept compile-only fixtures, TS execution shortcuts, prebuilt artifacts or marker tests.
- All ABI ownership/lifetime/trust rules in the architecture spec hold; no forbidden workaround or unapproved dependency.

Handoff to dependent units:
- Exact public contracts, ABI/message versions, artifact/receipt schema, candidate SHA and durable logs.

Subagent prompt:
```text
Implement only SEM-CA-V3F-RUST-EXTENSIONS-VALIDATE-R1 from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this unit and docs/porting/RUST_EXTENSION_ARCHITECTURE.md completely, then read the listed frozen Pi counterparts, current Rust callers/module roots and predecessor outputs.
Preserve maximum Pi ExtensionAPI capability alignment through the approved Rust cdylib substitution. Stay inside ownership. Do not add Bun/Deno, another crate, hot-unload, prebuilt artifacts, broad dispositions, or unaudited unsafe.
Task: Validate the approved Rust extension API, ABI/unsafe invariants, separately compiled fixture, source-only local build/provenance/trust, loader lifetime, executable handlers and resource integration. Do not accept compile-only fixtures, TS execution shortcuts, prebuilt artifacts or marker tests.
Run the stated validation responsibility. If blocked, stop with exact evidence and REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="sem-ca-v3g-rust-extensions-fidelity-r1"></a>
### SEM-CA-V3G-RUST-EXTENSIONS-FIDELITY-R1

Assignable: yes

Execution metadata:
- Wave: W2
- Context: fresh
- Depends on: SEM-CA-V3F-RUST-EXTENSIONS-VALIDATE-R1
- Can run in parallel with: none (ordered ABI/lifecycle contract)
- Must not run in parallel with: any Coding-agent writer

Scope boundaries:
- Goal: Independently verify maximum Pi ExtensionAPI capability alignment through Rust extensions while accepting the approved language/loading substitution. Review trust/digest/TOCTOU handling, C ABI soundness, panic/reentrancy/cancellation/stale handles, process-lifetime no-unload, source-only crates.io/GitHub install and local build receipts. TypeScript compatibility is deferred, not silently claimed.
- Non-goals: TypeScript/jiti compatibility, Stage 2, additional workspace crates, hot unload, prebuilt extension installation, neighboring units.
- Forbidden work: Rust trait objects/String/Vec/futures across ABI, broad dispositions, unaudited unsafe, marker tests, capability loss presented as parity.

Files:
| Action | Path | Purpose |
|---|---|---|
| read only | exact predecessor candidate and frozen Pi extension sources/tests | validation/review only |

Required context package:
- Plan references: Goal, Global Acceptance, Legacy policy, RF-S1, RF-S4, RF-S6, and this unit.
- Required decision/spec: `docs/porting/RUST_EXTENSION_ARCHITECTURE.md` in full.
- Frozen Pi references: `src/core/extensions/{types,loader,runner,wrapper,index}.ts`, `src/core/sdk.ts`, `src/core/{package-manager,resource-loader,skills,prompt-templates}.ts`, and mapped tests as relevant.
- Dependency outputs: exact predecessor result, changed API/ABI contracts, candidate SHA and logs.
- Neighboring out-of-scope units: all other IDs.

Implementation outline:
1. Read complete frozen behavior and current Rust callers before editing.
2. Implement only the owned layer against the approved Rust substitution; preserve all representable Pi ExtensionAPI behavior.
3. Add executable behavior tests, run only declared validation, and return exact evidence or a valid blocker classification.

Validation responsibility:
- Type: integration-validating
- Must run: independent source/behavior/security review necessary for the stated intent
- Must NOT run: source edits.

Output contract:
- Structured PASS/BLOCKED result with base/candidate SHA, changed/reviewed files, executable tests, command evidence, ABI/security residuals, and one valid blocker classification when blocked.

Acceptance criteria:
- Independently verify maximum Pi ExtensionAPI capability alignment through Rust extensions while accepting the approved language/loading substitution. Review trust/digest/TOCTOU handling, C ABI soundness, panic/reentrancy/cancellation/stale handles, process-lifetime no-unload, source-only crates.io/GitHub install and local build receipts. TypeScript compatibility is deferred, not silently claimed.
- All ABI ownership/lifetime/trust rules in the architecture spec hold; no forbidden workaround or unapproved dependency.

Handoff to dependent units:
- Exact public contracts, ABI/message versions, artifact/receipt schema, candidate SHA and durable logs.

Subagent prompt:
```text
Implement only SEM-CA-V3G-RUST-EXTENSIONS-FIDELITY-R1 from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this unit and docs/porting/RUST_EXTENSION_ARCHITECTURE.md completely, then read the listed frozen Pi counterparts, current Rust callers/module roots and predecessor outputs.
Preserve maximum Pi ExtensionAPI capability alignment through the approved Rust cdylib substitution. Stay inside ownership. Do not add Bun/Deno, another crate, hot-unload, prebuilt artifacts, broad dispositions, or unaudited unsafe.
Task: Independently verify maximum Pi ExtensionAPI capability alignment through Rust extensions while accepting the approved language/loading substitution. Review trust/digest/TOCTOU handling, C ABI soundness, panic/reentrancy/cancellation/stale handles, process-lifetime no-unload, source-only crates.io/GitHub install and local build receipts. TypeScript compatibility is deferred, not silently claimed.
Run the stated validation responsibility. If blocked, stop with exact evidence and REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="sem-ca-v4-interactive"></a>
### SEM-CA-V4-INTERACTIVE

Assignable: yes

Execution metadata:
- Wave: W2
- Context: fresh
- Depends on: SEM-CA-V3G-RUST-EXTENSIONS-FIDELITY-R1
- Can run in parallel with: only disjoint AI/Agent residual peer where applicable
- Must not run in parallel with: any unit sharing an ownership prefix

Scope boundaries:
- Goal: Port and wire the full Pi default interactive mode through zedflow-tui: streaming, tool execution, selectors, compaction, themes, startup, session tree, auth and extension UI. The minimal line REPL is not acceptable.
- Non-goals: neighboring units, Stage 2, promotion, unapproved dependencies.
- Forbidden work: placeholders, vacuous tests, broad exceptions, frozen Pi mutation, out-of-scope cleanup.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify/create as required | `crates/zedflow-coding-agent/src/modes/interactive` | exclusive controller ownership for this unit |

Required context package:
- Plan references: this unit, Goal, Global Acceptance, Legacy policy, RF-S1 through RF-S6 as applicable.
- Required files: owned Rust paths and their one-to-one files/tests under `references/pi/packages/` at frozen SHA `2b00dade7cec918aefb025c8b7a4fa304a30acdd`.
- Dependency outputs: predecessor result, candidate SHA, declared validation logs.
- Neighboring out-of-scope units: all other IDs in this plan.

Implementation outline:
1. Read the complete frozen Pi counterpart and all Rust callers/exports before editing.
2. Implement observable behavior at the shared root cause; wire modules and real tests.
3. Run only declared/local checks; return a structured result with evidence and remaining blocker classification.

Validation responsibility:
- Type: locally-validating
- Must run: cargo fmt --package zedflow-coding-agent --check
- Must NOT run: workspace-wide gates or mutate files outside ownership.

Output contract:
- Structured PASS/BLOCKED result, exact base/candidate SHA, changed files or reviewed scope, commands/evidence, residual risks, and one valid blocker classification when not accepted.

Acceptance criteria:
- Port and wire the full Pi default interactive mode through zedflow-tui: streaming, tool execution, selectors, compaction, themes, startup, session tree, auth and extension UI. The minimal line REPL is not acceptable.
- No forbidden workaround or out-of-scope edit; all declared commands pass.

Handoff to dependent units:
- Commit/result SHA and durable validation logs; name any intentionally deferred behavior and its owning downstream ID.

Subagent prompt:
```text
Implement only SEM-CA-V4-INTERACTIVE from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this entire unit, its listed frozen Pi counterparts, Rust callers/module roots, dependency outputs, and the global acceptance/legacy rules before editing.
Task: Port and wire the full Pi default interactive mode through zedflow-tui: streaming, tool execution, selectors, compaction, themes, startup, session tree, auth and extension UI. The minimal line REPL is not acceptable.
Stay inside ownership. Do not implement neighboring units or Stage 2. Do not use marker code, vacuous tests, broad exceptions, or unapproved dependency substitutions.
Run the stated validation responsibility. If blocked, stop and return exact evidence with REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="sem-ca-v5-cli-modes"></a>
### SEM-CA-V5-CLI-MODES

Assignable: yes

Execution metadata:
- Wave: W2
- Context: fresh
- Depends on: SEM-CA-V4-INTERACTIVE
- Can run in parallel with: only disjoint AI/Agent residual peer where applicable
- Must not run in parallel with: any unit sharing an ownership prefix

Scope boundaries:
- Goal: Wire every advertised Pi CLI route: default TUI, --print, text/json, RPC, sessions, list-models, export, package operations and startup selectors. Remove every not-wired path.
- Non-goals: neighboring units, Stage 2, promotion, unapproved dependencies.
- Forbidden work: placeholders, vacuous tests, broad exceptions, frozen Pi mutation, out-of-scope cleanup.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify/create as required | `crates/zedflow-coding-agent/src/main.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/lib.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/index.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/cli.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/cli` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/modes/mod.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/modes/index.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/modes/print-mode.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/modes/rpc` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/rpc-entry.rs` | exclusive controller ownership for this unit |

Required context package:
- Plan references: this unit, Goal, Global Acceptance, Legacy policy, RF-S1 through RF-S6 as applicable.
- Required files: owned Rust paths and their one-to-one files/tests under `references/pi/packages/` at frozen SHA `2b00dade7cec918aefb025c8b7a4fa304a30acdd`.
- Dependency outputs: predecessor result, candidate SHA, declared validation logs.
- Neighboring out-of-scope units: all other IDs in this plan.

Implementation outline:
1. Read the complete frozen Pi counterpart and all Rust callers/exports before editing.
2. Implement observable behavior at the shared root cause; wire modules and real tests.
3. Run only declared/local checks; return a structured result with evidence and remaining blocker classification.

Validation responsibility:
- Type: locally-validating
- Must run: cargo fmt --package zedflow-coding-agent --check
- Must NOT run: workspace-wide gates or mutate files outside ownership.

Output contract:
- Structured PASS/BLOCKED result, exact base/candidate SHA, changed files or reviewed scope, commands/evidence, residual risks, and one valid blocker classification when not accepted.

Acceptance criteria:
- Wire every advertised Pi CLI route: default TUI, --print, text/json, RPC, sessions, list-models, export, package operations and startup selectors. Remove every not-wired path.
- No forbidden workaround or out-of-scope edit; all declared commands pass.

Handoff to dependent units:
- Commit/result SHA and durable validation logs; name any intentionally deferred behavior and its owning downstream ID.

Subagent prompt:
```text
Implement only SEM-CA-V5-CLI-MODES from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this entire unit, its listed frozen Pi counterparts, Rust callers/module roots, dependency outputs, and the global acceptance/legacy rules before editing.
Task: Wire every advertised Pi CLI route: default TUI, --print, text/json, RPC, sessions, list-models, export, package operations and startup selectors. Remove every not-wired path.
Stay inside ownership. Do not implement neighboring units or Stage 2. Do not use marker code, vacuous tests, broad exceptions, or unapproved dependency substitutions.
Run the stated validation responsibility. If blocked, stop and return exact evidence with REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="sem-ca-v6-test-batch-1"></a>
### SEM-CA-V6-TEST-BATCH-1

Assignable: yes

Execution metadata:
- Wave: W2
- Context: fresh
- Depends on: SEM-CA-V5-CLI-MODES
- Can run in parallel with: only disjoint AI/Agent residual peer where applicable
- Must not run in parallel with: any unit sharing an ownership prefix

Scope boundaries:
- Goal: Convert frozen Pi Coding-agent test batch 1 into real executable Rust tests. TEST_PATH constants, empty test targets, blanket ignores and source-path-only assertions do not count. Exercise the actual public/runtime behavior.
- Non-goals: neighboring units, Stage 2, promotion, unapproved dependencies.
- Forbidden work: placeholders, vacuous tests, broad exceptions, frozen Pi mutation, out-of-scope cleanup.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify/create as required | `crates/zedflow-coding-agent/tests/agent-session-auto-compaction-queue.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/agent-session-branching.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/agent-session-compaction.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/agent-session-concurrent.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/agent-session-dynamic-provider.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/agent-session-dynamic-tools.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/agent-session-retry.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/agent-session-runtime-events.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/agent-session-stats.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/agent-session-tree-navigation.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/ansi-utils.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/args.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/assistant-message.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/bash-close-hang-windows.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/bash-execution-width.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/block-images.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/changelog.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/clipboard-image-bmp-conversion.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/clipboard-image.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/clipboard-native.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/clipboard.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/compaction-extensions-example.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/compaction-extensions.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/compaction-serialization.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/compaction-summary-reasoning.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/compaction.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/config-value-migration.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/config.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/edit-tool-legacy-input.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/edit-tool-no-full-redraw.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/experimental.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/export-html-skill-block.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/export-html-whitespace.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/export-html-xss.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/extensions-discovery.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/lib.rs` | exclusive controller ownership for this unit |

Required context package:
- Plan references: this unit, Goal, Global Acceptance, Legacy policy, RF-S1 through RF-S6 as applicable.
- Required files: owned Rust paths and their one-to-one files/tests under `references/pi/packages/` at frozen SHA `2b00dade7cec918aefb025c8b7a4fa304a30acdd`.
- Dependency outputs: predecessor result, candidate SHA, declared validation logs.
- Neighboring out-of-scope units: all other IDs in this plan.

Implementation outline:
1. Read the complete frozen Pi counterpart and all Rust callers/exports before editing.
2. Implement observable behavior at the shared root cause; wire modules and real tests.
3. Run only declared/local checks; return a structured result with evidence and remaining blocker classification.

Validation responsibility:
- Type: locally-validating
- Must run: cargo fmt --package zedflow-coding-agent --check
- Must NOT run: workspace-wide gates or mutate files outside ownership.

Output contract:
- Structured PASS/BLOCKED result, exact base/candidate SHA, changed files or reviewed scope, commands/evidence, residual risks, and one valid blocker classification when not accepted.

Acceptance criteria:
- Convert frozen Pi Coding-agent test batch 1 into real executable Rust tests. TEST_PATH constants, empty test targets, blanket ignores and source-path-only assertions do not count. Exercise the actual public/runtime behavior.
- No forbidden workaround or out-of-scope edit; all declared commands pass.

Handoff to dependent units:
- Commit/result SHA and durable validation logs; name any intentionally deferred behavior and its owning downstream ID.

Subagent prompt:
```text
Implement only SEM-CA-V6-TEST-BATCH-1 from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this entire unit, its listed frozen Pi counterparts, Rust callers/module roots, dependency outputs, and the global acceptance/legacy rules before editing.
Task: Convert frozen Pi Coding-agent test batch 1 into real executable Rust tests. TEST_PATH constants, empty test targets, blanket ignores and source-path-only assertions do not count. Exercise the actual public/runtime behavior.
Stay inside ownership. Do not implement neighboring units or Stage 2. Do not use marker code, vacuous tests, broad exceptions, or unapproved dependency substitutions.
Run the stated validation responsibility. If blocked, stop and return exact evidence with REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="sem-ca-v7-test-batch-2"></a>
### SEM-CA-V7-TEST-BATCH-2

Assignable: yes

Execution metadata:
- Wave: W2
- Context: fresh
- Depends on: SEM-CA-V6-TEST-BATCH-1
- Can run in parallel with: only disjoint AI/Agent residual peer where applicable
- Must not run in parallel with: any unit sharing an ownership prefix

Scope boundaries:
- Goal: Convert frozen Pi Coding-agent test batch 2 into real executable Rust tests. TEST_PATH constants, empty test targets, blanket ignores and source-path-only assertions do not count. Exercise the actual public/runtime behavior.
- Non-goals: neighboring units, Stage 2, promotion, unapproved dependencies.
- Forbidden work: placeholders, vacuous tests, broad exceptions, frozen Pi mutation, out-of-scope cleanup.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify/create as required | `crates/zedflow-coding-agent/tests/extensions-input-event.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/extensions-runner.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/file-mutation-queue.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/first-time-setup-fork.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/first-time-setup.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/footer-data-provider.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/footer-width.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/format-resume-command.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/frontmatter.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/git-merge-and-resolve-extension.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/git-ssh-url.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/git-update.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/image-processing.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/image-resize-callers.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/initial-message.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/input-transform-streaming-example.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/interactive-mode-anthropic-warning.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/interactive-mode-clone-command.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/interactive-mode-compaction.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/interactive-mode-import-command.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/interactive-mode-startup-input.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/interactive-mode-status.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/interactive-mode-suspend.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/keybindings-migration.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/oauth-selector.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/package-command-paths.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/package-manager-ssh.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/package-manager.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/path-utils.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/paths.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/pi-user-agent.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/plan-mode-extension.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/plan-mode-utils.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/print-mode.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/prompt-templates.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/lib.rs` | exclusive controller ownership for this unit |

Required context package:
- Plan references: this unit, Goal, Global Acceptance, Legacy policy, RF-S1 through RF-S6 as applicable.
- Required files: owned Rust paths and their one-to-one files/tests under `references/pi/packages/` at frozen SHA `2b00dade7cec918aefb025c8b7a4fa304a30acdd`.
- Dependency outputs: predecessor result, candidate SHA, declared validation logs.
- Neighboring out-of-scope units: all other IDs in this plan.

Implementation outline:
1. Read the complete frozen Pi counterpart and all Rust callers/exports before editing.
2. Implement observable behavior at the shared root cause; wire modules and real tests.
3. Run only declared/local checks; return a structured result with evidence and remaining blocker classification.

Validation responsibility:
- Type: locally-validating
- Must run: cargo fmt --package zedflow-coding-agent --check
- Must NOT run: workspace-wide gates or mutate files outside ownership.

Output contract:
- Structured PASS/BLOCKED result, exact base/candidate SHA, changed files or reviewed scope, commands/evidence, residual risks, and one valid blocker classification when not accepted.

Acceptance criteria:
- Convert frozen Pi Coding-agent test batch 2 into real executable Rust tests. TEST_PATH constants, empty test targets, blanket ignores and source-path-only assertions do not count. Exercise the actual public/runtime behavior.
- No forbidden workaround or out-of-scope edit; all declared commands pass.

Handoff to dependent units:
- Commit/result SHA and durable validation logs; name any intentionally deferred behavior and its owning downstream ID.

Subagent prompt:
```text
Implement only SEM-CA-V7-TEST-BATCH-2 from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this entire unit, its listed frozen Pi counterparts, Rust callers/module roots, dependency outputs, and the global acceptance/legacy rules before editing.
Task: Convert frozen Pi Coding-agent test batch 2 into real executable Rust tests. TEST_PATH constants, empty test targets, blanket ignores and source-path-only assertions do not count. Exercise the actual public/runtime behavior.
Stay inside ownership. Do not implement neighboring units or Stage 2. Do not use marker code, vacuous tests, broad exceptions, or unapproved dependency substitutions.
Run the stated validation responsibility. If blocked, stop and return exact evidence with REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="sem-ca-v8-test-batch-3"></a>
### SEM-CA-V8-TEST-BATCH-3

Assignable: yes

Execution metadata:
- Wave: W2
- Context: fresh
- Depends on: SEM-CA-V7-TEST-BATCH-2
- Can run in parallel with: only disjoint AI/Agent residual peer where applicable
- Must not run in parallel with: any unit sharing an ownership prefix

Scope boundaries:
- Goal: Convert frozen Pi Coding-agent test batch 3 into real executable Rust tests. TEST_PATH constants, empty test targets, blanket ignores and source-path-only assertions do not count. Exercise the actual public/runtime behavior.
- Non-goals: neighboring units, Stage 2, promotion, unapproved dependencies.
- Forbidden work: placeholders, vacuous tests, broad exceptions, frozen Pi mutation, out-of-scope cleanup.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify/create as required | `crates/zedflow-coding-agent/tests/resource-loader.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/restore-sandbox-env.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/rpc-client-clone.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/rpc-client-process-exit.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/rpc-example.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/rpc-jsonl.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/rpc-prompt-response-semantics.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/rpc.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/sdk-codex-cache-probe-tool-loop.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/sdk-openrouter-attribution.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/sdk-session-manager.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/sdk-skills.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/sdk-stream-options.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/session-cwd.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/session-file-invalid.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/session-id-readonly.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/session-info-modified-timestamp.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/session-manager/build-context.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/session-manager/custom-session-id.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/session-manager/file-operations.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/session-manager/labels.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/session-manager/migration.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/session-manager/save-entry.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/session-manager/tree-traversal.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/session-selector-path-delete.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/session-selector-rename.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/session-selector-search.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/settings-manager-bug.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/settings-manager.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/skills.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/startup-session-name.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/status-indicator.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/stdout-cleanliness.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/streaming-render-debug.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/agent-session-bash-persistence.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/lib.rs` | exclusive controller ownership for this unit |

Required context package:
- Plan references: this unit, Goal, Global Acceptance, Legacy policy, RF-S1 through RF-S6 as applicable.
- Required files: owned Rust paths and their one-to-one files/tests under `references/pi/packages/` at frozen SHA `2b00dade7cec918aefb025c8b7a4fa304a30acdd`.
- Dependency outputs: predecessor result, candidate SHA, declared validation logs.
- Neighboring out-of-scope units: all other IDs in this plan.

Implementation outline:
1. Read the complete frozen Pi counterpart and all Rust callers/exports before editing.
2. Implement observable behavior at the shared root cause; wire modules and real tests.
3. Run only declared/local checks; return a structured result with evidence and remaining blocker classification.

Validation responsibility:
- Type: locally-validating
- Must run: cargo fmt --package zedflow-coding-agent --check
- Must NOT run: workspace-wide gates or mutate files outside ownership.

Output contract:
- Structured PASS/BLOCKED result, exact base/candidate SHA, changed files or reviewed scope, commands/evidence, residual risks, and one valid blocker classification when not accepted.

Acceptance criteria:
- Convert frozen Pi Coding-agent test batch 3 into real executable Rust tests. TEST_PATH constants, empty test targets, blanket ignores and source-path-only assertions do not count. Exercise the actual public/runtime behavior.
- No forbidden workaround or out-of-scope edit; all declared commands pass.

Handoff to dependent units:
- Commit/result SHA and durable validation logs; name any intentionally deferred behavior and its owning downstream ID.

Subagent prompt:
```text
Implement only SEM-CA-V8-TEST-BATCH-3 from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this entire unit, its listed frozen Pi counterparts, Rust callers/module roots, dependency outputs, and the global acceptance/legacy rules before editing.
Task: Convert frozen Pi Coding-agent test batch 3 into real executable Rust tests. TEST_PATH constants, empty test targets, blanket ignores and source-path-only assertions do not count. Exercise the actual public/runtime behavior.
Stay inside ownership. Do not implement neighboring units or Stage 2. Do not use marker code, vacuous tests, broad exceptions, or unapproved dependency substitutions.
Run the stated validation responsibility. If blocked, stop and return exact evidence with REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="sem-ca-v9-test-batch-4"></a>
### SEM-CA-V9-TEST-BATCH-4

Assignable: yes

Execution metadata:
- Wave: W2
- Context: fresh
- Depends on: SEM-CA-V8-TEST-BATCH-3
- Can run in parallel with: only disjoint AI/Agent residual peer where applicable
- Must not run in parallel with: any unit sharing an ownership prefix

Scope boundaries:
- Goal: Convert frozen Pi Coding-agent test batch 4 into real executable Rust tests. TEST_PATH constants, empty test targets, blanket ignores and source-path-only assertions do not count. Exercise the actual public/runtime behavior.
- Non-goals: neighboring units, Stage 2, promotion, unapproved dependencies.
- Forbidden work: placeholders, vacuous tests, broad exceptions, frozen Pi mutation, out-of-scope cleanup.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/agent-session-compaction.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/agent-session-model-extension.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/agent-session-prompt.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/agent-session-queue.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/agent-session-retry-events.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/agent-session-runtime.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/harness.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/lax-message-content.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/regressions/1717-2113-agent-session-event-settlement.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/regressions/2023-queued-slash-command-followup.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/regressions/2753-reload-stale-resource-settings.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/regressions/2781-skill-collision-precedence.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/regressions/2791-fswatch-error-crash.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/regressions/2835-tools-allowlist-filters-extension-tools.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/regressions/2860-replaced-session-context.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/regressions/3217-scoped-model-order.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/regressions/3302-find-path-glob.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/regressions/3303-find-nested-gitignore.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/regressions/3317-network-connection-lost-retry.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/regressions/3592-no-builtin-tools-keeps-extension-tools.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/regressions/3616-settings-inmemory-reload.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/regressions/3686-session-name-event.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/regressions/3688-tree-cancel-compacting.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/regressions/3982-message-end-cost-override.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/regressions/4167-thinking-toggle-pending-tool-render.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/regressions/5080-signal-shutdown-extension-cleanup.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/regressions/5109-exclude-tools.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/regressions/5208-late-bash-output.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/regressions/5217-compaction-reason.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/regressions/5303-bash-output-truncation.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/regressions/5433-extension-oauth-prompt-input.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/regressions/5596-missing-theme-export.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/regressions/5661-uppercase-header-values.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/regressions/5724-sigterm-signal-exit.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/regressions/5868-rpc-unknown-command-id.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/lib.rs` | exclusive controller ownership for this unit |

Required context package:
- Plan references: this unit, Goal, Global Acceptance, Legacy policy, RF-S1 through RF-S6 as applicable.
- Required files: owned Rust paths and their one-to-one files/tests under `references/pi/packages/` at frozen SHA `2b00dade7cec918aefb025c8b7a4fa304a30acdd`.
- Dependency outputs: predecessor result, candidate SHA, declared validation logs.
- Neighboring out-of-scope units: all other IDs in this plan.

Implementation outline:
1. Read the complete frozen Pi counterpart and all Rust callers/exports before editing.
2. Implement observable behavior at the shared root cause; wire modules and real tests.
3. Run only declared/local checks; return a structured result with evidence and remaining blocker classification.

Validation responsibility:
- Type: locally-validating
- Must run: cargo fmt --package zedflow-coding-agent --check
- Must NOT run: workspace-wide gates or mutate files outside ownership.

Output contract:
- Structured PASS/BLOCKED result, exact base/candidate SHA, changed files or reviewed scope, commands/evidence, residual risks, and one valid blocker classification when not accepted.

Acceptance criteria:
- Convert frozen Pi Coding-agent test batch 4 into real executable Rust tests. TEST_PATH constants, empty test targets, blanket ignores and source-path-only assertions do not count. Exercise the actual public/runtime behavior.
- No forbidden workaround or out-of-scope edit; all declared commands pass.

Handoff to dependent units:
- Commit/result SHA and durable validation logs; name any intentionally deferred behavior and its owning downstream ID.

Subagent prompt:
```text
Implement only SEM-CA-V9-TEST-BATCH-4 from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this entire unit, its listed frozen Pi counterparts, Rust callers/module roots, dependency outputs, and the global acceptance/legacy rules before editing.
Task: Convert frozen Pi Coding-agent test batch 4 into real executable Rust tests. TEST_PATH constants, empty test targets, blanket ignores and source-path-only assertions do not count. Exercise the actual public/runtime behavior.
Stay inside ownership. Do not implement neighboring units or Stage 2. Do not use marker code, vacuous tests, broad exceptions, or unapproved dependency substitutions.
Run the stated validation responsibility. If blocked, stop and return exact evidence with REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="sem-ca-v10-test-batch-5"></a>
### SEM-CA-V10-TEST-BATCH-5

Assignable: yes

Execution metadata:
- Wave: W2
- Context: fresh
- Depends on: SEM-CA-V9-TEST-BATCH-4
- Can run in parallel with: only disjoint AI/Agent residual peer where applicable
- Must not run in parallel with: any unit sharing an ownership prefix

Scope boundaries:
- Goal: Convert frozen Pi Coding-agent test batch 5 into real executable Rust tests. TEST_PATH constants, empty test targets, blanket ignores and source-path-only assertions do not count. Exercise the actual public/runtime behavior.
- Non-goals: neighboring units, Stage 2, promotion, unapproved dependencies.
- Forbidden work: placeholders, vacuous tests, broad exceptions, frozen Pi mutation, out-of-scope cleanup.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/regressions/5943-session-start-notify.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/regressions/5996-session-name-newlines.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/regressions/6019-explicit-provider-retry-message.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/regressions/6162-extension-active-tools-next-turn.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/regressions/6260-inline-extension-naming.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/regressions/extension-factory-cache.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/suite/regressions/pre-prompt-compaction-no-continue.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/syntax-highlight.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/system-prompt.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/test-harness.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/test-theme-colors.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/theme-detection.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/theme-export.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/theme-picker.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/tool-execution-component.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/tools.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/tree-selector.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/trigger-compact-extension.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/truncate-to-width.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/trust-manager.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/trust-selector.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/user-message.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/utilities.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/tests/version-check.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-coding-agent/src/lib.rs` | exclusive controller ownership for this unit |

Required context package:
- Plan references: this unit, Goal, Global Acceptance, Legacy policy, RF-S1 through RF-S6 as applicable.
- Required files: owned Rust paths and their one-to-one files/tests under `references/pi/packages/` at frozen SHA `2b00dade7cec918aefb025c8b7a4fa304a30acdd`.
- Dependency outputs: predecessor result, candidate SHA, declared validation logs.
- Neighboring out-of-scope units: all other IDs in this plan.

Implementation outline:
1. Read the complete frozen Pi counterpart and all Rust callers/exports before editing.
2. Implement observable behavior at the shared root cause; wire modules and real tests.
3. Run only declared/local checks; return a structured result with evidence and remaining blocker classification.

Validation responsibility:
- Type: locally-validating
- Must run: cargo fmt --package zedflow-coding-agent --check
- Must NOT run: workspace-wide gates or mutate files outside ownership.

Output contract:
- Structured PASS/BLOCKED result, exact base/candidate SHA, changed files or reviewed scope, commands/evidence, residual risks, and one valid blocker classification when not accepted.

Acceptance criteria:
- Convert frozen Pi Coding-agent test batch 5 into real executable Rust tests. TEST_PATH constants, empty test targets, blanket ignores and source-path-only assertions do not count. Exercise the actual public/runtime behavior.
- No forbidden workaround or out-of-scope edit; all declared commands pass.

Handoff to dependent units:
- Commit/result SHA and durable validation logs; name any intentionally deferred behavior and its owning downstream ID.

Subagent prompt:
```text
Implement only SEM-CA-V10-TEST-BATCH-5 from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this entire unit, its listed frozen Pi counterparts, Rust callers/module roots, dependency outputs, and the global acceptance/legacy rules before editing.
Task: Convert frozen Pi Coding-agent test batch 5 into real executable Rust tests. TEST_PATH constants, empty test targets, blanket ignores and source-path-only assertions do not count. Exercise the actual public/runtime behavior.
Stay inside ownership. Do not implement neighboring units or Stage 2. Do not use marker code, vacuous tests, broad exceptions, or unapproved dependency substitutions.
Run the stated validation responsibility. If blocked, stop and return exact evidence with REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="sem-ca-v11-validate"></a>
### SEM-CA-V11-VALIDATE

Assignable: yes

Execution metadata:
- Wave: W2
- Context: fresh
- Depends on: SEM-CA-V10-TEST-BATCH-5
- Can run in parallel with: only disjoint AI/Agent residual peer where applicable
- Must not run in parallel with: any unit sharing an ownership prefix

Scope boundaries:
- Goal: Validate executable Coding-agent tests, strict manifest closure and all runtime modes. No source edits.
- Non-goals: neighboring units, Stage 2, promotion, unapproved dependencies.
- Forbidden work: placeholders, vacuous tests, broad exceptions, frozen Pi mutation, out-of-scope cleanup.

Files:
| Action | Path | Purpose |
|---|---|---|
| read only | exact predecessor candidate | validation/review only |

Required context package:
- Plan references: this unit, Goal, Global Acceptance, Legacy policy, RF-S1 through RF-S6 as applicable.
- Required files: owned Rust paths and their one-to-one files/tests under `references/pi/packages/` at frozen SHA `2b00dade7cec918aefb025c8b7a4fa304a30acdd`.
- Dependency outputs: predecessor result, candidate SHA, declared validation logs.
- Neighboring out-of-scope units: all other IDs in this plan.

Implementation outline:
1. Read the complete frozen Pi counterpart and all Rust callers/exports before editing.
2. Implement observable behavior at the shared root cause; wire modules and real tests.
3. Run only declared/local checks; return a structured result with evidence and remaining blocker classification.

Validation responsibility:
- Type: integration-validating
- Must run: cargo fmt --package zedflow-coding-agent --check; cargo check -p zedflow-coding-agent --all-targets; cargo test -p zedflow-coding-agent --all-targets; python3 tools/pi-port-swarm/manifest.py check --package zedflow-coding-agent
- Must NOT run: source edits.

Output contract:
- Structured PASS/BLOCKED result, exact base/candidate SHA, changed files or reviewed scope, commands/evidence, residual risks, and one valid blocker classification when not accepted.

Acceptance criteria:
- Validate executable Coding-agent tests, strict manifest closure and all runtime modes. No source edits.
- No forbidden workaround or out-of-scope edit; all declared commands pass.

Handoff to dependent units:
- Commit/result SHA and durable validation logs; name any intentionally deferred behavior and its owning downstream ID.

Subagent prompt:
```text
Implement only SEM-CA-V11-VALIDATE from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this entire unit, its listed frozen Pi counterparts, Rust callers/module roots, dependency outputs, and the global acceptance/legacy rules before editing.
Task: Validate executable Coding-agent tests, strict manifest closure and all runtime modes. No source edits.
Stay inside ownership. Do not implement neighboring units or Stage 2. Do not use marker code, vacuous tests, broad exceptions, or unapproved dependency substitutions.
Run the stated validation responsibility. If blocked, stop and return exact evidence with REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="sem-ca-v12-fidelity"></a>
### SEM-CA-V12-FIDELITY

Assignable: yes

Execution metadata:
- Wave: W2
- Context: fresh
- Depends on: SEM-CA-V11-VALIDATE
- Can run in parallel with: only disjoint AI/Agent residual peer where applicable
- Must not run in parallel with: any unit sharing an ownership prefix

Scope boundaries:
- Goal: Independently compare frozen Pi Coding-agent with Rust, including TUI default, print/text/json, RPC, sessions, tools, compaction, extensions, themes, skills and package management. Reject marker modules and vacuous tests.
- Non-goals: neighboring units, Stage 2, promotion, unapproved dependencies.
- Forbidden work: placeholders, vacuous tests, broad exceptions, frozen Pi mutation, out-of-scope cleanup.

Files:
| Action | Path | Purpose |
|---|---|---|
| read only | exact predecessor candidate | validation/review only |

Required context package:
- Plan references: this unit, Goal, Global Acceptance, Legacy policy, RF-S1 through RF-S6 as applicable.
- Required files: owned Rust paths and their one-to-one files/tests under `references/pi/packages/` at frozen SHA `2b00dade7cec918aefb025c8b7a4fa304a30acdd`.
- Dependency outputs: predecessor result, candidate SHA, declared validation logs.
- Neighboring out-of-scope units: all other IDs in this plan.

Implementation outline:
1. Read the complete frozen Pi counterpart and all Rust callers/exports before editing.
2. Implement observable behavior at the shared root cause; wire modules and real tests.
3. Run only declared/local checks; return a structured result with evidence and remaining blocker classification.

Validation responsibility:
- Type: integration-validating
- Must run: independent commands necessary to establish the stated review intent
- Must NOT run: source edits.

Output contract:
- Structured PASS/BLOCKED result, exact base/candidate SHA, changed files or reviewed scope, commands/evidence, residual risks, and one valid blocker classification when not accepted.

Acceptance criteria:
- Independently compare frozen Pi Coding-agent with Rust, including TUI default, print/text/json, RPC, sessions, tools, compaction, extensions, themes, skills and package management. Reject marker modules and vacuous tests.
- No forbidden workaround or out-of-scope edit; all declared commands pass.

Handoff to dependent units:
- Commit/result SHA and durable validation logs; name any intentionally deferred behavior and its owning downstream ID.

Subagent prompt:
```text
Implement only SEM-CA-V12-FIDELITY from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this entire unit, its listed frozen Pi counterparts, Rust callers/module roots, dependency outputs, and the global acceptance/legacy rules before editing.
Task: Independently compare frozen Pi Coding-agent with Rust, including TUI default, print/text/json, RPC, sessions, tools, compaction, extensions, themes, skills and package management. Reject marker modules and vacuous tests.
Stay inside ownership. Do not implement neighboring units or Stage 2. Do not use marker code, vacuous tests, broad exceptions, or unapproved dependency substitutions.
Run the stated validation responsibility. If blocked, stop and return exact evidence with REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="sem-orch-v1-runtime"></a>
### SEM-ORCH-V1-RUNTIME

Assignable: yes

Execution metadata:
- Wave: W3
- Context: fresh
- Depends on: SEM-CA-V12-FIDELITY
- Can run in parallel with: only disjoint AI/Agent residual peer where applicable
- Must not run in parallel with: any unit sharing an ownership prefix

Scope boundaries:
- Goal: Port all frozen Pi Orchestrator modules and create executable one-to-one tests: protocol, IPC client/server, socket paths, handler, RPC process, storage, supervisor, config, radius and serve lifecycle. No marker constants.
- Non-goals: neighboring units, Stage 2, promotion, unapproved dependencies.
- Forbidden work: placeholders, vacuous tests, broad exceptions, frozen Pi mutation, out-of-scope cleanup.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify/create as required | `crates/zedflow-orchestrator/src` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-orchestrator/tests` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-orchestrator/Cargo.toml` | exclusive controller ownership for this unit |
| modify/create as required | `Cargo.lock` | exclusive controller ownership for this unit |

Required context package:
- Plan references: this unit, Goal, Global Acceptance, Legacy policy, RF-S1 through RF-S6 as applicable.
- Required files: owned Rust paths and their one-to-one files/tests under `references/pi/packages/` at frozen SHA `2b00dade7cec918aefb025c8b7a4fa304a30acdd`.
- Dependency outputs: predecessor result, candidate SHA, declared validation logs.
- Neighboring out-of-scope units: all other IDs in this plan.

Implementation outline:
1. Read the complete frozen Pi counterpart and all Rust callers/exports before editing.
2. Implement observable behavior at the shared root cause; wire modules and real tests.
3. Run only declared/local checks; return a structured result with evidence and remaining blocker classification.

Validation responsibility:
- Type: locally-validating
- Must run: cargo fmt --package zedflow-orchestrator --check
- Must NOT run: workspace-wide gates or mutate files outside ownership.

Output contract:
- Structured PASS/BLOCKED result, exact base/candidate SHA, changed files or reviewed scope, commands/evidence, residual risks, and one valid blocker classification when not accepted.

Acceptance criteria:
- Port all frozen Pi Orchestrator modules and create executable one-to-one tests: protocol, IPC client/server, socket paths, handler, RPC process, storage, supervisor, config, radius and serve lifecycle. No marker constants.
- No forbidden workaround or out-of-scope edit; all declared commands pass.

Handoff to dependent units:
- Commit/result SHA and durable validation logs; name any intentionally deferred behavior and its owning downstream ID.

Subagent prompt:
```text
Implement only SEM-ORCH-V1-RUNTIME from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this entire unit, its listed frozen Pi counterparts, Rust callers/module roots, dependency outputs, and the global acceptance/legacy rules before editing.
Task: Port all frozen Pi Orchestrator modules and create executable one-to-one tests: protocol, IPC client/server, socket paths, handler, RPC process, storage, supervisor, config, radius and serve lifecycle. No marker constants.
Stay inside ownership. Do not implement neighboring units or Stage 2. Do not use marker code, vacuous tests, broad exceptions, or unapproved dependency substitutions.
Run the stated validation responsibility. If blocked, stop and return exact evidence with REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="sem-orch-v2-validate"></a>
### SEM-ORCH-V2-VALIDATE

Assignable: yes

Execution metadata:
- Wave: W3
- Context: fresh
- Depends on: SEM-ORCH-V1-RUNTIME
- Can run in parallel with: only disjoint AI/Agent residual peer where applicable
- Must not run in parallel with: any unit sharing an ownership prefix

Scope boundaries:
- Goal: Validate executable Orchestrator tests, strict manifest closure and package compilation.
- Non-goals: neighboring units, Stage 2, promotion, unapproved dependencies.
- Forbidden work: placeholders, vacuous tests, broad exceptions, frozen Pi mutation, out-of-scope cleanup.

Files:
| Action | Path | Purpose |
|---|---|---|
| read only | exact predecessor candidate | validation/review only |

Required context package:
- Plan references: this unit, Goal, Global Acceptance, Legacy policy, RF-S1 through RF-S6 as applicable.
- Required files: owned Rust paths and their one-to-one files/tests under `references/pi/packages/` at frozen SHA `2b00dade7cec918aefb025c8b7a4fa304a30acdd`.
- Dependency outputs: predecessor result, candidate SHA, declared validation logs.
- Neighboring out-of-scope units: all other IDs in this plan.

Implementation outline:
1. Read the complete frozen Pi counterpart and all Rust callers/exports before editing.
2. Implement observable behavior at the shared root cause; wire modules and real tests.
3. Run only declared/local checks; return a structured result with evidence and remaining blocker classification.

Validation responsibility:
- Type: integration-validating
- Must run: cargo fmt --package zedflow-orchestrator --check; cargo check -p zedflow-orchestrator --all-targets; cargo test -p zedflow-orchestrator --all-targets; python3 tools/pi-port-swarm/manifest.py check --package zedflow-orchestrator
- Must NOT run: source edits.

Output contract:
- Structured PASS/BLOCKED result, exact base/candidate SHA, changed files or reviewed scope, commands/evidence, residual risks, and one valid blocker classification when not accepted.

Acceptance criteria:
- Validate executable Orchestrator tests, strict manifest closure and package compilation.
- No forbidden workaround or out-of-scope edit; all declared commands pass.

Handoff to dependent units:
- Commit/result SHA and durable validation logs; name any intentionally deferred behavior and its owning downstream ID.

Subagent prompt:
```text
Implement only SEM-ORCH-V2-VALIDATE from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this entire unit, its listed frozen Pi counterparts, Rust callers/module roots, dependency outputs, and the global acceptance/legacy rules before editing.
Task: Validate executable Orchestrator tests, strict manifest closure and package compilation.
Stay inside ownership. Do not implement neighboring units or Stage 2. Do not use marker code, vacuous tests, broad exceptions, or unapproved dependency substitutions.
Run the stated validation responsibility. If blocked, stop and return exact evidence with REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="sem-orch-v3-fidelity"></a>
### SEM-ORCH-V3-FIDELITY

Assignable: yes

Execution metadata:
- Wave: W3
- Context: fresh
- Depends on: SEM-ORCH-V2-VALIDATE
- Can run in parallel with: only disjoint AI/Agent residual peer where applicable
- Must not run in parallel with: any unit sharing an ownership prefix

Scope boundaries:
- Goal: Independently compare every frozen Pi Orchestrator runtime module and protocol behavior against Rust; reject source-marker or compile-only closure.
- Non-goals: neighboring units, Stage 2, promotion, unapproved dependencies.
- Forbidden work: placeholders, vacuous tests, broad exceptions, frozen Pi mutation, out-of-scope cleanup.

Files:
| Action | Path | Purpose |
|---|---|---|
| read only | exact predecessor candidate | validation/review only |

Required context package:
- Plan references: this unit, Goal, Global Acceptance, Legacy policy, RF-S1 through RF-S6 as applicable.
- Required files: owned Rust paths and their one-to-one files/tests under `references/pi/packages/` at frozen SHA `2b00dade7cec918aefb025c8b7a4fa304a30acdd`.
- Dependency outputs: predecessor result, candidate SHA, declared validation logs.
- Neighboring out-of-scope units: all other IDs in this plan.

Implementation outline:
1. Read the complete frozen Pi counterpart and all Rust callers/exports before editing.
2. Implement observable behavior at the shared root cause; wire modules and real tests.
3. Run only declared/local checks; return a structured result with evidence and remaining blocker classification.

Validation responsibility:
- Type: integration-validating
- Must run: independent commands necessary to establish the stated review intent
- Must NOT run: source edits.

Output contract:
- Structured PASS/BLOCKED result, exact base/candidate SHA, changed files or reviewed scope, commands/evidence, residual risks, and one valid blocker classification when not accepted.

Acceptance criteria:
- Independently compare every frozen Pi Orchestrator runtime module and protocol behavior against Rust; reject source-marker or compile-only closure.
- No forbidden workaround or out-of-scope edit; all declared commands pass.

Handoff to dependent units:
- Commit/result SHA and durable validation logs; name any intentionally deferred behavior and its owning downstream ID.

Subagent prompt:
```text
Implement only SEM-ORCH-V3-FIDELITY from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this entire unit, its listed frozen Pi counterparts, Rust callers/module roots, dependency outputs, and the global acceptance/legacy rules before editing.
Task: Independently compare every frozen Pi Orchestrator runtime module and protocol behavior against Rust; reject source-marker or compile-only closure.
Stay inside ownership. Do not implement neighboring units or Stage 2. Do not use marker code, vacuous tests, broad exceptions, or unapproved dependency substitutions.
Run the stated validation responsibility. If blocked, stop and return exact evidence with REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="sem-ai-v1-residuals"></a>
### SEM-AI-V1-RESIDUALS

Assignable: yes

Execution metadata:
- Wave: W4
- Context: fresh
- Depends on: SEM-ORCH-V3-FIDELITY
- Can run in parallel with: only disjoint AI/Agent residual peer where applicable
- Must not run in parallel with: any unit sharing an ownership prefix

Scope boundaries:
- Goal: Remove Error::PortPlaceholder and transport no-op paths; make all deterministic formerly ignored AI tests executable for cancellation, provider handoff, reasoning replay, cache and model/error behavior. Keep live credential tests explicitly capability-gated with auditable reasons.
- Non-goals: neighboring units, Stage 2, promotion, unapproved dependencies.
- Forbidden work: placeholders, vacuous tests, broad exceptions, frozen Pi mutation, out-of-scope cleanup.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify/create as required | `crates/zedflow-ai/src/compat.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-ai/src/api/openai-codex-responses.rs` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-ai/tests` | exclusive controller ownership for this unit |

Required context package:
- Plan references: this unit, Goal, Global Acceptance, Legacy policy, RF-S1 through RF-S6 as applicable.
- Required files: owned Rust paths and their one-to-one files/tests under `references/pi/packages/` at frozen SHA `2b00dade7cec918aefb025c8b7a4fa304a30acdd`.
- Dependency outputs: predecessor result, candidate SHA, declared validation logs.
- Neighboring out-of-scope units: all other IDs in this plan.

Implementation outline:
1. Read the complete frozen Pi counterpart and all Rust callers/exports before editing.
2. Implement observable behavior at the shared root cause; wire modules and real tests.
3. Run only declared/local checks; return a structured result with evidence and remaining blocker classification.

Validation responsibility:
- Type: locally-validating
- Must run: cargo fmt --package zedflow-ai --check
- Must NOT run: workspace-wide gates or mutate files outside ownership.

Output contract:
- Structured PASS/BLOCKED result, exact base/candidate SHA, changed files or reviewed scope, commands/evidence, residual risks, and one valid blocker classification when not accepted.

Acceptance criteria:
- Remove Error::PortPlaceholder and transport no-op paths; make all deterministic formerly ignored AI tests executable for cancellation, provider handoff, reasoning replay, cache and model/error behavior. Keep live credential tests explicitly capability-gated with auditable reasons.
- No forbidden workaround or out-of-scope edit; all declared commands pass.

Handoff to dependent units:
- Commit/result SHA and durable validation logs; name any intentionally deferred behavior and its owning downstream ID.

Subagent prompt:
```text
Implement only SEM-AI-V1-RESIDUALS from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this entire unit, its listed frozen Pi counterparts, Rust callers/module roots, dependency outputs, and the global acceptance/legacy rules before editing.
Task: Remove Error::PortPlaceholder and transport no-op paths; make all deterministic formerly ignored AI tests executable for cancellation, provider handoff, reasoning replay, cache and model/error behavior. Keep live credential tests explicitly capability-gated with auditable reasons.
Stay inside ownership. Do not implement neighboring units or Stage 2. Do not use marker code, vacuous tests, broad exceptions, or unapproved dependency substitutions.
Run the stated validation responsibility. If blocked, stop and return exact evidence with REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="sem-ag-v1-residuals"></a>
### SEM-AG-V1-RESIDUALS

Assignable: yes

Execution metadata:
- Wave: W4
- Context: fresh
- Depends on: SEM-ORCH-V3-FIDELITY
- Can run in parallel with: only disjoint AI/Agent residual peer where applicable
- Must not run in parallel with: any unit sharing an ownership prefix

Scope boundaries:
- Goal: Close the four ignored Agent behaviors with deterministic executable tests and any root-cause implementation fix: cancellation/e2e, UTF-16 truncation, compaction and persistence.
- Non-goals: neighboring units, Stage 2, promotion, unapproved dependencies.
- Forbidden work: placeholders, vacuous tests, broad exceptions, frozen Pi mutation, out-of-scope cleanup.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify/create as required | `crates/zedflow-agent/src` | exclusive controller ownership for this unit |
| modify/create as required | `crates/zedflow-agent/tests` | exclusive controller ownership for this unit |

Required context package:
- Plan references: this unit, Goal, Global Acceptance, Legacy policy, RF-S1 through RF-S6 as applicable.
- Required files: owned Rust paths and their one-to-one files/tests under `references/pi/packages/` at frozen SHA `2b00dade7cec918aefb025c8b7a4fa304a30acdd`.
- Dependency outputs: predecessor result, candidate SHA, declared validation logs.
- Neighboring out-of-scope units: all other IDs in this plan.

Implementation outline:
1. Read the complete frozen Pi counterpart and all Rust callers/exports before editing.
2. Implement observable behavior at the shared root cause; wire modules and real tests.
3. Run only declared/local checks; return a structured result with evidence and remaining blocker classification.

Validation responsibility:
- Type: locally-validating
- Must run: cargo fmt --package zedflow-agent --check
- Must NOT run: workspace-wide gates or mutate files outside ownership.

Output contract:
- Structured PASS/BLOCKED result, exact base/candidate SHA, changed files or reviewed scope, commands/evidence, residual risks, and one valid blocker classification when not accepted.

Acceptance criteria:
- Close the four ignored Agent behaviors with deterministic executable tests and any root-cause implementation fix: cancellation/e2e, UTF-16 truncation, compaction and persistence.
- No forbidden workaround or out-of-scope edit; all declared commands pass.

Handoff to dependent units:
- Commit/result SHA and durable validation logs; name any intentionally deferred behavior and its owning downstream ID.

Subagent prompt:
```text
Implement only SEM-AG-V1-RESIDUALS from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this entire unit, its listed frozen Pi counterparts, Rust callers/module roots, dependency outputs, and the global acceptance/legacy rules before editing.
Task: Close the four ignored Agent behaviors with deterministic executable tests and any root-cause implementation fix: cancellation/e2e, UTF-16 truncation, compaction and persistence.
Stay inside ownership. Do not implement neighboring units or Stage 2. Do not use marker code, vacuous tests, broad exceptions, or unapproved dependency substitutions.
Run the stated validation responsibility. If blocked, stop and return exact evidence with REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="sem-residuals-v2-validate"></a>
### SEM-RESIDUALS-V2-VALIDATE

Assignable: yes

Execution metadata:
- Wave: W4
- Context: fresh
- Depends on: SEM-AI-V1-RESIDUALS, SEM-AG-V1-RESIDUALS
- Can run in parallel with: only disjoint AI/Agent residual peer where applicable
- Must not run in parallel with: any unit sharing an ownership prefix

Scope boundaries:
- Goal: Validate AI and Agent deterministic tests, transport/streaming/cancellation behavior and strict semantic closure.
- Non-goals: neighboring units, Stage 2, promotion, unapproved dependencies.
- Forbidden work: placeholders, vacuous tests, broad exceptions, frozen Pi mutation, out-of-scope cleanup.

Files:
| Action | Path | Purpose |
|---|---|---|
| read only | exact predecessor candidate | validation/review only |

Required context package:
- Plan references: this unit, Goal, Global Acceptance, Legacy policy, RF-S1 through RF-S6 as applicable.
- Required files: owned Rust paths and their one-to-one files/tests under `references/pi/packages/` at frozen SHA `2b00dade7cec918aefb025c8b7a4fa304a30acdd`.
- Dependency outputs: predecessor result, candidate SHA, declared validation logs.
- Neighboring out-of-scope units: all other IDs in this plan.

Implementation outline:
1. Read the complete frozen Pi counterpart and all Rust callers/exports before editing.
2. Implement observable behavior at the shared root cause; wire modules and real tests.
3. Run only declared/local checks; return a structured result with evidence and remaining blocker classification.

Validation responsibility:
- Type: integration-validating
- Must run: cargo check -p zedflow-ai --all-targets; cargo test -p zedflow-ai --all-targets; python3 tools/pi-port-swarm/manifest.py check --package zedflow-ai; cargo check -p zedflow-agent --all-targets; cargo test -p zedflow-agent --all-targets; python3 tools/pi-port-swarm/manifest.py check --package zedflow-agent
- Must NOT run: source edits.

Output contract:
- Structured PASS/BLOCKED result, exact base/candidate SHA, changed files or reviewed scope, commands/evidence, residual risks, and one valid blocker classification when not accepted.

Acceptance criteria:
- Validate AI and Agent deterministic tests, transport/streaming/cancellation behavior and strict semantic closure.
- No forbidden workaround or out-of-scope edit; all declared commands pass.

Handoff to dependent units:
- Commit/result SHA and durable validation logs; name any intentionally deferred behavior and its owning downstream ID.

Subagent prompt:
```text
Implement only SEM-RESIDUALS-V2-VALIDATE from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this entire unit, its listed frozen Pi counterparts, Rust callers/module roots, dependency outputs, and the global acceptance/legacy rules before editing.
Task: Validate AI and Agent deterministic tests, transport/streaming/cancellation behavior and strict semantic closure.
Stay inside ownership. Do not implement neighboring units or Stage 2. Do not use marker code, vacuous tests, broad exceptions, or unapproved dependency substitutions.
Run the stated validation responsibility. If blocked, stop and return exact evidence with REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="sem-final-v1-workspace"></a>
### SEM-FINAL-V1-WORKSPACE

Assignable: yes

Execution metadata:
- Wave: W5
- Context: fresh
- Depends on: SEM-RESIDUALS-V2-VALIDATE
- Can run in parallel with: only disjoint AI/Agent residual peer where applicable
- Must not run in parallel with: any unit sharing an ownership prefix

Scope boundaries:
- Goal: Run the full Stage-1 workspace gates on one immutable candidate with no unexplained ignore, marker-only target, dead mapped module, placeholder/no-op or unwired CLI mode.
- Non-goals: neighboring units, Stage 2, promotion, unapproved dependencies.
- Forbidden work: placeholders, vacuous tests, broad exceptions, frozen Pi mutation, out-of-scope cleanup.

Files:
| Action | Path | Purpose |
|---|---|---|
| read only | exact predecessor candidate | validation/review only |

Required context package:
- Plan references: this unit, Goal, Global Acceptance, Legacy policy, RF-S1 through RF-S6 as applicable.
- Required files: owned Rust paths and their one-to-one files/tests under `references/pi/packages/` at frozen SHA `2b00dade7cec918aefb025c8b7a4fa304a30acdd`.
- Dependency outputs: predecessor result, candidate SHA, declared validation logs.
- Neighboring out-of-scope units: all other IDs in this plan.

Implementation outline:
1. Read the complete frozen Pi counterpart and all Rust callers/exports before editing.
2. Implement observable behavior at the shared root cause; wire modules and real tests.
3. Run only declared/local checks; return a structured result with evidence and remaining blocker classification.

Validation responsibility:
- Type: integration-validating
- Must run: cargo fmt --all --check; cargo check --workspace --all-targets; cargo test --workspace --all-targets; python3 tools/pi-port-swarm/manifest.py check
- Must NOT run: source edits.

Output contract:
- Structured PASS/BLOCKED result, exact base/candidate SHA, changed files or reviewed scope, commands/evidence, residual risks, and one valid blocker classification when not accepted.

Acceptance criteria:
- Run the full Stage-1 workspace gates on one immutable candidate with no unexplained ignore, marker-only target, dead mapped module, placeholder/no-op or unwired CLI mode.
- No forbidden workaround or out-of-scope edit; all declared commands pass.

Handoff to dependent units:
- Commit/result SHA and durable validation logs; name any intentionally deferred behavior and its owning downstream ID.

Subagent prompt:
```text
Implement only SEM-FINAL-V1-WORKSPACE from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this entire unit, its listed frozen Pi counterparts, Rust callers/module roots, dependency outputs, and the global acceptance/legacy rules before editing.
Task: Run the full Stage-1 workspace gates on one immutable candidate with no unexplained ignore, marker-only target, dead mapped module, placeholder/no-op or unwired CLI mode.
Stay inside ownership. Do not implement neighboring units or Stage 2. Do not use marker code, vacuous tests, broad exceptions, or unapproved dependency substitutions.
Run the stated validation responsibility. If blocked, stop and return exact evidence with REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="sem-final-v2-fidelity"></a>
### SEM-FINAL-V2-FIDELITY

Assignable: yes

Execution metadata:
- Wave: W5
- Context: fresh
- Depends on: SEM-FINAL-V1-WORKSPACE
- Can run in parallel with: only disjoint AI/Agent residual peer where applicable
- Must not run in parallel with: any unit sharing an ownership prefix

Scope boundaries:
- Goal: Perform independent package-by-package Pi semantic fidelity review on the exact candidate SHA, including differential fixtures and the frozen TypeScript oracle.
- Non-goals: neighboring units, Stage 2, promotion, unapproved dependencies.
- Forbidden work: placeholders, vacuous tests, broad exceptions, frozen Pi mutation, out-of-scope cleanup.

Files:
| Action | Path | Purpose |
|---|---|---|
| read only | exact predecessor candidate | validation/review only |

Required context package:
- Plan references: this unit, Goal, Global Acceptance, Legacy policy, RF-S1 through RF-S6 as applicable.
- Required files: owned Rust paths and their one-to-one files/tests under `references/pi/packages/` at frozen SHA `2b00dade7cec918aefb025c8b7a4fa304a30acdd`.
- Dependency outputs: predecessor result, candidate SHA, declared validation logs.
- Neighboring out-of-scope units: all other IDs in this plan.

Implementation outline:
1. Read the complete frozen Pi counterpart and all Rust callers/exports before editing.
2. Implement observable behavior at the shared root cause; wire modules and real tests.
3. Run only declared/local checks; return a structured result with evidence and remaining blocker classification.

Validation responsibility:
- Type: integration-validating
- Must run: independent commands necessary to establish the stated review intent
- Must NOT run: source edits.

Output contract:
- Structured PASS/BLOCKED result, exact base/candidate SHA, changed files or reviewed scope, commands/evidence, residual risks, and one valid blocker classification when not accepted.

Acceptance criteria:
- Perform independent package-by-package Pi semantic fidelity review on the exact candidate SHA, including differential fixtures and the frozen TypeScript oracle.
- No forbidden workaround or out-of-scope edit; all declared commands pass.

Handoff to dependent units:
- Commit/result SHA and durable validation logs; name any intentionally deferred behavior and its owning downstream ID.

Subagent prompt:
```text
Implement only SEM-FINAL-V2-FIDELITY from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this entire unit, its listed frozen Pi counterparts, Rust callers/module roots, dependency outputs, and the global acceptance/legacy rules before editing.
Task: Perform independent package-by-package Pi semantic fidelity review on the exact candidate SHA, including differential fixtures and the frozen TypeScript oracle.
Stay inside ownership. Do not implement neighboring units or Stage 2. Do not use marker code, vacuous tests, broad exceptions, or unapproved dependency substitutions.
Run the stated validation responsibility. If blocked, stop and return exact evidence with REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="sem-final-v3-rust-quality"></a>
### SEM-FINAL-V3-RUST-QUALITY

Assignable: yes

Execution metadata:
- Wave: W5
- Context: fresh
- Depends on: SEM-FINAL-V2-FIDELITY
- Can run in parallel with: only disjoint AI/Agent residual peer where applicable
- Must not run in parallel with: any unit sharing an ownership prefix

Scope boundaries:
- Goal: Perform independent Rust safety, correctness, error-handling, cancellation, portability and maintainability review on the same exact candidate SHA.
- Non-goals: neighboring units, Stage 2, promotion, unapproved dependencies.
- Forbidden work: placeholders, vacuous tests, broad exceptions, frozen Pi mutation, out-of-scope cleanup.

Files:
| Action | Path | Purpose |
|---|---|---|
| read only | exact predecessor candidate | validation/review only |

Required context package:
- Plan references: this unit, Goal, Global Acceptance, Legacy policy, RF-S1 through RF-S6 as applicable.
- Required files: owned Rust paths and their one-to-one files/tests under `references/pi/packages/` at frozen SHA `2b00dade7cec918aefb025c8b7a4fa304a30acdd`.
- Dependency outputs: predecessor result, candidate SHA, declared validation logs.
- Neighboring out-of-scope units: all other IDs in this plan.

Implementation outline:
1. Read the complete frozen Pi counterpart and all Rust callers/exports before editing.
2. Implement observable behavior at the shared root cause; wire modules and real tests.
3. Run only declared/local checks; return a structured result with evidence and remaining blocker classification.

Validation responsibility:
- Type: integration-validating
- Must run: independent commands necessary to establish the stated review intent
- Must NOT run: source edits.

Output contract:
- Structured PASS/BLOCKED result, exact base/candidate SHA, changed files or reviewed scope, commands/evidence, residual risks, and one valid blocker classification when not accepted.

Acceptance criteria:
- Perform independent Rust safety, correctness, error-handling, cancellation, portability and maintainability review on the same exact candidate SHA.
- No forbidden workaround or out-of-scope edit; all declared commands pass.

Handoff to dependent units:
- Commit/result SHA and durable validation logs; name any intentionally deferred behavior and its owning downstream ID.

Subagent prompt:
```text
Implement only SEM-FINAL-V3-RUST-QUALITY from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this entire unit, its listed frozen Pi counterparts, Rust callers/module roots, dependency outputs, and the global acceptance/legacy rules before editing.
Task: Perform independent Rust safety, correctness, error-handling, cancellation, portability and maintainability review on the same exact candidate SHA.
Stay inside ownership. Do not implement neighboring units or Stage 2. Do not use marker code, vacuous tests, broad exceptions, or unapproved dependency substitutions.
Run the stated validation responsibility. If blocked, stop and return exact evidence with REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="sem-final-v4-enduser"></a>
### SEM-FINAL-V4-ENDUSER

Assignable: yes

Execution metadata:
- Wave: W5
- Context: fresh
- Depends on: SEM-FINAL-V3-RUST-QUALITY
- Can run in parallel with: only disjoint AI/Agent residual peer where applicable
- Must not run in parallel with: any unit sharing an ownership prefix

Scope boundaries:
- Goal: Run end-user black-box gates on the same SHA: default full-screen TUI, --print/text/json, RPC, sessions/no-session, tools, extensions, skills, themes, package management and Orchestrator; verify terminal restoration on normal and failure paths.
- Non-goals: neighboring units, Stage 2, promotion, unapproved dependencies.
- Forbidden work: placeholders, vacuous tests, broad exceptions, frozen Pi mutation, out-of-scope cleanup.

Files:
| Action | Path | Purpose |
|---|---|---|
| read only | exact predecessor candidate | validation/review only |

Required context package:
- Plan references: this unit, Goal, Global Acceptance, Legacy policy, RF-S1 through RF-S6 as applicable.
- Required files: owned Rust paths and their one-to-one files/tests under `references/pi/packages/` at frozen SHA `2b00dade7cec918aefb025c8b7a4fa304a30acdd`.
- Dependency outputs: predecessor result, candidate SHA, declared validation logs.
- Neighboring out-of-scope units: all other IDs in this plan.

Implementation outline:
1. Read the complete frozen Pi counterpart and all Rust callers/exports before editing.
2. Implement observable behavior at the shared root cause; wire modules and real tests.
3. Run only declared/local checks; return a structured result with evidence and remaining blocker classification.

Validation responsibility:
- Type: integration-validating
- Must run: independent commands necessary to establish the stated review intent
- Must NOT run: source edits.

Output contract:
- Structured PASS/BLOCKED result, exact base/candidate SHA, changed files or reviewed scope, commands/evidence, residual risks, and one valid blocker classification when not accepted.

Acceptance criteria:
- Run end-user black-box gates on the same SHA: default full-screen TUI, --print/text/json, RPC, sessions/no-session, tools, extensions, skills, themes, package management and Orchestrator; verify terminal restoration on normal and failure paths.
- No forbidden workaround or out-of-scope edit; all declared commands pass.

Handoff to dependent units:
- Commit/result SHA and durable validation logs; name any intentionally deferred behavior and its owning downstream ID.

Subagent prompt:
```text
Implement only SEM-FINAL-V4-ENDUSER from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this entire unit, its listed frozen Pi counterparts, Rust callers/module roots, dependency outputs, and the global acceptance/legacy rules before editing.
Task: Run end-user black-box gates on the same SHA: default full-screen TUI, --print/text/json, RPC, sessions/no-session, tools, extensions, skills, themes, package management and Orchestrator; verify terminal restoration on normal and failure paths.
Stay inside ownership. Do not implement neighboring units or Stage 2. Do not use marker code, vacuous tests, broad exceptions, or unapproved dependency substitutions.
Run the stated validation responsibility. If blocked, stop and return exact evidence with REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="sem-final-v5-docs"></a>
### SEM-FINAL-V5-DOCS

Assignable: yes

Execution metadata:
- Wave: W5
- Context: fresh
- Depends on: SEM-FINAL-V4-ENDUSER
- Can run in parallel with: only disjoint AI/Agent residual peer where applicable
- Must not run in parallel with: any unit sharing an ownership prefix

Scope boundaries:
- Goal: Record the exact accepted integration SHA and evidence from all Stage-1 gates. Do not claim completion or authorize Stage 2 unless every predecessor accepted the same SHA.
- Non-goals: neighboring units, Stage 2, promotion, unapproved dependencies.
- Forbidden work: placeholders, vacuous tests, broad exceptions, frozen Pi mutation, out-of-scope cleanup.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify/create as required | `docs/porting/BASELINE.md` | exclusive controller ownership for this unit |
| modify/create as required | `.agents/port-swarm/decisions.md` | exclusive controller ownership for this unit |
| modify/create as required | `.agents/state` | exclusive controller ownership for this unit |

Required context package:
- Plan references: this unit, Goal, Global Acceptance, Legacy policy, RF-S1 through RF-S6 as applicable.
- Required files: owned Rust paths and their one-to-one files/tests under `references/pi/packages/` at frozen SHA `2b00dade7cec918aefb025c8b7a4fa304a30acdd`.
- Dependency outputs: predecessor result, candidate SHA, declared validation logs.
- Neighboring out-of-scope units: all other IDs in this plan.

Implementation outline:
1. Read the complete frozen Pi counterpart and all Rust callers/exports before editing.
2. Implement observable behavior at the shared root cause; wire modules and real tests.
3. Run only declared/local checks; return a structured result with evidence and remaining blocker classification.

Validation responsibility:
- Type: locally-validating
- Must run: git diff --check
- Must NOT run: workspace-wide gates or mutate files outside ownership.

Output contract:
- Structured PASS/BLOCKED result, exact base/candidate SHA, changed files or reviewed scope, commands/evidence, residual risks, and one valid blocker classification when not accepted.

Acceptance criteria:
- Record the exact accepted integration SHA and evidence from all Stage-1 gates. Do not claim completion or authorize Stage 2 unless every predecessor accepted the same SHA.
- No forbidden workaround or out-of-scope edit; all declared commands pass.

Handoff to dependent units:
- Commit/result SHA and durable validation logs; name any intentionally deferred behavior and its owning downstream ID.

Subagent prompt:
```text
Implement only SEM-FINAL-V5-DOCS from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this entire unit, its listed frozen Pi counterparts, Rust callers/module roots, dependency outputs, and the global acceptance/legacy rules before editing.
Task: Record the exact accepted integration SHA and evidence from all Stage-1 gates. Do not claim completion or authorize Stage 2 unless every predecessor accepted the same SHA.
Stay inside ownership. Do not implement neighboring units or Stage 2. Do not use marker code, vacuous tests, broad exceptions, or unapproved dependency substitutions.
Run the stated validation responsibility. If blocked, stop and return exact evidence with REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="sem-final-v6-checkpoint"></a>
### SEM-FINAL-V6-CHECKPOINT

Assignable: yes

Execution metadata:
- Wave: W5
- Context: fresh
- Depends on: SEM-FINAL-V5-DOCS
- Can run in parallel with: only disjoint AI/Agent residual peer where applicable
- Must not run in parallel with: any unit sharing an ownership prefix

Scope boundaries:
- Goal: Record the terminal Stage-1 semantic checkpoint. Promotion to main remains a separate explicit human action; Stage 2 remains forbidden until promotion and repeated gates.
- Non-goals: neighboring units, Stage 2, promotion, unapproved dependencies.
- Forbidden work: placeholders, vacuous tests, broad exceptions, frozen Pi mutation, out-of-scope cleanup.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify/create as required | `docs/porting/BASELINE.md` | exclusive controller ownership for this unit |
| modify/create as required | `.agents/port-swarm/decisions.md` | exclusive controller ownership for this unit |

Required context package:
- Plan references: this unit, Goal, Global Acceptance, Legacy policy, RF-S1 through RF-S6 as applicable.
- Required files: owned Rust paths and their one-to-one files/tests under `references/pi/packages/` at frozen SHA `2b00dade7cec918aefb025c8b7a4fa304a30acdd`.
- Dependency outputs: predecessor result, candidate SHA, declared validation logs.
- Neighboring out-of-scope units: all other IDs in this plan.

Implementation outline:
1. Read the complete frozen Pi counterpart and all Rust callers/exports before editing.
2. Implement observable behavior at the shared root cause; wire modules and real tests.
3. Run only declared/local checks; return a structured result with evidence and remaining blocker classification.

Validation responsibility:
- Type: integration-validating
- Must run: git diff --check
- Must NOT run: source edits.

Output contract:
- Structured PASS/BLOCKED result, exact base/candidate SHA, changed files or reviewed scope, commands/evidence, residual risks, and one valid blocker classification when not accepted.

Acceptance criteria:
- Record the terminal Stage-1 semantic checkpoint. Promotion to main remains a separate explicit human action; Stage 2 remains forbidden until promotion and repeated gates.
- No forbidden workaround or out-of-scope edit; all declared commands pass.

Handoff to dependent units:
- Commit/result SHA and durable validation logs; name any intentionally deferred behavior and its owning downstream ID.

Subagent prompt:
```text
Implement only SEM-FINAL-V6-CHECKPOINT from .agents/plans/pi-stage-1-port-recovery.md in fresh context.
Read this entire unit, its listed frozen Pi counterparts, Rust callers/module roots, dependency outputs, and the global acceptance/legacy rules before editing.
Task: Record the terminal Stage-1 semantic checkpoint. Promotion to main remains a separate explicit human action; Stage 2 remains forbidden until promotion and repeated gates.
Stay inside ownership. Do not implement neighboring units or Stage 2. Do not use marker code, vacuous tests, broad exceptions, or unapproved dependency substitutions.
Run the stated validation responsibility. If blocked, stop and return exact evidence with REPAIRABLE, PLAN_CHANGE_REQUIRED, ARBITRATION_REQUIRED, or TRANSIENT.
```

<a id="pre-finalization-review"></a>
## Pre-finalization Review Summary

| Reviewer | Status | Required changes applied | Remaining concerns |
|---|---|---|---|
| Feasibility / file references | reviewed | TUI/native blockers resolved; extension surface split into API, ABI/loader, SDK fixture, source installer, resources and gates | native cdylibs remain trusted code and v1 intentionally never unloads |
| Sequencing / dependency graph | reviewed | strict guard first; TUI→Coding-agent; V3A→V3G before interactive V4; exact-SHA gates | one writer makes execution intentionally serial |
| Scope isolation / prompt quality | reviewed | fresh IDs, exact ownership, full ABI safety contract and executable fixture gate | large later Coding-agent test corpus remains five bounded batches |
