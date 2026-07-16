<!-- migration-document-status: SUPERSEDED -->
> [!CAUTION]
> **Migration status: SUPERSEDED.** Historical plan only. Use `.agents/plans/zedflow-ai-agent-pi-fidelity-consolidation.md` and `.agents/state/zedflow-ai-agent-pi-fidelity-current-status.md` for current work.

# Pi to Rust Package Port

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

Port Pi TypeScript packages from `references/pi/packages/` to Rust crates in `crates/` with behavior matching Pi, package structure matching Pi as closely as Rust module conventions allow, documented Rust public APIs, and tests ported to Rust. The port is an identity port of Pi, not a Zedflow feature implementation.

<a id="non-goals"></a>
## Non-goals

- Do not implement Zedflow Flow, Runtime Graph, LangGraph sidecar, or product-specific behavior.
- Do not preserve the inherited monolithic Rust port except as optional reference material explicitly named by a unit.
- Do not add compatibility shims for the old monolith.
- Do not replace missing third-party integrations with speculative new designs; use documented placeholders.
- Do not run live provider/network tests unless the original Pi test is explicitly live and the unit declares it.

<a id="review-flags"></a>
## Review Flags

| ID | Type | Severity | Summary | Affects | Follow-up / accepted risk |
|---|---|---:|---|---|---|
| RF1 | R | Medium | Some TypeScript dependencies have no selected Rust equivalent. | All package chains | Use the required `PORT PLACEHOLDER` marker and keep APIs documented/compilable. |
| RF2 | R | Medium | `ai` and `coding-agent` are large; a package-level chain will fan out to many file subagents. | W2, W6 | Use manifest rows as file-unit boundaries; sequence same-target conflicts. |
| RF3 | OQ | Low | Some TypeScript tests are integration or live-network tests. | Test chains | Port as ignored Rust tests with documented reason unless deterministic local behavior is possible. |

<a id="global-acceptance"></a>
## Global Acceptance Criteria

- Every TypeScript source file listed in `.agents/port-manifests/*-src.tsv` has a corresponding Rust target file or a documented file-level `PORT PLACEHOLDER` explaining why it cannot be implemented yet.
- Every TypeScript test file listed in `.agents/port-manifests/*-tests.tsv` is ported to Rust unit/integration tests, or represented by an ignored test with an explicit parity blocker.
- Every public Rust item added by the port has rustdoc documentation, including `# Errors` for fallible public functions and `# Panics` only when panics are intentional invariants.
- All implementation subagents load and apply `/home/zedium/workspaces/zedflow/.agents/skills/rust-skills/SKILL.md` before editing.
- No Zedflow-specific behavior is introduced.
- Final validation passes: `cargo fmt --all --check`, `cargo check --workspace --all-targets`, `cargo test --workspace --all-targets --no-run`, and `cargo doc --workspace --no-deps`.

<a id="legacy-policy"></a>
## Legacy / workaround policy

- No temporary aliases unless explicitly listed as allowed in a task.
- No compatibility shims unless they are the goal of a task.
- No type weakening to satisfy intermediate compilation.
- No preserving legacy names when the planned change is a removal or rename.
- Do not expand a task scope to fix breakages assigned to later units.
- If blocked, report the breakage and reference the downstream unit responsible.
- The only allowed placeholder is the documented `PORT PLACEHOLDER` shape in this plan.

<a id="breaking-changes"></a>
## Planned Breaking Changes and Propagation Map

| Change | Introduced by | Expected temporary breakage | Fixed by | Forbidden workaround |
|---|---|---|---|---|
| New Rust crate APIs replace empty skeleton crates. | Package source chains W2-W6 | Downstream crates may fail until dependency package chains finish. | Package integration gates and W7 final validation. | Re-exporting old monolith APIs to hide missing package APIs. |
| Tests move into package crates instead of old root integration-test layout. | Package test chains W2-W6 | Some package tests may be ignored until source parity exists. | Same package source/test completion. | Network-only tests or fake success tests without parity assertions. |

<a id="orchestration"></a>
## Subagent Orchestration Plan

- W1: Run P0.T1 once.
- W2: Run P1.T1, then P1.T2 after P1.T1 completes.
- W3: Run P2.T1, then P2.T2 after P2.T1 completes.
- W4: Run P3.T1, then P3.T2 after P3.T1 completes.
- W5: Run P4.T1. P4.T2 is skipped unless `.agents/port-manifests/orchestrator-tests.tsv` later contains rows.
- W6: Run P5.T1, then P5.T2 after P5.T1 completes.
- W7: Run P6.T1 after W2-W6 complete.
- For each package source/test task, the orchestrator must launch one fresh subagent per non-empty TSV row in that task's manifest. The row's first column is the source reference file and the second column is the Rust target file. Each row-subagent gets the task prompt plus its row values.

<a id="parallelization-constraints"></a>
## Parallelization Constraints

| Constraint | Reason | Affected units |
|---|---|---|
| Do not run a package test chain before its source chain. | Tests depend on source module shape. | P1.T2, P2.T2, P3.T2, P5.T2 |
| Do not run downstream package source chains before dependencies unless the subagent is told to add placeholders only. | Avoid speculative public APIs. | agent after ai/core/tools/session, coding-agent after agent/orchestrator/tui |
| Same target file writes are sequential. | Multiple TS tests may map to one Rust module or integration file. | All manifest row-subagents |
| All row-subagents use fresh context and must load rust-skills. | User requirement and Rust quality gate. | All assignable units |

<a id="canonical-line-references"></a>
## Canonical Line References

<!-- CANONICAL_LINE_REFERENCES_START -->
<!-- This block is generated by finalize-plan-lines.mjs. Do not edit manually. -->
| ID | Anchor | Lines | Description |
|---|---|---|---|
| how-to-use | #how-to-use | L3-L15 | How to use this plan |
| legend | #legend | L17-L51 | Legend |
| goal | #goal | L53-L56 | Goal |
| non-goals | #non-goals | L58-L65 | Non-goals |
| review-flags | #review-flags | L67-L74 | Review Flags |
| global-acceptance | #global-acceptance | L76-L84 | Global Acceptance Criteria |
| legacy-policy | #legacy-policy | L86-L95 | Legacy / workaround policy |
| breaking-changes | #breaking-changes | L97-L103 | Planned Breaking Changes and Propagation Map |
| orchestration | #orchestration | L105-L115 | Subagent Orchestration Plan |
| parallelization-constraints | #parallelization-constraints | L117-L125 | Parallelization Constraints |
| canonical-line-references | #canonical-line-references | L127-L134 | Canonical Line References |
| phases-and-tasks | #phases-and-tasks | L136-L796 | Phases and Tasks |
| P0.T1 | #P0.T1 | L139-L217 | Task P0.T1 — Porting infrastructure and rules |
| P1.T1 | #P1.T1 | L219-L289 | Task P1.T1 — Port `packages/ai` source files |
| P1.T2 | #P1.T2 | L291-L351 | Task P1.T2 — Port `packages/ai` tests |
| P2.T1 | #P2.T1 | L353-L411 | Task P2.T1 — Port `packages/agent` source files |
| P2.T2 | #P2.T2 | L413-L464 | Task P2.T2 — Port `packages/agent` tests |
| P3.T1 | #P3.T1 | L466-L518 | Task P3.T1 — Port `packages/tui` source files |
| P3.T2 | #P3.T2 | L520-L571 | Task P3.T2 — Port `packages/tui` tests |
| P4.T1 | #P4.T1 | L573-L625 | Task P4.T1 — Port `packages/orchestrator` source files |
| P5.T1 | #P5.T1 | L627-L680 | Task P5.T1 — Port `packages/coding-agent` source files |
| P5.T2 | #P5.T2 | L682-L734 | Task P5.T2 — Port `packages/coding-agent` tests |
| P6.T1 | #P6.T1 | L736-L796 | Task P6.T1 — Final workspace integration validation |
| pre-finalization-review | #pre-finalization-review | L798-L805 | Pre-finalization Review Summary |
<!-- CANONICAL_LINE_REFERENCES_END -->

<a id="phases-and-tasks"></a>
## Phases and Tasks

<a id="P0.T1"></a>
### Task P0.T1 — Porting infrastructure and rules

Assignable: yes

Execution metadata:
- Wave: W1
- Context: fresh
- Depends on: none
- Can run in parallel with: none
- Must not run in parallel with: package source/test chains

Scope boundaries:
- Goal: Create shared porting infrastructure and rules used by all package chains.
- Non-goals: Do not port package implementation files.
- Forbidden work: No Zedflow-specific runtime behavior; no old monolith compatibility layer.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `crates/zedflow-core/src/lib.rs` | Expose shared core modules. |
| create | `crates/zedflow-core/src/error.rs` | Common error type/conventions for ported packages. |
| create | `crates/zedflow-core/src/placeholders.rs` | Canonical placeholder helpers/marker docs. |
| create | `crates/zedflow-core/src/porting.rs` | Porting metadata helpers if needed. |
| modify | `crates/zedflow-*/Cargo.toml` | Add shared workspace dependency conventions only where needed. |
| create | `docs/planning/PI_RUST_PORTING_RULES.md` | Human-readable porting rules and placeholder policy. |

Required context package:
- Plan references: goal, global acceptance, legacy policy, RF1.
- Required skills: read `/home/zedium/workspaces/zedflow/.agents/skills/rust-skills/SKILL.md` completely enough to apply documentation, API, error, async, and testing rules.
- Required files/symbols to read: root `Cargo.toml`, all `crates/zedflow-*/Cargo.toml`, `docs/planning/ZEDFLOW_WORKSPACE_ARCHITECTURE.md`.
- Required URLs/docs: none.
- Dependency outputs to read: none.
- Neighboring out-of-scope units: all package source/test chains.

Implementation outline:
1. Add minimal, documented shared error and placeholder modules.
2. Document the exact `PORT PLACEHOLDER` marker and when it is allowed.
3. Keep code small; do not design future Zedflow abstractions.

Major snippets:

#### [CANONICAL] Placeholder marker
```rust
/// PORT PLACEHOLDER:
/// Original dependency: `<npm package / API>`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `<exact Pi behavior to preserve>`.
/// Replacement decision needed before production use.
```

Validation responsibility:
- Type: integration-validating
- Must run: `cargo fmt --all --check`; `cargo check --workspace --all-targets`
- Must NOT run: live network tests
- Expected temporary breakage: none
- Forbidden fixes/workarounds: old monolith re-exports or blanket compatibility aliases

Output contract:
- List files changed.
- List placeholder API introduced.
- List any package Cargo changes.

Acceptance criteria:
- Shared placeholder policy compiles and is documented.
- Porting rules doc exists and matches this plan.

Handoff to dependent units:
- Package chain subagents use the placeholder marker and rules from this task.

Subagent prompt:
```text
You are implementing only P0.T1 from .agents/plans/pi-to-rust-package-port.md.
Run in fresh context. First read /home/zedium/workspaces/zedflow/.agents/skills/rust-skills/SKILL.md and apply it strictly.
Read the plan references and required files listed in P0.T1 before editing.
Do not edit outside the listed file scope unless you first report why the plan is insufficient.
Do not implement package ports.
Task: create minimal shared Rust porting infrastructure, documented placeholder policy, and porting rules.
```

<a id="P1.T1"></a>
### Task P1.T1 — Port `packages/ai` source files

Assignable: yes

Execution metadata:
- Wave: W2
- Context: fresh per manifest row
- Depends on: P0.T1
- Can run in parallel with: other P1.T1 rows that do not write the same target file
- Must not run in parallel with: P1.T2 rows targeting the same module

Scope boundaries:
- Goal: Port every row in `.agents/port-manifests/ai-src.tsv` from Pi TypeScript to `crates/zedflow-ai` Rust.
- Non-goals: Do not port tests except local unit tests required by the source file.
- Forbidden work: No live provider calls in unit tests; no Zedflow behavior.

Files:
| Action | Path | Purpose |
|---|---|---|
| read | `.agents/port-manifests/ai-src.tsv` | Exact source-to-target file scopes; one subagent per row. |
| read | `references/pi/packages/ai/src/**/*.ts` | Source reference files listed in manifest. |
| create/modify | `crates/zedflow-ai/src/**/*.rs` | Rust target files listed in manifest. |
| modify | `crates/zedflow-ai/src/lib.rs` | Module exposure as required by targets. |
| modify | `crates/zedflow-ai/Cargo.toml` | Dependencies required by implemented files. |

Required context package:
- Plan references: goal, global acceptance, legacy policy, RF1, RF2.
- Required skills: read `/home/zedium/workspaces/zedflow/.agents/skills/rust-skills/SKILL.md` before editing.
- Required files/symbols to read: row source file, neighboring source files it imports, existing `crates/zedflow-ai/src/lib.rs`.
- Required URLs/docs: none.
- Dependency outputs to read: P0.T1 output.
- Neighboring out-of-scope units: other package crates, ai test rows not assigned.

Implementation outline:
1. For each manifest row, create the matching Rust file and module declarations.
2. Preserve Pi public semantics and naming where idiomatic Rust allows.
3. Document every public item.
4. Add `PORT PLACEHOLDER` only for unresolved external dependencies.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; `cargo check -p zedflow-ai`
- Must NOT run: workspace-wide fixes; live provider/network tests
- Expected temporary breakage: downstream crates may fail until later phases
- Forbidden fixes/workarounds: fake provider success paths, monolith re-exports

Output contract:
- Manifest row processed.
- Files changed.
- Behavior mapped.
- Placeholders added.
- Local tests added.
- Parity blockers.

Acceptance criteria:
- Assigned file compiles or has a documented placeholder blocker.
- Public Rust API is documented.

Handoff to dependent units:
- P1.T2 ports tests against the created APIs; P2.T1 depends on stable ai APIs.

Subagent prompt:
```text
You are a fresh subagent implementing one row of P1.T1 from .agents/plans/pi-to-rust-package-port.md.
First read /home/zedium/workspaces/zedflow/.agents/skills/rust-skills/SKILL.md and apply it strictly.
Read .agents/port-manifests/ai-src.tsv and use only your assigned row.
Source: <ROW_SOURCE>
Target: <ROW_TARGET>
Port exactly this Pi TypeScript source file to Rust. Preserve Pi behavior. Do not add Zedflow behavior. Document every public Rust item. Use the plan's PORT PLACEHOLDER marker for unresolved external dependencies. Edit only the target file, required module declarations in crates/zedflow-ai/src/lib.rs, and crates/zedflow-ai/Cargo.toml if necessary. Run only the validation commands allowed by P1.T1. Report blockers instead of inventing compatibility workarounds.
```

<a id="P1.T2"></a>
### Task P1.T2 — Port `packages/ai` tests

Assignable: yes

Execution metadata:
- Wave: W2
- Context: fresh per manifest row
- Depends on: P1.T1
- Can run in parallel with: other P1.T2 rows that do not write the same target file
- Must not run in parallel with: same-target rows

Scope boundaries:
- Goal: Port every row in `.agents/port-manifests/ai-tests.tsv` into Rust tests.
- Non-goals: Do not implement missing source behavior except tiny test-only helpers.
- Forbidden work: No live network tests unless original test is live and marked ignored.

Files:
| Action | Path | Purpose |
|---|---|---|
| read | `.agents/port-manifests/ai-tests.tsv` | Exact test scopes; one subagent per row. |
| read | `references/pi/packages/ai/test/**/*.ts` | Test reference files listed in manifest. |
| modify | `crates/zedflow-ai/src/**/*.rs` | Preferred co-located unit tests for source behavior. |
| create/modify | `crates/zedflow-ai/tests/**/*.rs` | Integration tests when co-location is inappropriate. |

Required context package:
- Plan references: global acceptance, RF3, P1.T2.
- Required skills: read `/home/zedium/workspaces/zedflow/.agents/skills/rust-skills/SKILL.md` before editing.
- Required files/symbols to read: row test file and Rust source under test.
- Dependency outputs to read: relevant P1.T1 row output.

Implementation outline:
1. Preserve assertions and fixtures from the TS test.
2. Prefer deterministic local tests.
3. Mark live or blocked parity tests as `#[ignore]` with a clear reason.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; targeted `cargo test -p zedflow-ai <test_name>` when possible
- Must NOT run: live provider calls; global workaround fixes

Output contract:
- Test row processed.
- Tests added/ignored with reasons.
- Missing source blockers.

Acceptance criteria:
- Assigned test behavior is represented in Rust.

Handoff to dependent units:
- P6.T1 validates full workspace after all packages.

Subagent prompt:
```text
You are a fresh subagent implementing one row of P1.T2 from .agents/plans/pi-to-rust-package-port.md.
First read /home/zedium/workspaces/zedflow/.agents/skills/rust-skills/SKILL.md and apply it strictly.
Read .agents/port-manifests/ai-tests.tsv and use only your assigned row.
Source test: <ROW_SOURCE>
Preferred target: <ROW_TARGET>
Port exactly this Pi TypeScript test to Rust. Prefer #[cfg(test)] in the Rust source file under test; use crates/zedflow-ai/tests only for integration-level behavior. Preserve assertions. Do not add Zedflow behavior. If source behavior is still placeholder/incomplete, add an ignored test with a documented reason. Run only P1.T2 allowed validation.
```

<a id="P2.T1"></a>
### Task P2.T1 — Port `packages/agent` source files

Assignable: yes

Execution metadata:
- Wave: W3
- Context: fresh per manifest row
- Depends on: P0.T1, P1.T1
- Can run in parallel with: other P2.T1 rows that do not write the same target file
- Must not run in parallel with: P2.T2 rows targeting the same module

Scope boundaries:
- Goal: Port every row in `.agents/port-manifests/agent-src.tsv` to `crates/zedflow-agent`.
- Non-goals: Do not implement coding-agent CLI or TUI behavior.
- Forbidden work: No Zedflow orchestration behavior.

Files:
| Action | Path | Purpose |
|---|---|---|
| read | `.agents/port-manifests/agent-src.tsv` | Exact file scopes. |
| read | `references/pi/packages/agent/src/**/*.ts` | Source references. |
| create/modify | `crates/zedflow-agent/src/**/*.rs` | Rust targets. |
| modify | `crates/zedflow-agent/src/lib.rs` | Module exposure. |
| modify | `crates/zedflow-agent/Cargo.toml` | Required dependencies. |

Required context package:
- Required skills: read `/home/zedium/workspaces/zedflow/.agents/skills/rust-skills/SKILL.md` before editing.
- Required files/symbols to read: assigned row source, imported Pi files, relevant zedflow-ai APIs.
- Dependency outputs to read: P0.T1, relevant P1.T1 rows.

Implementation outline:
1. Port agent loop, harness, session abstractions, skills and utilities as Pi behavior.
2. Use placeholders for environment-specific Node APIs not yet selected in Rust.
3. Document all public items.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; `cargo check -p zedflow-agent`
- Must NOT run: global workaround fixes

Output contract:
- Same as P1.T1, adapted to package `agent`.

Acceptance criteria:
- Assigned file behavior is represented and documented.

Handoff to dependent units:
- P2.T2 ports tests; P5.T1 coding-agent depends on agent APIs.

Subagent prompt:
```text
You are a fresh subagent implementing one row of P2.T1 from .agents/plans/pi-to-rust-package-port.md.
First read /home/zedium/workspaces/zedflow/.agents/skills/rust-skills/SKILL.md and apply it strictly.
Read .agents/port-manifests/agent-src.tsv and use only your assigned row.
Source: <ROW_SOURCE>
Target: <ROW_TARGET>
Port exactly this Pi agent source file to Rust. Preserve Pi behavior, document public items, use PORT PLACEHOLDER for unresolved external dependencies, and do not add Zedflow behavior. Edit only the target, required module declarations, and package Cargo.toml if necessary.
```

<a id="P2.T2"></a>
### Task P2.T2 — Port `packages/agent` tests

Assignable: yes

Execution metadata:
- Wave: W3
- Context: fresh per manifest row
- Depends on: P2.T1
- Can run in parallel with: other P2.T2 rows that do not write the same target file
- Must not run in parallel with: same-target rows

Files:
| Action | Path | Purpose |
|---|---|---|
| read | `.agents/port-manifests/agent-tests.tsv` | Exact test scopes. |
| read | `references/pi/packages/agent/test/**/*.ts` | Test references. |
| modify | `crates/zedflow-agent/src/**/*.rs` | Co-located unit tests. |
| create/modify | `crates/zedflow-agent/tests/**/*.rs` | Integration tests if required. |

Required context package:
- Required skills: read `/home/zedium/workspaces/zedflow/.agents/skills/rust-skills/SKILL.md` before editing.
- Required files/symbols to read: assigned test and Rust source under test.

Implementation outline:
1. Port assertions exactly.
2. Keep tests deterministic.
3. Use ignored tests for unavailable environment behavior.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; targeted `cargo test -p zedflow-agent <test_name>` when possible
- Must NOT run: global workaround fixes

Output contract:
- Same as P1.T2, adapted to package `agent`.

Acceptance criteria:
- Assigned test is represented in Rust.

Handoff to dependent units:
- P6.T1 validates full workspace.

Subagent prompt:
```text
You are a fresh subagent implementing one row of P2.T2 from .agents/plans/pi-to-rust-package-port.md.
First read /home/zedium/workspaces/zedflow/.agents/skills/rust-skills/SKILL.md and apply it strictly.
Read .agents/port-manifests/agent-tests.tsv and use only your assigned row.
Source test: <ROW_SOURCE>
Preferred target: <ROW_TARGET>
Port exactly this Pi agent test to Rust. Prefer co-located #[cfg(test)] tests. Preserve assertions. Use #[ignore] only for documented parity blockers. Do not implement neighboring source files.
```

<a id="P3.T1"></a>
### Task P3.T1 — Port `packages/tui` source files

Assignable: yes

Execution metadata:
- Wave: W4
- Context: fresh per manifest row
- Depends on: P0.T1
- Can run in parallel with: other P3.T1 rows that do not write the same target file
- Must not run in parallel with: P3.T2 rows targeting the same module

Files:
| Action | Path | Purpose |
|---|---|---|
| read | `.agents/port-manifests/tui-src.tsv` | Exact source scopes. |
| read | `references/pi/packages/tui/src/**/*.ts` | TUI source references. |
| create/modify | `crates/zedflow-tui/src/**/*.rs` | Rust targets. |
| modify | `crates/zedflow-tui/src/lib.rs` | Module exposure. |
| modify | `crates/zedflow-tui/Cargo.toml` | Required dependencies. |

Required context package:
- Required skills: read `/home/zedium/workspaces/zedflow/.agents/skills/rust-skills/SKILL.md` before editing.
- Required files/symbols to read: assigned source and imports.

Implementation outline:
1. Port terminal UI primitives and components to Rust equivalents.
2. Use placeholders for terminal backend behavior without selected Rust library.
3. Document all public UI types/functions.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; `cargo check -p zedflow-tui`
- Must NOT run: global workaround fixes

Output contract:
- Same as P1.T1, adapted to package `tui`.

Acceptance criteria:
- Assigned TUI file is represented and documented.

Handoff to dependent units:
- P3.T2 tests components; P5.T1 coding-agent may depend on tui APIs.

Subagent prompt:
```text
You are a fresh subagent implementing one row of P3.T1 from .agents/plans/pi-to-rust-package-port.md.
First read /home/zedium/workspaces/zedflow/.agents/skills/rust-skills/SKILL.md and apply it strictly.
Read .agents/port-manifests/tui-src.tsv and use only your assigned row.
Source: <ROW_SOURCE>
Target: <ROW_TARGET>
Port exactly this Pi TUI source file to Rust. Preserve behavior and public semantics. Document every public item. Use PORT PLACEHOLDER for unresolved terminal/backend dependencies. Do not add Zedflow behavior.
```

<a id="P3.T2"></a>
### Task P3.T2 — Port `packages/tui` tests

Assignable: yes

Execution metadata:
- Wave: W4
- Context: fresh per manifest row
- Depends on: P3.T1
- Can run in parallel with: other P3.T2 rows that do not write the same target file
- Must not run in parallel with: same-target rows

Files:
| Action | Path | Purpose |
|---|---|---|
| read | `.agents/port-manifests/tui-tests.tsv` | Exact test scopes. |
| read | `references/pi/packages/tui/test/**/*.ts` | Test references. |
| modify | `crates/zedflow-tui/src/**/*.rs` | Co-located unit tests. |
| create/modify | `crates/zedflow-tui/tests/**/*.rs` | Integration tests if required. |

Required context package:
- Required skills: read `/home/zedium/workspaces/zedflow/.agents/skills/rust-skills/SKILL.md` before editing.
- Required files/symbols to read: assigned test and Rust source under test.

Implementation outline:
1. Port rendering/width/keybinding assertions deterministically.
2. Avoid terminal side effects in unit tests.
3. Mark environment-dependent tests ignored with reason.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; targeted `cargo test -p zedflow-tui <test_name>` when possible
- Must NOT run: global workaround fixes

Output contract:
- Same as P1.T2, adapted to package `tui`.

Acceptance criteria:
- Assigned TUI test is represented in Rust.

Handoff to dependent units:
- P6.T1 validates full workspace.

Subagent prompt:
```text
You are a fresh subagent implementing one row of P3.T2 from .agents/plans/pi-to-rust-package-port.md.
First read /home/zedium/workspaces/zedflow/.agents/skills/rust-skills/SKILL.md and apply it strictly.
Read .agents/port-manifests/tui-tests.tsv and use only your assigned row.
Source test: <ROW_SOURCE>
Preferred target: <ROW_TARGET>
Port exactly this Pi TUI test to Rust. Prefer co-located #[cfg(test)] tests. Preserve assertions and edge cases. Do not implement neighboring source files.
```

<a id="P4.T1"></a>
### Task P4.T1 — Port `packages/orchestrator` source files

Assignable: yes

Execution metadata:
- Wave: W5
- Context: fresh per manifest row
- Depends on: P0.T1
- Can run in parallel with: other P4.T1 rows that do not write the same target file
- Must not run in parallel with: none

Files:
| Action | Path | Purpose |
|---|---|---|
| read | `.agents/port-manifests/orchestrator-src.tsv` | Exact source scopes. |
| read | `references/pi/packages/orchestrator/src/**/*.ts` | Source references. |
| create/modify | `crates/zedflow-orchestrator/src/**/*.rs` | Rust targets. |
| modify | `crates/zedflow-orchestrator/src/lib.rs` | Module exposure. |
| modify | `crates/zedflow-orchestrator/Cargo.toml` | Required dependencies. |

Required context package:
- Required skills: read `/home/zedium/workspaces/zedflow/.agents/skills/rust-skills/SKILL.md` before editing.
- Required files/symbols to read: assigned source and imports.

Implementation outline:
1. Port IPC/config/supervisor/storage behavior from Pi.
2. Keep it Pi-identical; do not add LangGraph/Zedflow flow semantics.
3. Document public APIs.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; `cargo check -p zedflow-orchestrator`
- Must NOT run: global workaround fixes

Output contract:
- Same as P1.T1, adapted to package `orchestrator`.

Acceptance criteria:
- Assigned orchestrator file is represented and documented.

Handoff to dependent units:
- P5.T1 coding-agent can depend on orchestrator APIs.

Subagent prompt:
```text
You are a fresh subagent implementing one row of P4.T1 from .agents/plans/pi-to-rust-package-port.md.
First read /home/zedium/workspaces/zedflow/.agents/skills/rust-skills/SKILL.md and apply it strictly.
Read .agents/port-manifests/orchestrator-src.tsv and use only your assigned row.
Source: <ROW_SOURCE>
Target: <ROW_TARGET>
Port exactly this Pi orchestrator source file to Rust. Preserve Pi behavior. Document every public item. Use PORT PLACEHOLDER for unresolved external dependencies. Do not add Zedflow flow or LangGraph behavior.
```

<a id="P5.T1"></a>
### Task P5.T1 — Port `packages/coding-agent` source files

Assignable: yes

Execution metadata:
- Wave: W6
- Context: fresh per manifest row
- Depends on: P0.T1, P1.T1, P2.T1, P3.T1, P4.T1
- Can run in parallel with: other P5.T1 rows that do not write the same target file
- Must not run in parallel with: P5.T2 rows targeting the same module

Files:
| Action | Path | Purpose |
|---|---|---|
| read | `.agents/port-manifests/coding-agent-src.tsv` | Exact source scopes. |
| read | `references/pi/packages/coding-agent/src/**/*.ts` | Source references. |
| create/modify | `crates/zedflow-coding-agent/src/**/*.rs` | Rust targets. |
| modify | `crates/zedflow-coding-agent/src/lib.rs` | Module exposure. |
| modify | `crates/zedflow-coding-agent/Cargo.toml` | Required dependencies. |

Required context package:
- Required skills: read `/home/zedium/workspaces/zedflow/.agents/skills/rust-skills/SKILL.md` before editing.
- Required files/symbols to read: assigned source, imported Pi files, dependency crate APIs.
- Dependency outputs to read: relevant ai/agent/tui/orchestrator source outputs.

Implementation outline:
1. Port CLI/config/utils/package assembly behavior from Pi.
2. Use placeholders for OS/platform APIs lacking selected Rust equivalents.
3. Document all public items.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; `cargo check -p zedflow-coding-agent`
- Must NOT run: global workaround fixes; live external commands unless the source file owns that behavior and uses temp dirs

Output contract:
- Same as P1.T1, adapted to package `coding-agent`.

Acceptance criteria:
- Assigned coding-agent file is represented and documented.

Handoff to dependent units:
- P5.T2 ports tests; P6.T1 validates workspace.

Subagent prompt:
```text
You are a fresh subagent implementing one row of P5.T1 from .agents/plans/pi-to-rust-package-port.md.
First read /home/zedium/workspaces/zedflow/.agents/skills/rust-skills/SKILL.md and apply it strictly.
Read .agents/port-manifests/coding-agent-src.tsv and use only your assigned row.
Source: <ROW_SOURCE>
Target: <ROW_TARGET>
Port exactly this Pi coding-agent source file to Rust. Preserve Pi behavior. Document every public item. Use PORT PLACEHOLDER for unresolved external dependencies. Do not add Zedflow behavior or edit neighboring scopes.
```

<a id="P5.T2"></a>
### Task P5.T2 — Port `packages/coding-agent` tests

Assignable: yes

Execution metadata:
- Wave: W6
- Context: fresh per manifest row
- Depends on: P5.T1
- Can run in parallel with: other P5.T2 rows that do not write the same target file
- Must not run in parallel with: same-target rows

Files:
| Action | Path | Purpose |
|---|---|---|
| read | `.agents/port-manifests/coding-agent-tests.tsv` | Exact test scopes. |
| read | `references/pi/packages/coding-agent/test/**/*.ts` | Test references. |
| modify | `crates/zedflow-coding-agent/src/**/*.rs` | Co-located unit tests. |
| create/modify | `crates/zedflow-coding-agent/tests/**/*.rs` | Integration tests if required. |

Required context package:
- Required skills: read `/home/zedium/workspaces/zedflow/.agents/skills/rust-skills/SKILL.md` before editing.
- Required files/symbols to read: assigned test and Rust source under test.
- Dependency outputs to read: relevant P5.T1 row output.

Implementation outline:
1. Port assertions and fixtures from Pi tests.
2. Use temp dirs and deterministic fake IO where Pi tests do.
3. Use ignored tests for live/external dependencies with explicit reason.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; targeted `cargo test -p zedflow-coding-agent <test_name>` when possible
- Must NOT run: global workaround fixes; live provider calls

Output contract:
- Same as P1.T2, adapted to package `coding-agent`.

Acceptance criteria:
- Assigned coding-agent test behavior is represented in Rust.

Handoff to dependent units:
- P6.T1 validates workspace.

Subagent prompt:
```text
You are a fresh subagent implementing one row of P5.T2 from .agents/plans/pi-to-rust-package-port.md.
First read /home/zedium/workspaces/zedflow/.agents/skills/rust-skills/SKILL.md and apply it strictly.
Read .agents/port-manifests/coding-agent-tests.tsv and use only your assigned row.
Source test: <ROW_SOURCE>
Preferred target: <ROW_TARGET>
Port exactly this Pi coding-agent test to Rust. Prefer co-located #[cfg(test)] tests unless the behavior is integration-level. Preserve assertions. Use #[ignore] only for documented parity blockers.
```

<a id="P6.T1"></a>
### Task P6.T1 — Final workspace integration validation

Assignable: yes

Execution metadata:
- Wave: W7
- Context: fresh
- Depends on: P1.T1, P1.T2, P2.T1, P2.T2, P3.T1, P3.T2, P4.T1, P5.T1, P5.T2
- Can run in parallel with: none
- Must not run in parallel with: implementation chains

Scope boundaries:
- Goal: Validate the full port and report remaining placeholders/parity gaps.
- Non-goals: Do not implement missing files except trivial module-list fixes required by completed units.
- Forbidden work: No broad compatibility shims, no old monolith re-exports, no Zedflow behavior.

Files:
| Action | Path | Purpose |
|---|---|---|
| read | `.agents/port-manifests/*.tsv` | Confirm all rows handled. |
| read | `crates/**` | Validate workspace state. |
| create | `docs/planning/PI_RUST_PORT_STATUS.md` | Final status report with placeholders and ignored tests. |

Required context package:
- Required skills: read `/home/zedium/workspaces/zedflow/.agents/skills/rust-skills/SKILL.md` before editing.
- Required files/symbols to read: all package outputs, manifests, Cargo workspace.
- Dependency outputs to read: all package source/test task outputs.

Implementation outline:
1. Verify each manifest row has a corresponding Rust target or documented blocker.
2. Grep for `PORT PLACEHOLDER` and ignored tests; summarize them.
3. Run final validation gates.
4. Write status report.

Validation responsibility:
- Type: integration-validating
- Must run: `cargo fmt --all --check`; `cargo check --workspace --all-targets`; `cargo test --workspace --all-targets --no-run`; `cargo doc --workspace --no-deps`
- Must NOT run: live provider/network tests
- Expected temporary breakage: none; report if any remains
- Forbidden fixes/workarounds: broad compatibility patches

Output contract:
- Validation commands and results.
- Placeholder inventory.
- Ignored test inventory.
- Unresolved parity blockers.

Acceptance criteria:
- Final validation gates pass or every failure is mapped to a specific incomplete manifest row/blocker.

Handoff to dependent units:
- None; this closes the port planning run.

Subagent prompt:
```text
You are implementing only P6.T1 from .agents/plans/pi-to-rust-package-port.md.
Run in fresh context. First read /home/zedium/workspaces/zedflow/.agents/skills/rust-skills/SKILL.md and apply it strictly.
Read all manifests in .agents/port-manifests and dependency outputs from package chains.
Validate the full workspace, inventory PORT PLACEHOLDER markers and ignored tests, and write docs/planning/PI_RUST_PORT_STATUS.md. Do not implement missing package files except trivial module-list fixes required by already completed units. Do not add Zedflow behavior or old monolith compatibility shims.
```

<a id="implementation-progress"></a>
## Implementation Progress

Last updated: 2026-07-07.

### Orchestration status

| Unit | Status | Notes |
|---|---|---|
| P0.T1 | done | Shared porting infrastructure and `docs/planning/PI_RUST_PORTING_RULES.md` exist. |
| P1.T1 | in progress | `ai-src.tsv`: 112/148 Rust targets exist. Current row cursor is `src/providers/xiaomi-token-plan-cn.models.ts` → `crates/zedflow-ai/src/providers/xiaomi-token-plan-cn.models.rs`. |
| P1.T2 | not started | Blocked until P1.T1 source chain is complete. |
| P2.T1 | not started | Blocked on stable `zedflow-ai` APIs. |
| P2.T2 | not started | Blocked until P2.T1 source chain is complete. |
| P3.T1 | not started | No TUI source rows started. |
| P3.T2 | not started | Blocked until P3.T1 source chain is complete. |
| P4.T1 | not started | No orchestrator source rows started. |
| P5.T1 | not started | Blocked on agent/orchestrator/tui source chains. |
| P5.T2 | not started | Blocked until P5.T1 source chain is complete. |
| P6.T1 | not started | Final validation only after W2-W6 complete. |

### P1.T1 rows completed in this orchestration session

| Source | Target | Validation |
|---|---|---|
| `src/providers/anthropic.models.ts` | `crates/zedflow-ai/src/providers/anthropic.models.rs` | `cargo fmt --all --check`; `cargo check -p zedflow-ai` pass. |
| `src/providers/anthropic.ts` | `crates/zedflow-ai/src/providers/anthropic.rs` | `cargo fmt --all --check`; `cargo check -p zedflow-ai` pass. |
| `src/providers/azure-openai-responses.models.ts` | `crates/zedflow-ai/src/providers/azure-openai-responses.models.rs` | `cargo fmt --all --check`; `cargo check -p zedflow-ai` pass. |
| `src/providers/azure-openai-responses.ts` | `crates/zedflow-ai/src/providers/azure-openai-responses.rs` | `cargo fmt --all --check`; `cargo check -p zedflow-ai` pass. |
| `src/providers/kimi-coding.ts` | `crates/zedflow-ai/src/providers/kimi-coding.rs` | `cargo fmt --all`; `cargo check -p zedflow-ai` pass. |
| `src/providers/minimax-cn.models.ts` | `crates/zedflow-ai/src/providers/minimax-cn.models.rs` | `cargo fmt --all`; `cargo check -p zedflow-ai` pass. |
| `src/providers/minimax-cn.ts` | `crates/zedflow-ai/src/providers/minimax-cn.rs` | `cargo fmt --all`; `cargo check -p zedflow-ai` pass. |
| `src/providers/minimax.models.ts` | `crates/zedflow-ai/src/providers/minimax.models.rs` | `cargo fmt --all`; `cargo check -p zedflow-ai` pass. |
| `src/providers/minimax.ts` | `crates/zedflow-ai/src/providers/minimax.rs` | `cargo fmt --all`; `cargo check -p zedflow-ai` pass. |
| `src/providers/mistral.models.ts` | `crates/zedflow-ai/src/providers/mistral.models.rs` | `cargo fmt --all`; `cargo check -p zedflow-ai` pass. |
| `src/providers/mistral.ts` | `crates/zedflow-ai/src/providers/mistral.rs` | `cargo fmt --all`; `cargo check -p zedflow-ai` pass. |
| `src/providers/moonshotai-cn.models.ts` | `crates/zedflow-ai/src/providers/moonshotai-cn.models.rs` | `cargo fmt --all`; `cargo check -p zedflow-ai` pass. |
| `src/providers/moonshotai-cn.ts` | `crates/zedflow-ai/src/providers/moonshotai-cn.rs` | `cargo fmt --all`; `cargo check -p zedflow-ai` pass. |
| `src/providers/moonshotai.models.ts` | `crates/zedflow-ai/src/providers/moonshotai.models.rs` | `cargo fmt --all`; `cargo check -p zedflow-ai` pass. |
| `src/providers/moonshotai.ts` | `crates/zedflow-ai/src/providers/moonshotai.rs` | `cargo fmt --all`; `cargo check -p zedflow-ai` pass. |
| `src/providers/nvidia.models.ts` | `crates/zedflow-ai/src/providers/nvidia.models.rs` | `cargo fmt --all`; `cargo check -p zedflow-ai` pass. |
| `src/providers/nvidia.ts` | `crates/zedflow-ai/src/providers/nvidia.rs` | `cargo fmt --all`; `cargo check -p zedflow-ai` pass. |
| `src/providers/openai-codex.models.ts` | `crates/zedflow-ai/src/providers/openai-codex.models.rs` | `cargo fmt --all`; `cargo check -p zedflow-ai` pass. |
| `src/providers/openai-codex.ts` | `crates/zedflow-ai/src/providers/openai-codex.rs` | `cargo fmt --all`; `cargo check -p zedflow-ai` pass. |
| `src/providers/openai.models.ts` | `crates/zedflow-ai/src/providers/openai.models.rs` | `cargo fmt --all`; `cargo check -p zedflow-ai` pass. |
| `src/providers/openai.ts` | `crates/zedflow-ai/src/providers/openai.rs` | `cargo fmt --all`; `cargo check -p zedflow-ai` pass. |
| `src/providers/opencode-go.models.ts` | `crates/zedflow-ai/src/providers/opencode-go.models.rs` | `cargo fmt --all`; `cargo check -p zedflow-ai` pass. |
| `src/providers/opencode-go.ts` | `crates/zedflow-ai/src/providers/opencode-go.rs` | `cargo fmt --all`; `cargo check -p zedflow-ai` pass. |
| `src/providers/opencode.models.ts` | `crates/zedflow-ai/src/providers/opencode.models.rs` | `cargo fmt --all`; `cargo check -p zedflow-ai` pass. |
| `src/providers/opencode.ts` | `crates/zedflow-ai/src/providers/opencode.rs` | `cargo fmt --all`; `cargo check -p zedflow-ai` pass. |
| `src/providers/openrouter-images.ts` | `crates/zedflow-ai/src/providers/openrouter-images.rs` | `cargo fmt --all`; `cargo check -p zedflow-ai` pass. |
| `src/providers/openrouter.models.ts` | `crates/zedflow-ai/src/providers/openrouter.models.rs` | `cargo fmt --all`; `cargo check -p zedflow-ai` pass. |
| `src/providers/openrouter.ts` | `crates/zedflow-ai/src/providers/openrouter.rs` | `cargo fmt --all`; `cargo check -p zedflow-ai` pass. |
| `src/providers/together.models.ts` | `crates/zedflow-ai/src/providers/together.models.rs` | `cargo fmt --all`; `cargo check -p zedflow-ai` pass. |
| `src/providers/together.ts` | `crates/zedflow-ai/src/providers/together.rs` | `cargo fmt --all`; `cargo check -p zedflow-ai` pass. |
| `src/providers/vercel-ai-gateway.models.ts` | `crates/zedflow-ai/src/providers/vercel-ai-gateway.models.rs` | `cargo fmt --all`; `cargo check -p zedflow-ai` pass. |
| `src/providers/vercel-ai-gateway.ts` | `crates/zedflow-ai/src/providers/vercel-ai-gateway.rs` | `cargo fmt --all`; `cargo check -p zedflow-ai` pass. |
| `src/providers/xai.models.ts` | `crates/zedflow-ai/src/providers/xai.models.rs` | `cargo fmt --all`; `cargo check -p zedflow-ai` pass. |
| `src/providers/xai.ts` | `crates/zedflow-ai/src/providers/xai.rs` | `cargo fmt --all`; `cargo check -p zedflow-ai` pass. |
| `src/providers/xiaomi-token-plan-ams.models.ts` | `crates/zedflow-ai/src/providers/xiaomi-token-plan-ams.models.rs` | `cargo fmt --all`; `cargo check -p zedflow-ai` pass. |
| `src/providers/xiaomi-token-plan-ams.ts` | `crates/zedflow-ai/src/providers/xiaomi-token-plan-ams.rs` | `cargo fmt --all`; `cargo check -p zedflow-ai` pass. |

Notes:
- Subagents for the four rows above exceeded turn budgets after writing useful artifacts; the parent completed formatting/routing and reran validation.
- Same-target writes remain sequential because provider rows update `crates/zedflow-ai/src/lib.rs`.
- Keep using small one-row handoffs; broad fanout is currently not worth the merge/conflict overhead.
- Latest parent validation after subagent batches: `cargo fmt --all` and `cargo check -p zedflow-ai` pass.
- Latest parallel batches advanced P1.T1 to 112/148 targets. Remaining P1.T1 rows start at `src/providers/xiaomi-token-plan-cn.models.ts`.

<a id="pre-finalization-review"></a>
## Pre-finalization Review Summary

| Reviewer | Status | Required changes applied | Remaining concerns |
|---|---|---|---|
| Feasibility / file references | Pass | Generated package manifests from `references/pi/packages`; attached exact manifest paths to tasks. | Per-row conflicts must be handled by orchestrator sequencing. |
| Sequencing / dependency graph | Pass | Ordered ai before agent/coding-agent; tests after source chains; final validation last. | Large chains require careful progress tracking outside this plan. |
| Scope isolation / prompt quality | Pass | Every prompt requires fresh context, exact row scope, rust-skills, and no Zedflow behavior. | None. |
