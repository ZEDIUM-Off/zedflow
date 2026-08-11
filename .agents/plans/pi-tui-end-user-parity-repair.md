# Pi TUI End-User Parity Repair

<a id="how-to-use"></a>
## How to use this plan

This plan is self-contained for orchestration by a fresh agent session.

- All implementation subagents must run in fresh context.
- Execute only assignable unit IDs listed in the orchestration waves.
- Before launching a unit, pass its full `Subagent prompt` plus the relevant plan references from `Canonical Line References`.
- Do not infer requirements from outside this plan and the listed references.
- Do not execute neighboring task scopes.
- If a unit is marked `non-validating`, do not run global validation or add compatibility workarounds to make the repo compile.
- Only units marked `integration-validating` own global validation gates.
- If blocked by missing context or unexpected codebase reality, report the blocker and stop instead of inventing a workaround.
- Execution baseline is `refs/heads/automation/pi-port@6c07885e721e9f000cb7abe37d59e84dae00d68b`; product code is `a9a23c387f372ed027c5a742047f93d0689955ed`; frozen Pi is `references/pi@2b00dade7cec918aefb025c8b7a4fa304a30acdd`.
- Before W1, the orchestrator must create a reviewed control candidate descending from the execution baseline, add this plan and fresh non-reused DAG unit IDs to `tools/pi-port-swarm/dag.json`, preserve the frozen gitlink, run the controller/control tests, and adopt it with `controller.py upgrade`. This control adoption is not an assignable implementation unit.

<a id="legend"></a>
## Legend

### Execution

- `fresh`: subagent receives only the prompt, this plan, and listed references.
- `sequential`: must run after listed dependencies.
- `parallel`: may run in the same orchestration wave.
- `Assignable: yes`: this unit may be launched as a subagent task.
- `Assignable: no`: this unit is a container or explanatory grouping only.

### Validation responsibility

- `non-validating`: do not run global compile/lint/test gates; only perform explicitly listed local checks.
- `locally-validating`: validate only the owned scope.
- `integration-validating`: owns broader validation gates.

### Artifact tags

- `[CANONICAL]`: required shape; preserve unless implementation evidence contradicts it.
- `[ILLUSTRATIVE]`: explains intent; adapt to local architecture.

### Review flag types

- `BQ`: blocking question requiring human decision before affected work.
- `OQ`: non-blocking open question.
- `C`: contradiction or ambiguity.
- `R`: implementation risk / watch item.

### File actions

- `create`: file must be created.
- `modify`: file must be edited.
- `delete`: file must be removed.
- `read`: file/reference must be read before editing.

<a id="goal"></a>
## Goal

Restore the Stage-1 Rust default interactive experience to observable Pi fidelity: complete terminal input and editor behavior, stateful transcript and incremental streaming, tool and compaction lifecycle rendering, themes/chrome/overlays/selectors, every built-in slash command, deterministic Pi-versus-Rust terminal comparison, and truthful one-SHA end-user acceptance. The live command `cargo run -p zedflow-coding-agent` must no longer render only `working`, `received`, and `idle`, must not route `/login` as a model prompt, and must use the ported editor rather than the bespoke append-only `InteractiveInput`.

<a id="non-goals"></a>
## Non-goals

- No multithreaded renderer, renderer thread, Ratatui migration, alternate-screen redesign, speculative caching, or new dependency before fidelity is accepted and profiling demonstrates a bottleneck.
- No Stage-2 Flow/Runtime Graph/LangGraph behavior.
- No TypeScript/jiti extension compatibility beyond the already approved Rust `cdylib` architecture.
- No credentialed provider calls in deterministic gates; provider behavior uses existing fake/capture fixtures.
- No release packaging or promotion to `main` without the explicit human checkpoint in RF-BQ1.
- No broad AI, Agent, Orchestrator, or non-interactive coding-agent refactor.

<a id="review-flags"></a>
## Review Flags

| ID | Type | Severity | Summary | Affects | Follow-up / accepted risk |
|---|---|---:|---|---|---|
| RF-C1 | C | blocking | The completed semantic DAG and end-user review accepted a runtime that bypasses the editor, drops streaming payloads, and treats built-ins as prompts. | Control adoption, P1.T1, P8.T1, P9.T1 | Reopen the completed DAG and replace the prior TUI/end-user acceptance evidence. |
| RF-R1 | R | high | PTY bytes and terminal capabilities vary by platform. | P2.T3, P7.T1, P8.T1 | Normalize dimensions/capabilities; require deterministic virtual-terminal parity everywhere and PTY restoration on supported Unix CI. |
| RF-R2 | R | high | Selector ports may expose missing Rust boundary methods for auth/settings/session/trust/package services. | P1.T2, P4.T2-P4.T4 | P1.T2 exclusively owns the fixed adapter scope; downstream units stop rather than edit core boundaries. |
| RF-R3 | R | medium | The frozen Pi oracle requires tracked npm dependencies, not local untracked pnpm state. | P2.T3, P7.T1 | Use frozen `references/pi/package-lock.json` and `npm ci`; fail clearly if Node/npm is unavailable. |
| RF-R4 | R | medium | Existing same-name tests may be structural stand-ins and cannot prove parity. | All implementation units | Port behavior-level assertions and require differential fixtures; source-path markers and filename-only evidence are forbidden. |
| RF-BQ1 | BQ | blocking after P9.T1 | Promotion to `main` is an explicit human action. | Post-plan promotion and post-promotion gates | Stop after evidence recording and request approval; then promote and rerun every gate on `main`. |
| RF-R5 | R | accepted | Renderer concurrency is deferred. | Entire plan | Retain one owner thread for state/composition/diff/output; background model/tool work continues through events. Profile only after parity. |

<a id="global-acceptance"></a>
## Global Acceptance Criteria

1. `cargo run -p zedflow-coding-agent` in a PTY renders actual user and assistant content, incremental assistant updates, tool states, status, editor, and footer; literal lifecycle placeholders are absent.
2. The live composition uses the Pi-compatible editor with cursor movement, grapheme-safe deletion, kill/yank/undo, multiline editing, history, autocomplete, paste handling, and application keybindings.
3. `MessageStart`, every `MessageUpdate`, `MessageEnd`, tool start/update/end, abort/error, queue, compaction, and session lifecycle update one stateful transcript without duplicate or lost content.
4. `/settings`, `/model`, `/scoped-models`, `/export`, `/import`, `/share`, `/copy`, `/name`, `/session`, `/changelog`, `/hotkeys`, `/fork`, `/clone`, `/tree`, `/trust`, `/login`, `/logout`, `/new`, `/compact`, `/resume`, `/reload`, `/quit`, and `/exit` are intercepted and behavior-tested; none reaches `session.prompt()`.
5. Every frozen Pi module under `packages/tui/src` and `packages/coding-agent/src/modes/interactive` has a reachable semantic Rust implementation or an explicit approved disposition; `source_path()`-only modules do not count.
6. Themes and assets (`dark.json`, `light.json`, `theme-schema.json`, `clankolas.png`) are available through the Rust runtime without filesystem assumptions that fail in a built binary.
7. Deterministic Pi and Rust fixture runs produce equal normalized terminal frames for input editing, streaming, tools, commands, overlays, Unicode/CJK/emoji, resize, abort/error, and compaction.
8. Terminal startup, suspend/resume, `/exit`, Ctrl-C, and panic/error paths restore cursor, bracketed paste, keyboard protocol, and termios state.
9. `cargo fmt --all --check`, `cargo check --workspace --all-targets`, `cargo test --workspace --all-targets`, strict manifest checks, the frozen differential oracle, independent Pi-fidelity review, Rust-quality review, and end-user PTY review accept one immutable candidate SHA.
10. Documentation records the accepted product SHA without claiming that the later documentation commit itself was gate-tested.

<a id="legacy-policy"></a>
## Legacy / workaround policy

- No temporary aliases unless explicitly listed as allowed in a task.
- No compatibility shims unless they are the goal of a task.
- No type weakening to satisfy intermediate compilation.
- No preserving legacy names when the planned change is a removal or rename.
- Do not expand a task scope to fix breakages assigned to later units.
- If blocked, report the breakage and reference the downstream unit responsible.
- Do not retain `EventLog`, the private append-only `InteractiveInput`, literal `received` labels, or marker-only `source_path()` modules as fallback paths.
- Do not weaken, skip, snapshot-update, or replace frozen Pi assertions merely to make parity gates pass.

<a id="breaking-changes"></a>
## Planned Breaking Changes and Propagation Map

| Change | Introduced by | Expected temporary breakage | Fixed by | Forbidden workaround |
|---|---|---|---|---|
| Expand TUI key/input/terminal contracts to frozen Pi behavior. | P1.T3 | Editor and interactive callers may require new callbacks/state. | P2.T2 and P5.T1 | Keeping the primitive input API as a second live path. |
| Replace marker selector/theme/component modules with semantic types. | P3.T1, P4.T1-P4.T5 | Flat exports and constructors may stop compiling. | P5.T1 | Re-exporting `source_path()` markers or adding empty constructors. |
| Replace event-string transcript with stateful message/tool entries. | P5.T1 | Existing tests that assert `working/received/idle` become invalid. | P5.T1 and P7.T1 | Preserving placeholder strings behind a feature flag. |
| Intercept built-in commands before prompting. | P6.T1 | Tests expecting slash text in model context must change. | P6.T1 | Sending unknown built-ins to the model or silently ignoring them. |
| Revoke prior TUI/end-user completion evidence. | P1.T1 | Stage 1 is explicitly incomplete again. | P8.T1 and P9.T1 | Retaining contradictory completion language. |

<a id="orchestration"></a>
## Subagent Orchestration Plan

Control adoption precondition: a fresh coordinator creates a clean candidate descending from `automation/pi-port@6c07885e`, commits this plan and fresh DAG IDs, preserves the Pi gitlink, runs `python3 tools/pi-port-swarm/test_controller.py`, `python3 tools/pi-port-swarm/test_manifest.py`, `python3 tools/pi-port-swarm/controller.py validate`, then performs the reviewed completed-DAG `controller.py upgrade`.

- **W1 — foundations in parallel:** P1.T1, P1.T2, P1.T3.
- **W2 — TUI surfaces in parallel after P1.T3:** P2.T1, P2.T2, P2.T3.
- **W3 — theme/chrome:** P3.T1 after P1.T2, P2.T1, and P2.T2.
- **W4 — component families in parallel:** P4.T1, P4.T2, P4.T3, P4.T4, P4.T5 after their listed dependencies.
- **W5 — root composition:** P5.T1 only.
- **W6 — complete built-in command routing:** P6.T1 only.
- **W7 — differential and PTY acceptance implementation:** P7.T1 only.
- **W8 — immutable candidate validation/reviews:** P8.T1 only.
- **W9 — evidence correction:** P9.T1 only.

<a id="parallelization-constraints"></a>
## Parallelization Constraints

| Constraint | Reason | Affected units |
|---|---|---|
| One writer per worktree and managed isolated worktree per parallel unit. | Controller CAS and same-repo safety. | All assignable units |
| Input and terminal foundations are one atomic unit. | Frozen `tui.ts`, `terminal.ts`, keys, utils, and input form a dependency cycle. | P1.T3 |
| Renderer-rich and editor units start only after foundation acceptance. | They consume the expanded terminal/input contracts. | P2.T1, P2.T2 |
| Theme/chrome follows renderer, editor, and core boundary adapters. | Theme controller consumes settings; chrome consumes loader/editor primitives. | P3.T1 |
| W4 component families have disjoint writer files and may run in parallel. | They share read-only core/TUI contracts only. | P4.T1-P4.T5 |
| Root composition waits for every visible component family. | It is the only unit allowed to assemble the live tree and rewrite module exports. | P5.T1 |
| Built-in routing follows composition and shares `interactive-mode.rs`. | Same-file write/write must be sequential. | P5.T1, P6.T1 |
| Oracle acceptance follows command routing. | Snapshots must describe final behavior, not intermediate structure. | P7.T1 |
| Docs/evidence follow read-only gate acceptance. | Evidence must name the tested product SHA. | P8.T1, P9.T1 |

<a id="canonical-line-references"></a>
## Canonical Line References

<!-- CANONICAL_LINE_REFERENCES_START -->
<!-- This block is generated by finalize-plan-lines.mjs. Do not edit manually. -->
| ID | Anchor | Lines | Description |
|---|---|---|---|
| how-to-use | #how-to-use | L3-L17 | How to use this plan |
| legend | #legend | L19-L53 | Legend |
| goal | #goal | L55-L58 | Goal |
| non-goals | #non-goals | L60-L68 | Non-goals |
| review-flags | #review-flags | L70-L81 | Review Flags |
| global-acceptance | #global-acceptance | L83-L95 | Global Acceptance Criteria |
| legacy-policy | #legacy-policy | L97-L107 | Legacy / workaround policy |
| breaking-changes | #breaking-changes | L109-L118 | Planned Breaking Changes and Propagation Map |
| orchestration | #orchestration | L120-L133 | Subagent Orchestration Plan |
| parallelization-constraints | #parallelization-constraints | L135-L148 | Parallelization Constraints |
| canonical-line-references | #canonical-line-references | L150-L196 | Canonical Line References |
| phases-and-tasks | #phases-and-tasks | L198-L1189 | Phases and Tasks |
| P1 | #P1 | L201-L202 | Phase P1 — Reopen fidelity and establish foundations |
| P1.T1 | #P1.T1 | L204-L255 | Task P1.T1 — TUI semantic closure guard |
| P1.T2 | #P1.T2 | L257-L322 | Task P1.T2 — Selector service boundary contracts |
| P1.T3 | #P1.T3 | L324-L397 | Task P1.T3 — Atomic TUI input and terminal foundation |
| P2 | #P2 | L399-L400 | Phase P2 — Complete reusable TUI surfaces and oracle scaffold |
| P2.T1 | #P2.T1 | L402-L460 | Task P2.T1 — Rich terminal rendering components |
| P2.T2 | #P2.T2 | L462-L518 | Task P2.T2 — Full editor, autocomplete, and list fidelity |
| P2.T3 | #P2.T3 | L520-L572 | Task P2.T3 — Differential oracle scaffold |
| P3 | #P3 | L574-L575 | Phase P3 — Theme and chrome |
| P3.T1 | #P3.T1 | L577-L632 | Task P3.T1 — Themes, assets, footer, status, and chrome |
| P4 | #P4 | L634-L635 | Phase P4 — Interactive component families |
| P4.T1 | #P4.T1 | L637-L691 | Task P4.T1 — Stateful transcript components |
| P4.T2 | #P4.T2 | L693-L747 | Task P4.T2 — Authentication, model, configuration, and settings selectors |
| P4.T3 | #P4.T3 | L749-L796 | Task P4.T3 — Session, tree, trust, and user-message selectors |
| P4.T4 | #P4.T4 | L798-L844 | Task P4.T4 — Custom editor and extension dialogs |
| P4.T5 | #P4.T5 | L846-L891 | Task P4.T5 — Remaining visible Pi components |
| P5 | #P5 | L893-L894 | Phase P5 — Live interactive composition |
| P5.T1 | #P5.T1 | L896-L968 | Task P5.T1 — Stateful transcript/editor/layout composition |
| P6 | #P6 | L970-L971 | Phase P6 — Built-in command fidelity |
| P6.T1 | #P6.T1 | L973-L1029 | Task P6.T1 — Complete built-in slash command routing |
| P7 | #P7 | L1031-L1032 | Phase P7 — Differential end-user proof |
| P7.T1 | #P7.T1 | L1034-L1090 | Task P7.T1 — Pi/Rust terminal oracle and PTY acceptance |
| P8 | #P8 | L1092-L1093 | Phase P8 — Immutable candidate acceptance |
| P8.T1 | #P8.T1 | L1095-L1138 | Task P8.T1 — One-SHA workspace, fidelity, quality, and end-user gates |
| P9 | #P9 | L1140-L1141 | Phase P9 — Truthful evidence and human promotion checkpoint |
| P9.T1 | #P9.T1 | L1143-L1189 | Task P9.T1 — Replace invalid TUI/end-user completion evidence |
| pre-finalization-review | #pre-finalization-review | L1191-L1198 | Pre-finalization Review Summary |
<!-- CANONICAL_LINE_REFERENCES_END -->

<a id="phases-and-tasks"></a>
## Phases and Tasks

<a id="P1"></a>
### Phase P1 — Reopen fidelity and establish foundations

<a id="P1.T1"></a>
### Task P1.T1 — TUI semantic closure guard

Assignable: yes

Execution metadata:
- Wave: W1
- Context: fresh
- Depends on: control adoption precondition
- Can run in parallel with: P1.T2, P1.T3
- Must not run in parallel with: P9.T1

Scope boundaries:
- Goal: make the manifest reject marker-only TUI/interactive modules and the known placeholder live composition.
- Non-goals: no product implementation or documentation edits.
- Forbidden work: no global marker detector outside `zedflow-tui/src` and coding-agent interactive surfaces.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `tools/pi-port-swarm/manifest.py` | Add targeted semantic guards for source-path-only modules and placeholder composition. |
| modify | `tools/pi-port-swarm/test_manifest.py` | Add focused positive/negative regression fixtures. |

Required context package:
- Read RF-C1, global criteria 1, 5, 9, and the execution baseline.
- Read frozen interactive/TUI inventories and current `manifest.py` semantic checks.
- Neighboring out-of-scope units: P1.T3-P6.T1 repair the failures this guard exposes.

Implementation outline:
1. Detect modules whose executable surface is only `source_path()`/marker metadata within the two planned directories.
2. Detect the live `EventLog` plus bespoke `InteractiveInput` composition and ignored assistant update payloads.
3. Report exact source/target paths and do not disposition failures away.

Validation responsibility:
- Type: locally-validating
- Must run: `python3 tools/pi-port-swarm/test_manifest.py`; `python3 tools/pi-port-swarm/manifest.py check` and confirm it fails for the pre-repair candidate with named TUI failures.
- Must NOT run: Cargo workspace gates.

Output contract:
- Commit with guard/tests and durable output showing the intended pre-repair failure set.

Acceptance criteria:
- Test fixtures prove semantic implementations pass and markers/placeholders fail.
- Failures are limited to planned TUI/interactive files.

Handoff to dependent units:
- P8.T1 reruns the same strict manifest and requires zero failures.

Subagent prompt:
```text
Implement only P1.T1 from .agents/plans/pi-tui-end-user-parity-repair.md in fresh context. Modify only tools/pi-port-swarm/manifest.py and test_manifest.py. Add targeted, behavior-preserving guards that reject marker-only modules and the known placeholder live TUI composition; do not broaden to unrelated packages or disposition failures. Run only the listed Python checks and return the exact pre-repair failure inventory.
```

<a id="P1.T2"></a>
### Task P1.T2 — Selector service boundary contracts

Assignable: yes

Execution metadata:
- Wave: W1
- Context: fresh
- Depends on: control adoption precondition
- Can run in parallel with: P1.T1, P1.T3
- Must not run in parallel with: none

Scope boundaries:
- Goal: expose the smallest Pi-compatible Rust service contracts required by theme, auth/model/settings, session/tree/trust, and extension selectors.
- Non-goals: no UI implementation and no command routing.
- Forbidden work: no fake service, global singleton, new dependency, or provider network call.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `crates/zedflow-coding-agent/src/config.rs` | Expose stable config paths/constants used by selectors. |
| modify | `crates/zedflow-coding-agent/src/core/auth-storage.rs` | Deterministic auth/login/logout callback boundary. |
| modify | `crates/zedflow-coding-agent/src/core/model-registry.rs` | Model/provider enumeration and selection boundary. |
| modify | `crates/zedflow-coding-agent/src/core/settings-manager.rs` | Theme/settings/scoped-model persistence boundary. |
| modify | `crates/zedflow-coding-agent/src/core/http-dispatcher.rs` | Existing timeout choices/format contract. |
| modify | `crates/zedflow-coding-agent/src/core/package-manager.rs` | Resolved path/resource metadata contract. |
| modify | `crates/zedflow-coding-agent/src/core/session-manager.rs` | Session list/tree/metadata actions. |
| modify | `crates/zedflow-coding-agent/src/core/trust-manager.rs` | Project trust read/write contract. |
| modify | `crates/zedflow-coding-agent/src/core/keybindings.rs` | App keybinding lookup/display contract. |
| modify | `crates/zedflow-coding-agent/src/utils/open-browser.rs` | Injectable browser-opening result boundary. |
| modify | `crates/zedflow-coding-agent/src/utils/paths.rs` | Canonical/local path resolution used by selectors. |
| modify | `crates/zedflow-coding-agent/tests/auth-storage.rs` | Boundary behavior tests. |
| modify | `crates/zedflow-coding-agent/tests/model-registry.rs` | Boundary behavior tests. |
| modify | `crates/zedflow-coding-agent/tests/settings-manager.rs` | Boundary behavior tests. |
| modify | `crates/zedflow-coding-agent/tests/session-manager/file-operations.rs` | Session mutation boundary tests. |
| modify | `crates/zedflow-coding-agent/tests/trust-manager.rs` | Trust boundary tests. |

Required context package:
- Read RF-R2 and Pi imports in interactive `theme-controller.ts`, `login-dialog.ts`, selectors, and `custom-editor.ts`.
- Read existing Rust APIs before adding methods; reuse them where semantics already match.
- Neighboring units P3.T1 and P4.T2-P4.T4 are read-only consumers of these files.

Implementation outline:
1. Map each frozen callback/action to an existing Rust method or add the smallest typed adapter.
2. Make browser/auth flows injectable for deterministic tests.
3. Preserve persistence and trust-boundary error handling.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --package zedflow-coding-agent --check`; focused tests for the listed test targets.
- Must NOT run: workspace-wide tests.

Output contract:
- Commit plus a contract table mapping Pi selector dependency to Rust symbol.

Acceptance criteria:
- Every downstream selector dependency is available without editing these files later.
- Tests cover success, cancellation, persistence, and error propagation without credentials/network.

Handoff to dependent units:
- P3.T1 and P4.T2-P4.T4 consume the contract table and exact symbols.

Subagent prompt:
```text
Implement only P1.T2 from .agents/plans/pi-tui-end-user-parity-repair.md in fresh context. Own only the listed coding-agent core/config/utils files and focused tests. Reuse existing APIs first, then add the smallest typed adapters needed by frozen Pi theme/selectors. No UI, commands, global state, credentials, or network. Return a Pi-import-to-Rust-symbol contract table.
```

<a id="P1.T3"></a>
### Task P1.T3 — Atomic TUI input and terminal foundation

Assignable: yes

Execution metadata:
- Wave: W1
- Context: fresh
- Depends on: control adoption precondition
- Can run in parallel with: P1.T1, P1.T2
- Must not run in parallel with: P2.T1, P2.T2

Scope boundaries:
- Goal: port the mutually dependent Pi terminal, key, input, width/navigation, kill/yank, undo, and root TUI contracts.
- Non-goals: no coding-agent UI and no rich Markdown/image work.
- Forbidden work: no second terminal backend, renderer thread, alternate screen, or new crate.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `crates/zedflow-tui/src/tui.rs` | Root component tree, focus, overlays, invalidation, differential rendering, resize. |
| modify | `crates/zedflow-tui/src/terminal.rs` | Input protocol, lifecycle, restoration, terminal writes. |
| modify | `crates/zedflow-tui/src/keys.rs` | Frozen Pi key parsing and matching. |
| modify | `crates/zedflow-tui/src/keybindings.rs` | Default/application keybinding resolution. |
| modify | `crates/zedflow-tui/src/stdin-buffer.rs` | Incremental terminal sequence framing. |
| modify | `crates/zedflow-tui/src/native-modifiers.rs` | Native modifier normalization. |
| modify | `crates/zedflow-tui/src/utils.rs` | ANSI-aware width/truncate/wrap primitives. |
| modify | `crates/zedflow-tui/src/fuzzy.rs` | Semantic module API, not marker/re-export evidence only. |
| modify | `crates/zedflow-tui/src/word-navigation.rs` | Semantic module API and Unicode behavior. |
| modify | `crates/zedflow-tui/src/kill-ring.rs` | Pi kill/yank behavior. |
| modify | `crates/zedflow-tui/src/undo-stack.rs` | Pi undo behavior. |
| modify | `crates/zedflow-tui/src/primitives.rs` | Remove inappropriate consolidation after semantic modules own behavior. |
| modify | `crates/zedflow-tui/src/index.rs` | Public Pi-compatible exports. |
| modify | `crates/zedflow-tui/src/components/input.rs` | Cursor-aware input, callbacks, paste and horizontal viewport. |
| modify | `crates/zedflow-tui/tests/input.rs` | Port frozen input assertions. |
| modify | `crates/zedflow-tui/tests/keys.rs` | Port frozen key assertions. |
| modify | `crates/zedflow-tui/tests/keybindings.rs` | Port frozen keybinding assertions. |
| modify | `crates/zedflow-tui/tests/stdin-buffer.rs` | Port sequence framing assertions. |
| modify | `crates/zedflow-tui/tests/word-navigation.rs` | Port Unicode navigation assertions. |
| modify | `crates/zedflow-tui/tests/terminal.rs` | Terminal lifecycle/input assertions. |
| modify | `crates/zedflow-tui/tests/tui-render.rs` | Differential rendering assertions. |
| modify | `crates/zedflow-tui/tests/virtual-terminal.rs` | Deterministic terminal frames. |

Required context package:
- Read frozen Pi counterparts and complete tests, not only current Rust tests.
- Read AGENTS Rust workflow and RF-R1/RF-R5.
- Required skill: ponytail; preserve fidelity/security despite minimal implementation.

Implementation outline:
1. Port key/sequence/Unicode primitives and input callbacks.
2. Port single-owner TUI focus/overlay/render invalidation semantics.
3. Keep terminal output serialized on the owner thread and guarantee restoration on every exit path.
4. Replace consolidated stand-ins with reachable module behavior.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --package zedflow-tui --check`; all listed TUI test targets; `cargo test -p zedflow-tui --lib`.
- Must NOT run: workspace tests or coding-agent tests.

Output contract:
- Commit and parity ledger listing every frozen assertion ported or explicitly blocked.

Acceptance criteria:
- Input supports callbacks, cursor, paste, Unicode, kill/yank/undo, and terminal protocols.
- No live renderer concurrency is introduced.
- Listed Rust tests contain behavior assertions comparable to frozen Pi tests.

Handoff to dependent units:
- P2.T1/P2.T2 receive the public API and parity ledger.

Subagent prompt:
```text
Implement only P1.T3 from .agents/plans/pi-tui-end-user-parity-repair.md in fresh context. Port the frozen Pi terminal/key/input/root-TUI foundation into exactly the listed zedflow-tui files and tests. Read the complete frozen sources/tests. Keep one terminal owner thread, add no dependency or alternate-screen design, and do not touch coding-agent. Return the public API and assertion parity ledger.
```

<a id="P2"></a>
### Phase P2 — Complete reusable TUI surfaces and oracle scaffold

<a id="P2.T1"></a>
### Task P2.T1 — Rich terminal rendering components

Assignable: yes

Execution metadata:
- Wave: W2
- Context: fresh
- Depends on: P1.T3
- Can run in parallel with: P2.T2, P2.T3
- Must not run in parallel with: P3.T1

Scope boundaries:
- Goal: complete Pi-compatible text box, Markdown, images, loaders, colors, and truncation rendering.
- Non-goals: no coding-agent composition.
- Forbidden work: no new image/Markdown dependency or snapshot weakening.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `crates/zedflow-tui/src/components/box.rs` | Border/padding/width semantics. |
| modify | `crates/zedflow-tui/src/components/text.rs` | ANSI-aware text component. |
| modify | `crates/zedflow-tui/src/components/spacer.rs` | Pi spacer behavior. |
| modify | `crates/zedflow-tui/src/components/truncated-text.rs` | Visual truncation. |
| modify | `crates/zedflow-tui/src/components/markdown.rs` | Markdown/ANSI/OSC-8 rendering. |
| modify | `crates/zedflow-tui/src/components/image.rs` | Image component lifecycle. |
| modify | `crates/zedflow-tui/src/components/loader.rs` | Loader animation/state. |
| modify | `crates/zedflow-tui/src/components/cancellable-loader.rs` | Cancellation behavior. |
| modify | `crates/zedflow-tui/src/terminal-image.rs` | Image protocol handling. |
| modify | `crates/zedflow-tui/src/terminal-colors.rs` | Color detection/parsing. |
| modify | `crates/zedflow-tui/src/components/mod.rs` | Semantic exports. |
| modify | `crates/zedflow-tui/tests/markdown.rs` | Frozen Markdown assertions. |
| modify | `crates/zedflow-tui/tests/terminal-image.rs` | Frozen image assertions. |
| modify | `crates/zedflow-tui/tests/terminal-colors.rs` | Frozen color assertions. |
| modify | `crates/zedflow-tui/tests/truncated-text.rs` | Frozen truncation assertions. |
| modify | `crates/zedflow-tui/tests/wrap-ansi.rs` | ANSI/OSC width assertions. |

Required context package:
- Read P1.T3 output and frozen counterpart files/tests.
- Read RF-R1 and global criteria 6-8.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --package zedflow-tui --check`; listed test targets.
- Must NOT run: coding-agent/workspace tests.

Output contract:
- Commit and normalized rendering examples for Markdown, CJK, ANSI, OSC-8, image fallback, and loaders.

Acceptance criteria:
- Frozen deterministic cases match without terminal-specific hardcoding.

Handoff to dependent units:
- P3.T1 and P4.T1 use these components read-only.

Subagent prompt:
```text
Implement only P2.T1 from the plan in fresh context. Complete exactly the listed rich-rendering files/tests against frozen Pi, using the P1.T3 contracts. No coding-agent edits, new dependencies, renderer thread, or snapshot weakening. Return normalized example frames and test evidence.
```

<a id="P2.T2"></a>
### Task P2.T2 — Full editor, autocomplete, and list fidelity

Assignable: yes

Execution metadata:
- Wave: W2
- Context: fresh
- Depends on: P1.T3
- Can run in parallel with: P2.T1, P2.T3
- Must not run in parallel with: P3.T1

Scope boundaries:
- Goal: port the frozen Pi multiline editor and its autocomplete/list contracts.
- Non-goals: no coding-agent custom editor or live composition.
- Forbidden work: no wrapper around primitive `Input` as the final editor.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `crates/zedflow-tui/src/components/editor.rs` | Full editor state machine/rendering. |
| modify | `crates/zedflow-tui/src/editor-component.rs` | Editor trait/options/theme contract. |
| modify | `crates/zedflow-tui/src/autocomplete.rs` | Path/command autocomplete and ranking. |
| modify | `crates/zedflow-tui/src/components/select-list.rs` | Selection/filter/scroll behavior. |
| modify | `crates/zedflow-tui/src/components/settings-list.rs` | Settings list behavior. |
| modify | `crates/zedflow-tui/tests/editor.rs` | Port frozen editor cases. |
| modify | `crates/zedflow-tui/tests/autocomplete.rs` | Port frozen autocomplete cases. |
| modify | `crates/zedflow-tui/tests/select-list.rs` | Port frozen list cases. |
| modify | `crates/zedflow-tui/tests/fuzzy.rs` | Search/ranking assertions. |

Required context package:
- Read complete frozen `editor.ts` and `editor.test.ts`; do not infer parity from current short Rust tests.
- Read P1.T3 public contracts.

Implementation outline:
1. Port cursor/selection/multiline/history/undo/kill/yank and render viewport behavior.
2. Port autocomplete providers, cancellation, navigation, and application keybindings.
3. Port behavior-level frozen assertions.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --package zedflow-tui --check`; listed tests.
- Must NOT run: coding-agent/workspace tests.

Output contract:
- Commit and editor capability matrix tied to test names.

Acceptance criteria:
- Editor is not an `Input` facade and covers global criteria 2 and 7.

Handoff to dependent units:
- P3.T1/P4.T2-P4.T4/P5.T1 consume the editor API.

Subagent prompt:
```text
Implement only P2.T2 from the plan in fresh context. Port the complete frozen Pi editor/autocomplete/list behavior into exactly the listed files/tests. Read full frozen sources and assertions. A thin Input wrapper is forbidden. Return a capability-to-test matrix.
```

<a id="P2.T3"></a>
### Task P2.T3 — Differential oracle scaffold

Assignable: yes

Execution metadata:
- Wave: W2
- Context: fresh
- Depends on: P1.T3
- Can run in parallel with: P2.T1, P2.T2
- Must not run in parallel with: P7.T1

Scope boundaries:
- Goal: create a deterministic, tracked Pi/Rust terminal fixture protocol and runner scaffold.
- Non-goals: no final snapshots or product edits.
- Forbidden work: no untracked pnpm files, network, credentials, or opaque screenshot comparison.

Files:
| Action | Path | Purpose |
|---|---|---|
| create | `tools/tui-parity/run.py` | Stdlib orchestrator for frozen Pi and Rust oracle processes. |
| create | `tools/tui-parity/frozen-pi-oracle.mjs` | Frozen Pi fixture renderer using tracked npm dependencies. |
| create | `tools/tui-parity/README.md` | Reproducible prerequisites/protocol. |
| create | `tools/tui-parity/fixtures/schema.json` | Fixture event/input schema. |
| create | `crates/zedflow-coding-agent/tests/tui-parity-rust.rs` | Rust fixture consumer skeleton. |

Required context package:
- Read `references/pi/package-lock.json`, TUI virtual terminal tests, Rust virtual-terminal tests, RF-R1/RF-R3.
- The runner must use `npm ci` in a disposable frozen-reference workspace or verify an exact installed tree.

Implementation outline:
1. Define JSON input events, dimensions, capabilities, lifecycle events, and normalized frame output.
2. Strip nondeterministic timestamps/paths/terminal queries while preserving visible cells/styles/cursor.
3. Implement one self-test fixture proving both sides speak the protocol; final equality comes in P7.T1.

Validation responsibility:
- Type: locally-validating
- Must run: Python unit/self-check; `cargo test -p zedflow-coding-agent --test tui-parity-rust`; frozen oracle protocol self-check.
- Must NOT run: full parity comparison or workspace tests.

Output contract:
- Commit, protocol schema, reproducible setup command, and self-check outputs.

Acceptance criteria:
- No local pnpm state is required; missing Node/npm fails with actionable diagnostics.

Handoff to dependent units:
- P7.T1 extends fixtures and enforces equality.

Subagent prompt:
```text
Implement only P2.T3 from the plan in fresh context. Create the exact oracle scaffold files using Python stdlib, a tracked .mjs frozen-Pi runner, package-lock/npm-ci reproducibility, and a Rust fixture consumer skeleton. No product edits, network, credentials, screenshots, or untracked pnpm assumptions.
```

<a id="P3"></a>
### Phase P3 — Theme and chrome

<a id="P3.T1"></a>
### Task P3.T1 — Themes, assets, footer, status, and chrome

Assignable: yes

Execution metadata:
- Wave: W3
- Context: fresh
- Depends on: P1.T2, P2.T1, P2.T2
- Can run in parallel with: none
- Must not run in parallel with: P4.T1-P4.T5

Scope boundaries:
- Goal: replace theme/chrome markers and skeletal components with frozen Pi behavior and packaged assets.
- Non-goals: no transcript/selectors/root composition.
- Forbidden work: no hard-coded absolute asset path or marker fallback.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `crates/zedflow-coding-agent/src/modes/interactive/theme/theme.rs` | Theme schema/load/color/style behavior. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/theme/theme-controller.rs` | Settings-aware theme lifecycle. |
| create | `crates/zedflow-coding-agent/src/modes/interactive/theme/dark.json` | Frozen dark theme asset. |
| create | `crates/zedflow-coding-agent/src/modes/interactive/theme/light.json` | Frozen light theme asset. |
| create | `crates/zedflow-coding-agent/src/modes/interactive/theme/theme-schema.json` | Frozen schema asset. |
| create | `crates/zedflow-coding-agent/src/modes/interactive/assets/clankolas.png` | Frozen announcement asset. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/dynamic-border.rs` | Theme-aware border. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/status-indicator.rs` | Status/spinner lifecycle. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/footer.rs` | Model/session/token/footer display. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/keybinding-hints.rs` | Pi key hint rendering. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/bordered-loader.rs` | Loader chrome. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/countdown-timer.rs` | Countdown lifecycle. |
| create | `crates/zedflow-coding-agent/tests/interactive-tui-chrome.rs` | Theme/footer/status behavior tests. |

Required context package:
- Read P1.T2 contract table, P2 outputs, and frozen theme/component/assets.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --package zedflow-coding-agent --check`; `cargo test -p zedflow-coding-agent --test interactive-tui-chrome`; relevant existing footer/status/theme tests.
- Must NOT run: workspace tests.

Output contract:
- Commit and frame examples for dark/light, working/idle/error, narrow/wide footer.

Acceptance criteria:
- Assets work from a built binary in a different cwd.
- No theme/chrome marker remains.

Handoff to dependent units:
- W4 components consume theme/chrome APIs.

Subagent prompt:
```text
Implement only P3.T1 from the plan in fresh context. Port frozen themes/assets/footer/status/chrome into exactly the listed files and tests, using accepted core/TUI contracts. Assets must work from built binaries. No transcript, selector, or root-composition edits.
```

<a id="P4"></a>
### Phase P4 — Interactive component families

<a id="P4.T1"></a>
### Task P4.T1 — Stateful transcript components

Assignable: yes

Execution metadata:
- Wave: W4
- Context: fresh
- Depends on: P2.T1, P3.T1
- Can run in parallel with: P4.T2, P4.T3, P4.T4, P4.T5
- Must not run in parallel with: P5.T1

Scope boundaries:
- Goal: port visible user/assistant/tool/bash/diff/custom/compaction transcript components.
- Non-goals: no session event subscription or root layout.
- Forbidden work: no literal lifecycle placeholder component.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/assistant-message.rs` | Incremental content/thinking rendering. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/user-message.rs` | User content rendering. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/tool-execution.rs` | Tool lifecycle/details. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/bash-execution.rs` | Bash streaming/truncation. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/diff.rs` | Diff rendering. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/custom-message.rs` | Extension/custom message. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/custom-entry.rs` | Extension/custom entry. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/skill-invocation-message.rs` | Skill invocation. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/compaction-summary-message.rs` | Compaction summary. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/branch-summary-message.rs` | Branch summary. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/visual-truncate.rs` | Visual truncation. |
| create | `crates/zedflow-coding-agent/tests/interactive-transcript.rs` | Component state/render tests. |

Required context package:
- Read complete frozen counterparts and Agent message/tool event types.
- Read P2.T1/P3.T1 handoffs.

Validation responsibility:
- Type: locally-validating
- Must run: focused existing component tests and `interactive-transcript`.
- Must NOT run: root interactive/workspace tests.

Output contract:
- Commit and component event-state table.

Acceptance criteria:
- Components update in place from partial to final state and preserve actual content.

Handoff to dependent units:
- P5.T1 maps session events into these component states.

Subagent prompt:
```text
Implement only P4.T1 from the plan in fresh context. Port exactly the listed transcript components/tests against frozen Pi. Components must support incremental state and real content; do not wire session events or edit interactive-mode.rs.
```

<a id="P4.T2"></a>
### Task P4.T2 — Authentication, model, configuration, and settings selectors

Assignable: yes

Execution metadata:
- Wave: W4
- Context: fresh
- Depends on: P1.T2, P2.T2, P3.T1
- Can run in parallel with: P4.T1, P4.T3, P4.T4, P4.T5
- Must not run in parallel with: P5.T1

Scope boundaries:
- Goal: port deterministic UI behavior for login/OAuth/model/config/settings/theme/thinking selectors.
- Non-goals: no real browser/provider call and no root command routing.
- Forbidden work: no edits to P1.T2 core files.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `crates/zedflow-coding-agent/src/modes/interactive/model-search.rs` | Pi model search text/ranking. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/login-dialog.rs` | Login flow UI. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/oauth-selector.rs` | OAuth/provider selection. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/config-selector.rs` | Resource/config selector. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/model-selector.rs` | Model selector/search. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/scoped-models-selector.rs` | Scoped model toggles. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/settings-selector.rs` | Settings editor. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/show-images-selector.rs` | Image mode selector. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/thinking-selector.rs` | Thinking selector. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/theme-selector.rs` | Theme selector. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/first-time-setup.rs` | First-use auth/setup UI. |
| create | `crates/zedflow-coding-agent/tests/interactive-login.rs` | Login/logout fixture tests. |
| create | `crates/zedflow-coding-agent/tests/interactive-settings-selectors.rs` | Selector behavior tests. |

Required context package:
- Read P1.T2 contract table, P2.T2/P3.T1 APIs, and frozen components.

Validation responsibility:
- Type: locally-validating
- Must run: new tests plus existing OAuth/model/settings/first-time-setup tests.
- Must NOT run: network, root interactive, workspace tests.

Output contract:
- Commit and deterministic action/cancel/error callback matrix.

Acceptance criteria:
- No marker remains and every flow can be exercised without credentials/network.

Handoff to dependent units:
- P6.T1 opens these selectors for built-ins.

Subagent prompt:
```text
Implement only P4.T2 from the plan in fresh context. Port the exact auth/model/config/settings selector files/tests using P1.T2 contracts. Do not edit core adapters, interactive-mode.rs, or call network/browser/provider services; use deterministic injected callbacks.
```

<a id="P4.T3"></a>
### Task P4.T3 — Session, tree, trust, and user-message selectors

Assignable: yes

Execution metadata:
- Wave: W4
- Context: fresh
- Depends on: P1.T2, P2.T2, P3.T1
- Can run in parallel with: P4.T1, P4.T2, P4.T4, P4.T5
- Must not run in parallel with: P5.T1

Scope boundaries:
- Goal: port session search/list/delete/rename, tree navigation, trust, and message selection.
- Non-goals: no root command routing.
- Forbidden work: no core adapter edits or destructive real-session fixtures.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/session-selector.rs` | Session list/actions. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/session-selector-search.rs` | Search/filter/sort parity. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/tree-selector.rs` | Tree navigation. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/trust-selector.rs` | Trust decision UI. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/user-message-selector.rs` | Fork/branch message selection. |
| create | `crates/zedflow-coding-agent/tests/interactive-session-selectors.rs` | Deterministic selector tests. |

Required context package:
- Read P1.T2 contracts, frozen components, and existing session-selector tests.

Validation responsibility:
- Type: locally-validating
- Must run: new test plus existing session selector/search/tree/trust tests.
- Must NOT run: root interactive/workspace tests.

Output contract:
- Commit with selection/action parity matrix.

Acceptance criteria:
- Temporary fixtures cover list/search/sort/rename/delete/cancel/tree/trust without touching user data.

Handoff to dependent units:
- P6.T1 invokes selectors for `/resume`, `/tree`, `/fork`, `/clone`, `/trust`.

Subagent prompt:
```text
Implement only P4.T3 from the plan in fresh context. Port exactly the session/tree/trust/message selectors and deterministic tests using P1.T2 contracts. Do not edit core files or interactive-mode.rs and never touch real user sessions.
```

<a id="P4.T4"></a>
### Task P4.T4 — Custom editor and extension dialogs

Assignable: yes

Execution metadata:
- Wave: W4
- Context: fresh
- Depends on: P1.T2, P2.T2, P3.T1
- Can run in parallel with: P4.T1, P4.T2, P4.T3, P4.T5
- Must not run in parallel with: P5.T1

Scope boundaries:
- Goal: port custom editor keybindings and extension input/selection/editor dialogs.
- Non-goals: no extension runtime or root composition changes.
- Forbidden work: no shell/editor execution outside injected fixtures.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/custom-editor.rs` | Application editor/keybindings. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/extension-editor.rs` | External extension editor UI. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/extension-input.rs` | Extension input dialog. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/extension-selector.rs` | Extension selection dialog. |
| create | `crates/zedflow-coding-agent/tests/interactive-extension-dialogs.rs` | Dialog/keybinding tests. |

Required context package:
- Read P1.T2 keybinding contract, P2.T2 editor API, frozen components.

Validation responsibility:
- Type: locally-validating
- Must run: new test and existing extension-input/runner tests relevant to callbacks.
- Must NOT run: real editor command, extension build, workspace tests.

Output contract:
- Commit and callback/keybinding matrix.

Acceptance criteria:
- Dialog focus, submit, cancel, timeout, and cleanup match Pi deterministically.

Handoff to dependent units:
- P5.T1 mounts custom editor; extension runner remains read-only.

Subagent prompt:
```text
Implement only P4.T4 from the plan in fresh context. Port exactly the custom editor and extension dialog files/tests using accepted contracts. External editor/process behavior must be injected; do not edit extension runtime, core adapters, or interactive-mode.rs.
```

<a id="P4.T5"></a>
### Task P4.T5 — Remaining visible Pi components

Assignable: yes

Execution metadata:
- Wave: W4
- Context: fresh
- Depends on: P2.T1, P3.T1, P4.T1
- Can run in parallel with: P4.T2, P4.T3, P4.T4
- Must not run in parallel with: P5.T1

Scope boundaries:
- Goal: complete remaining visible announcement/decorative components so no interactive marker remains.
- Non-goals: no new product behavior.
- Forbidden work: no placeholder art/text or missing-asset fallback presented as parity.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/armin.rs` | Frozen Armin behavior completion. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/daxnuts.rs` | Frozen component behavior. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/earendil-announcement.rs` | Announcement/image behavior. |
| create | `crates/zedflow-coding-agent/tests/interactive-visible-components.rs` | Focused render tests. |

Required context package:
- Read frozen counterparts, P3.T1 asset handoff, and P4.T1 component APIs.

Validation responsibility:
- Type: locally-validating
- Must run: new focused test.
- Must NOT run: workspace tests.

Output contract:
- Commit with no marker-only file in scope.

Acceptance criteria:
- Deterministic narrow/wide and image-fallback frames match frozen behavior.

Handoff to dependent units:
- P5.T1 mounts these only where frozen Pi does.

Subagent prompt:
```text
Implement only P4.T5 from the plan in fresh context. Complete exactly Armin, Daxnuts, and Earendil announcement behavior/tests using the accepted theme/assets/transcript APIs. No placeholders, root composition, or unrelated novelty.
```

<a id="P5"></a>
### Phase P5 — Live interactive composition

<a id="P5.T1"></a>
### Task P5.T1 — Stateful transcript/editor/layout composition

Assignable: yes

Execution metadata:
- Wave: W5
- Context: fresh
- Depends on: P1.T1, P3.T1, P4.T1, P4.T2, P4.T3, P4.T4, P4.T5
- Can run in parallel with: none
- Must not run in parallel with: P6.T1

Scope boundaries:
- Goal: replace the placeholder live tree with Pi-order header/resources/chat/pending/status/widgets/custom editor/footer and stateful event mapping.
- Non-goals: built-in command semantics beyond `/compact` and exit are P6.T1.
- Forbidden work: no EventLog/string transcript, bespoke InteractiveInput, ignored MessageUpdate, or renderer thread.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `crates/zedflow-coding-agent/src/modes/interactive/interactive-mode.rs` | Stateful UI tree and event mapping. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/components/index.rs` | Semantic component exports. |
| modify | `crates/zedflow-coding-agent/src/modes/interactive/mod.rs` | Interactive module graph. |
| modify | `crates/zedflow-coding-agent/src/modes/index.rs` | Replace marker export surface. |
| modify | `crates/zedflow-coding-agent/src/modes/mod.rs` | Public mode exports. |
| modify | `crates/zedflow-coding-agent/src/lib.rs` | Remove flat marker wiring and export semantic modules. |
| modify | `crates/zedflow-coding-agent/src/main.rs` | Mount final composition while preserving owner pump. |
| modify | `crates/zedflow-coding-agent/tests/interactive-mode.rs` | Stateful lifecycle assertions. |
| modify | `crates/zedflow-coding-agent/tests/interactive-mode-suspend.rs` | Owner-pump/restoration assertions. |
| create | `crates/zedflow-coding-agent/tests/interactive-enduser-flow.rs` | Fake-provider full interactive flow. |

Required context package:
- Read every dependency handoff and frozen `interactive-mode.ts` event/layout sections.
- Read AgentHarness event/message/tool types and existing terminal restoration tests.

Implementation outline:
1. Define stable transcript entries keyed by message/tool IDs.
2. Apply `MessageStart → MessageUpdate* → MessageEnd` and tool/queue/compaction/abort/error transitions in place.
3. Compose Pi-order visible regions and use `CustomEditor` as the sole live editor.
4. Request render through the owner-thread TUI after state changes.

Explanatory artifacts:

#### [CANONICAL] Live tree
```text
Root
├─ announcements/resources
├─ transcript (user/assistant/tool/summary entries)
├─ pending/status/loader overlays
├─ active selector/dialog overlay
├─ CustomEditor
└─ Footer
```

Validation responsibility:
- Type: locally-validating
- Must run: listed interactive tests, transcript component test, terminal restoration tests.
- Must NOT run: workspace tests or command tests assigned to P6.T1.

Output contract:
- Commit, event-state table, and captured fake-provider PTY transcript without placeholder labels.

Acceptance criteria:
- Global criteria 1-3 and 8 pass for deterministic fake-provider flows.
- `EventLog`, private `InteractiveInput`, literal `received`, and ignored update branches are removed.

Handoff to dependent units:
- P6.T1 adds complete command routing on this composition.

Subagent prompt:
```text
Implement only P5.T1 from the plan in fresh context. Using all accepted component handoffs, replace the placeholder interactive EventLog/Input with a Pi-order stateful transcript/editor/footer/overlay tree and map every message/tool/queue/compaction lifecycle update in place. Modify only listed files/tests. Do not implement built-in commands beyond existing compact/exit semantics and do not add rendering concurrency.
```

<a id="P6"></a>
### Phase P6 — Built-in command fidelity

<a id="P6.T1"></a>
### Task P6.T1 — Complete built-in slash command routing

Assignable: yes

Execution metadata:
- Wave: W6
- Context: fresh
- Depends on: P5.T1
- Can run in parallel with: none
- Must not run in parallel with: P5.T1, P7.T1

Scope boundaries:
- Goal: intercept and implement every frozen built-in command before model prompting.
- Non-goals: extension commands already owned by extension runtime; no live provider login.
- Forbidden work: no catch-all prompt fallback for known built-ins and no silent no-op.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `crates/zedflow-coding-agent/src/modes/interactive/interactive-mode.rs` | Command dispatch and selector/action integration. |
| modify | `crates/zedflow-coding-agent/src/main.rs` | Runtime services supplied to command dispatcher. |
| modify | `crates/zedflow-coding-agent/src/core/slash-commands.rs` | Complete command metadata/dispatch identifiers. |
| create | `crates/zedflow-coding-agent/tests/interactive-builtins.rs` | One deterministic path per built-in. |
| modify | `crates/zedflow-coding-agent/tests/interactive-mode-compaction.rs` | Compact command parity. |
| modify | `crates/zedflow-coding-agent/tests/interactive-mode-import-command.rs` | Import command parity. |
| modify | `crates/zedflow-coding-agent/tests/interactive-mode-clone-command.rs` | Clone command parity. |

Required context package:
- Read P5.T1 composition/event contracts, P4 selector outputs, P1.T2 service contracts, and complete frozen command switch.
- Fixed inventory is global criterion 4 plus `/exit` alias behavior.

Implementation outline:
1. Parse exact command name and args once; preserve prompt/skill/extension precedence matching Pi.
2. Dispatch every listed built-in to a typed action/selector.
3. Test success/cancel/error and assert no session prompt for each command.
4. Preserve unknown slash input behavior exactly as frozen Pi.

Validation responsibility:
- Type: locally-validating
- Must run: `interactive-builtins` and listed existing command tests; focused auth/session/package tests as read-only regression gates.
- Must NOT run: workspace tests or provider network.

Output contract:
- Commit and command matrix: parser, action, fixture, prompt-call count.

Acceptance criteria:
- Every built-in has a tested non-placeholder path and prompt-call count zero.
- `/login` opens deterministic auth UI; `/logout` mutates only fixture auth storage.

Handoff to dependent units:
- P7.T1 records command fixtures in the differential oracle.

Subagent prompt:
```text
Implement only P6.T1 from the plan in fresh context. Complete the fixed built-in inventory in interactive-mode/main/slash-commands and exact tests. Known built-ins must never reach session.prompt; each needs success/cancel/error fixture coverage. Use accepted selectors/services, no network, no unrelated core edits, and preserve frozen precedence for extensions/prompts/skills/unknown slash text.
```

<a id="P7"></a>
### Phase P7 — Differential end-user proof

<a id="P7.T1"></a>
### Task P7.T1 — Pi/Rust terminal oracle and PTY acceptance

Assignable: yes

Execution metadata:
- Wave: W7
- Context: fresh
- Depends on: P2.T3, P6.T1
- Can run in parallel with: none
- Must not run in parallel with: P8.T1

Scope boundaries:
- Goal: finish deterministic same-input Pi/Rust terminal comparison and end-user PTY tests.
- Non-goals: no product behavior changes.
- Forbidden work: no fixture normalization that removes visible semantic differences and no snapshot blessing without explaining the Pi source frame.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `tools/tui-parity/run.py` | Execute/compare all fixtures. |
| modify | `tools/tui-parity/frozen-pi-oracle.mjs` | Render frozen Pi frames. |
| modify | `tools/tui-parity/README.md` | Exact reproducible commands. |
| create | `tools/tui-parity/fixtures/input-editing.json` | Editor/key/paste/history fixture. |
| create | `tools/tui-parity/fixtures/streaming.json` | Message lifecycle fixture. |
| create | `tools/tui-parity/fixtures/tools-compaction.json` | Tool/compaction fixture. |
| create | `tools/tui-parity/fixtures/commands.json` | Built-in selector/action fixture. |
| create | `tools/tui-parity/fixtures/overlays.json` | Selector/overlay fixture. |
| create | `tools/tui-parity/fixtures/unicode-resize.json` | CJK/emoji/ANSI/resize fixture. |
| create | `tools/tui-parity/fixtures/abort-error.json` | Abort/error/restoration fixture. |
| modify | `crates/zedflow-coding-agent/tests/tui-parity-rust.rs` | Render Rust normalized frames. |
| create | `crates/zedflow-coding-agent/tests/interactive-pty-parity.rs` | PTY lifecycle and terminal restoration. |
| modify | `crates/zedflow-coding-agent/tests/interactive-terminal-restoration.rs` | Extend exit/error restoration cases. |

Required context package:
- Read oracle protocol handoff, final composition/command matrices, RF-R1/RF-R3/RF-R4.
- Use frozen `package-lock.json`; record exact Node/npm versions.

Validation responsibility:
- Type: integration-validating for TUI/coding-agent parity only
- Must run: all oracle fixtures; `cargo test -p zedflow-tui --all-targets`; `cargo test -p zedflow-coding-agent --test tui-parity-rust --test interactive-pty-parity --test interactive-terminal-restoration`.
- Must NOT run: full workspace gate owned by P8.T1; no provider network.

Output contract:
- Commit, equal normalized frame artifacts, PTY byte/restoration evidence, exact tool versions.

Acceptance criteria:
- Every fixture compares equal or has an explicit platform disposition approved in RF-R1.
- The test fails on literal `received`, dropped update content, command-to-prompt routing, or terminal restoration leaks.

Handoff to dependent units:
- P8.T1 reruns commands read-only at the candidate SHA.

Subagent prompt:
```text
Implement only P7.T1 from the plan in fresh context. Complete the exact oracle/fixture/PTTY test files and compare frozen Pi versus final Rust behavior. Do not edit product code, weaken normalization, bless unexplained frames, use untracked pnpm state, credentials, or network. Return equal frame artifacts and terminal restoration evidence.
```

<a id="P8"></a>
### Phase P8 — Immutable candidate acceptance

<a id="P8.T1"></a>
### Task P8.T1 — One-SHA workspace, fidelity, quality, and end-user gates

Assignable: yes

Execution metadata:
- Wave: W8
- Context: fresh
- Depends on: P1.T1, P7.T1
- Can run in parallel with: none
- Must not run in parallel with: any writer

Scope boundaries:
- Goal: validate one exact immutable candidate SHA without edits.
- Non-goals: no repair, docs, promotion, or snapshot update.
- Forbidden work: any file mutation; a failure is a blocker requiring a new repair unit and a new SHA.

Files:
| Action | Path | Purpose |
|---|---|---|
| read | entire clean candidate worktree | Validation only. |

Required context package:
- Read all unit handoffs, RF-C1/RF-R1/RF-R4, global criteria, controller state, and exact candidate SHA.
- Required reviewers: independent Pi-fidelity reviewer, Rust-quality reviewer, and end-user reviewer, each fresh/read-only.

Validation responsibility:
- Type: integration-validating
- Must run: `cargo fmt --all --check`; `cargo check --workspace --all-targets`; `cargo test --workspace --all-targets`; `python3 tools/pi-port-swarm/manifest.py check`; all `tools/tui-parity/run.py` fixtures; controller-declared validations; fresh independent reviews; manual PTY checklist with deterministic fake provider.
- Must NOT run: code format mutation, cargo fix, docs edits, promotion, provider network.

Output contract:
- Exact SHA, command logs/return codes, oracle artifacts, three independent review outcomes, ignored-test inventory, residual risks.

Acceptance criteria:
- Every command returns zero, no unexplained ignore/marker/placeholder remains in planned surfaces, and all reviews accept the same SHA.

Handoff to dependent units:
- P9.T1 records this exact product SHA; it must not substitute its later docs SHA.

Subagent prompt:
```text
Execute only P8.T1 from the plan in fresh read-only context on the exact candidate SHA supplied by the controller. Run every listed workspace, manifest, differential, PTY, and independent review gate. Do not edit or repair anything. On any failure, return the exact blocker and stop. On success, return immutable SHA and complete evidence bundle.
```

<a id="P9"></a>
### Phase P9 — Truthful evidence and human promotion checkpoint

<a id="P9.T1"></a>
### Task P9.T1 — Replace invalid TUI/end-user completion evidence

Assignable: yes

Execution metadata:
- Wave: W9
- Context: fresh
- Depends on: P8.T1 accepted
- Can run in parallel with: none
- Must not run in parallel with: any validation/writer unit

Scope boundaries:
- Goal: record the tested product SHA and corrected TUI/end-user evidence, then stop for RF-BQ1.
- Non-goals: no product code, DAG mutation, promotion, or claim that the docs commit was tested.
- Forbidden work: no promotion to `main` and no Stage-2 authorization.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `docs/porting/BASELINE.md` | Replace contradictory checkpoint with accepted product-SHA evidence and remaining promotion gate. |
| modify | `.agents/state/stage-1-final-gate-evidence.md` | Record one-SHA commands/reviews/artifacts. |
| modify | `.agents/port-swarm/decisions.md` | Record revocation and replacement of prior TUI/end-user acceptance. |

Required context package:
- Read P8.T1 immutable evidence, RF-C1/RF-BQ1, current baseline/evidence/decisions.

Validation responsibility:
- Type: locally-validating
- Must run: `git diff --check`; verify every recorded SHA/log path against P8 output.
- Must NOT run: product tests as evidence for the docs commit; controller dispatch; promotion.

Output contract:
- Documentation commit naming the tested product SHA separately from the evidence commit, plus explicit human promotion request.

Acceptance criteria:
- No text claims the documentation SHA passed product gates.
- Prior invalid TUI/end-user acceptance is explicitly superseded.
- Stage 2 remains forbidden and RF-BQ1 remains visible.

Handoff to dependent units:
- Human decides promotion. After approval, a fresh read-only validator repeats P8.T1 on promoted `main` before Stage 1 can close.

Subagent prompt:
```text
Implement only P9.T1 from the plan in fresh context after P8 acceptance. Modify only the three evidence/docs files. Record the exact tested product SHA and explicitly distinguish the later docs commit. Supersede prior invalid TUI/end-user evidence, keep Stage 2 forbidden, request human promotion approval, and do not run product tests or promote main.
```

<a id="pre-finalization-review"></a>
## Pre-finalization Review Summary

| Reviewer | Status | Required changes applied | Remaining concerns |
|---|---|---|---|
| Feasibility / file references | accepted after revision | Added completed-DAG control adoption, `model-search.rs`, module-export files, fixed core boundary ownership, assets, and tracked npm oracle prerequisite. | Selector core contracts remain a bounded implementation risk under RF-R2. |
| Sequencing / dependency graph | accepted after revision | Combined input/terminal foundation, sequenced rich renderer/editor/theme/transcript, isolated same-file interactive writes, and separated validation from evidence. | No unresolved write/write conflicts. |
| Scope isolation / prompt quality | accepted after revision | Removed conditional scopes, assigned omitted marker/re-export files, fixed full command inventory, exact tests/commands, and one-SHA protocol. | PTY platform variance remains RF-R1; human promotion remains RF-BQ1. |
