# Pi Stage-1 Port Recovery

<a id="how-to-use"></a>
## How to use this plan

This plan is self-contained for orchestration by fresh agent sessions.

- Execute only units marked `Assignable: yes` and in wave order.
- Every implementation subagent runs with fresh context and the exact files listed in its unit.
- One product-code writer is active at a time. Read-only reviews may run in parallel.
- Ordinary implementation failures enter the repair loop; they do not mutate the DAG.
- A dependency replacement or unavoidable non-1:1 mapping stops with `ARBITRATION_REQUIRED`.
- Stage-2 Flow/LangGraph implementation remains forbidden.

<a id="legend"></a>
## Legend

- `fresh`: no inherited conversation context.
- `sequential`: starts only after dependencies are accepted.
- `Assignable: yes`: bounded implementation unit.
- `locally-validating`: validates owned scope.
- `integration-validating`: owns workspace-wide gates.
- `[CANONICAL]`: required shape.
- `BQ`: blocking human decision.
- `R`: implementation risk.

<a id="goal"></a>
## Goal

Restore a single auditable Stage-1 control plane, reduce the workspace to one Rust crate per frozen Pi package, enforce manifest closure and explicit dependency arbitration, recover the blocked TUI work, and resume the automatic port through TUI, Coding-agent, Orchestrator, and the final Stage-1 gate.

<a id="non-goals"></a>
## Non-goals

- No Zedflow Flow, Runtime Graph, sidecar, or LangGraph implementation.
- No speculative replacement for unresolved npm/native dependencies.
- No wholesale rewrite of accepted AI, Agent, or Coding-agent port code.
- No direct checkout of the CAS-managed `automation/pi-port` ref.
- No cleanup of historical failed worktrees until the cleanup command has passed dry-run review.

<a id="review-flags"></a>
## Review Flags

| ID | Type | Severity | Summary | Affects | Follow-up / accepted risk |
|---|---|---:|---|---|---|
| RF-1 | R | High | `main` contains newer controller fixes while `automation/pi-port` contains port progress. | U1 | Merge control-plane history into the recovery branch, preserving the integration DAG and port sources. |
| RF-2 | R | High | Existing manifests measure target presence but do not disposition consolidations or dependency substitutions. | U3, U5-U7 | Add deterministic exception/arbitration records before package closure. |
| RF-3 | R | Medium | Historical runtime retains hundreds of worktrees and refs. | U4 | Implement safe cleanup first; apply historical cleanup only after explicit review. |

<a id="global-acceptance"></a>
## Global Acceptance Criteria

1. The recovery head contains the current integration sources and the tested latest controller fixes.
2. Controller and recovery tests pass; runtime identity records controller, integration, DAG, plan, and Pi SHAs.
3. The Stage-1 workspace contains exactly `zedflow-ai`, `zedflow-agent`, `zedflow-tui`, `zedflow-coding-agent`, and `zedflow-orchestrator`.
4. Package dependencies match frozen Pi: Coding-agent depends on AI/Agent/TUI; Orchestrator depends on Coding-agent.
5. Every frozen Pi source/test is inventoried and either maps to an existing Rust target or has an explicit approved disposition.
6. Ordinary review/validation blockers enter a bounded repair loop and resume automatically; structural/dependency decisions pause safely.
7. Successful accepted worktrees and unit refs can be cleaned safely after reachability checks.
8. `BASELINE.md` is the only current human status; historical plans/status files are labelled historical.
9. Final Stage-1 completion requires executed workspace tests and independent fidelity/Rust reviews on one immutable SHA.

<a id="legacy-policy"></a>
## Legacy / workaround policy

- No compatibility shim, temporary alias, type weakening, or placeholder used only to make an intermediate compile.
- Default mapping is one Pi package to one Rust crate and one Pi source/test to one Rust source/test.
- Any non-1:1 mapping requires an explicit disposition; dependency replacement requires human arbitration.
- Do not reopen accepted code without a demonstrated fidelity or build defect.

<a id="breaking-changes"></a>
## Planned Breaking Changes and Propagation Map

| Change | Introduced by | Expected temporary breakage | Fixed by | Forbidden workaround |
|---|---|---|---|---|
| Remove four non-Pi crates | U2 | `zedflow_core` imports and stale Cargo dependencies | U2 in the same unit | Reintroducing facade crates or aliases |
| Reverse Coding-agent/Orchestrator dependency | U2 | Cargo metadata and imports | U2 | Circular dependency |
| Replace dynamic repair replans with repair state | U1 | Existing failed TUI reviewer needs migration | U1/U5 | Reusing terminal DAG IDs |
| Enforce manifest closure | U3 | Current partial packages cannot claim closure | U5-U7 | Weakening closure to target counts only |

<a id="orchestration"></a>
## Subagent Orchestration Plan

- Wave 1: U1 sequential.
- Wave 2: U2 sequential after U1.
- Wave 3: U3 and U4 sequential because both modify controller/state surfaces.
- Wave 4: U5 sequential.
- Wave 5: U6 sequential after TUI closure.
- Wave 6: U7 sequential after Coding-agent closure.
- Wave 7: U8 integration validation and Stage-1 checkpoint.
- Read-only fidelity and Rust reviewers may run in parallel after each writer candidate.

<a id="parallelization-constraints"></a>
## Parallelization Constraints

| Constraint | Reason | Affected units |
|---|---|---|
| One writer only | Cargo, DAG, runtime state, and docs share integration state | U1-U8 |
| U2 precedes package continuation | Restores the required package dependency graph | U5-U7 |
| TUI closes before Coding-agent | Frozen Coding-agent imports Pi TUI broadly | U5, U6 |
| Coding-agent closes before Orchestrator | Frozen Orchestrator depends on Coding-agent | U6, U7 |

<a id="canonical-line-references"></a>
## Canonical Line References

<!-- CANONICAL_LINE_REFERENCES_START -->
<!-- This block is generated by finalize-plan-lines.mjs. Do not edit manually. -->
| ID | Anchor | Lines | Description |
|---|---|---|---|
| how-to-use | #how-to-use | L3-L13 | How to use this plan |
| legend | #legend | L15-L25 | Legend |
| goal | #goal | L27-L30 | Goal |
| non-goals | #non-goals | L32-L39 | Non-goals |
| review-flags | #review-flags | L41-L48 | Review Flags |
| global-acceptance | #global-acceptance | L50-L61 | Global Acceptance Criteria |
| legacy-policy | #legacy-policy | L63-L69 | Legacy / workaround policy |
| breaking-changes | #breaking-changes | L71-L79 | Planned Breaking Changes and Propagation Map |
| orchestration | #orchestration | L81-L91 | Subagent Orchestration Plan |
| parallelization-constraints | #parallelization-constraints | L93-L101 | Parallelization Constraints |
| canonical-line-references | #canonical-line-references | L103-L108 | Canonical Line References |
| u1 | #u1 | L110-L147 | U1 — Recover and harden the control plane |
| u2 | #u2 | L149-L174 | U2 — Restore the one-package/one-crate workspace |
| u3 | #u3 | L176-L200 | U3 — Enforce deterministic manifest closure |
| u4 | #u4 | L202-L226 | U4 — Add bounded automatic cleanup |
| u5 | #u5 | L228-L252 | U5 — Reconcile and close TUI |
| u6 | #u6 | L254-L277 | U6 — Reconcile and close Coding-agent |
| u7 | #u7 | L279-L302 | U7 — Port and close Orchestrator |
| u8 | #u8 | L304-L329 | U8 — Final Stage-1 checkpoint and promotion |
| documentation-changes | #documentation-changes | L331-L338 | Documentation changes included across units |
| pre-finalization-review | #pre-finalization-review | L340-L346 | Pre-finalization review summary |
<!-- CANONICAL_LINE_REFERENCES_END -->

<a id="u1"></a>
## U1 — Recover and harden the control plane

**Assignable:** yes  
**Wave:** 1  
**Context:** fresh  
**Dependencies:** none  
**Validation:** integration-validating

### Scope

- Modify `tools/pi-port-swarm/controller.py`.
- Modify `tools/pi-port-swarm/test_controller.py`.
- Create or reconcile `tools/pi-port-swarm/recovery.py` and `test_recovery.py`.
- Modify `.pi/prompts/pi-port-{worker-session,reviewer,validator,coordinator,checkpoint}.md`.
- Modify `.agents/port-swarm/state.json` and `tools/pi-port-swarm/README.md`.
- Read `main`-only controller commits and external runtime state; preserve product sources and the frozen Pi gitlink.

### Requirements

- Use unit refs outside the integration leaf namespace.
- Replace seed `HEAD` with an immutable SHA.
- Persist `controller_sha`, `integration_sha`, `dag_sha`, `plan_sha`, and `pi_gitlink`.
- Classify outcomes as `REPAIRABLE`, `PLAN_CHANGE_REQUIRED`, `ARBITRATION_REQUIRED`, or `TRANSIENT`.
- Route ordinary blockers through bounded repair attempts without DAG mutation.
- Permit structural replans only through a coordinator that explicitly reads `/home/zedium/.agents/skills/plan-writer/SKILL.md` and `REFERENCE.md`.
- Never reuse a terminal unit ID; verify a revised graph leaves a ready/reachable frontier.
- Execute only allow-listed validation command forms and persist command outcome logs.

### Acceptance

- Controller/recovery tests pass.
- Current failed TUI state can be represented as a repairable frontier with fresh IDs.
- No service is restarted by this unit.

### Subagent prompt

Implement U1 only. Preserve `automation/pi-port` sources and frozen Pi gitlink. Reconcile the tested controller fixes from `main`, then add the minimum repair/arbitration/identity invariants above. Do not touch Cargo crates or resume the service. Return changed files, tests, and the exact runtime-state migration required.

<a id="u2"></a>
## U2 — Restore the one-package/one-crate workspace

**Assignable:** yes  
**Wave:** 2  
**Context:** fresh  
**Dependencies:** U1  
**Validation:** integration-validating

### Scope

- Modify root `Cargo.toml` and `Cargo.lock`.
- Modify Cargo manifests for the five retained crates.
- Move `zedflow-core` error behavior into `zedflow-ai/src/error.rs` and update AI imports/tests.
- Delete `crates/zedflow-core`, `crates/zedflow-tools`, `crates/zedflow-session`, and `crates/zedflow-langgraph`.
- Remove Coding-agent → Orchestrator and add Orchestrator → Coding-agent.

### Acceptance

- Workspace contains exactly five Pi package crates.
- `cargo fmt`, workspace check, test build, and executed tests pass.
- No compatibility facade or circular dependency remains.

### Subagent prompt

Implement U2 only. Make the Rust workspace mirror the five frozen Pi packages. Migrate real error behavior into AI, remove unused substrate crates, and correct package dependency direction. Do not change port behavior or Stage-2 code. Run full Cargo gates.

<a id="u3"></a>
## U3 — Enforce deterministic manifest closure

**Assignable:** yes  
**Wave:** 3  
**Context:** fresh  
**Dependencies:** U2  
**Validation:** locally-validating

### Scope

- Create `tools/pi-port-swarm/manifest.py` and `test_manifest.py`.
- Extend `.agents/port-manifests/` with one exception/disposition ledger rather than duplicating current status prose.
- Integrate manifest validation into `controller.py status`, package closure, and final closure.
- Include every frozen `.ts`, `.tsx`, and `.d.ts` source/test according to package policy.

### Acceptance

- Missing, duplicate, consolidated, type-only, platform-specific, live-capability, and dependency-arbitration rows are reported deterministically.
- A package cannot close with an unexplained source/test row.
- Dependency arbitration blocks execution rather than producing a speculative placeholder.

### Subagent prompt

Implement U3 only. Keep existing TSV inventories as the base mapping and add the smallest explicit exception ledger. Make closure executable and tested. Do not port missing product files in this unit.

<a id="u4"></a>
## U4 — Add bounded automatic cleanup

**Assignable:** yes  
**Wave:** 3  
**Context:** fresh  
**Dependencies:** U3  
**Validation:** locally-validating

### Scope

- Modify `controller.py` and `test_controller.py`.
- Add `cleanup --dry-run` and `cleanup --accepted`.
- Update `tools/pi-port-swarm/README.md`.

### Acceptance

- Accepted worktrees are removable only when their candidate is reachable from integration and logs/state are durable.
- Failed/blocked current attempts are retained.
- Unit refs are deleted only after the same reachability check.
- No historical cleanup is applied by this unit.

### Subagent prompt

Implement U4 only. Add safe, test-covered cleanup with dry-run default. Never perform broad filesystem removal and do not clean the existing historical inventory during implementation.

<a id="u5"></a>
## U5 — Reconcile and close TUI

**Assignable:** yes  
**Wave:** 4  
**Context:** fresh  
**Dependencies:** U4  
**Validation:** integration-validating

### Scope

- Repair the known Kitty CSI-u Unicode/Shift fidelity defect.
- Reconcile all TUI source/test manifest rows.
- Port missing TUI behavior one-to-one or emit `ARBITRATION_REQUIRED`.
- Use fresh validator and fidelity/Rust reviewers.

### Acceptance

- TUI manifest closes with no unexplained row.
- Focused and package tests execute successfully.
- Reviews accept the same SHA.

### Subagent prompt

Execute U5 through the recovered controller. Start from the known keys blocker, then close every frozen TUI source/test row. Prefer file-for-file ports; stop for dependency arbitration instead of inventing substitutes.

<a id="u6"></a>
## U6 — Reconcile and close Coding-agent

**Assignable:** yes  
**Wave:** 5  
**Context:** fresh  
**Dependencies:** U5  
**Validation:** integration-validating

### Scope

- Preserve accepted Coding-agent implementations.
- Reconcile missing source/test rows after TUI closure.
- Port only proven residual rows one-to-one.

### Acceptance

- Coding-agent manifest closes with no unexplained row.
- CLI/core/session/tool/TUI integration behavior passes deterministic tests.
- Reviews accept the same SHA.

### Subagent prompt

Execute U6 through the recovered controller. Audit mappings before writing, keep accepted code, and port only real residual Coding-agent rows. Do not use Orchestrator as a dependency.

<a id="u7"></a>
## U7 — Port and close Orchestrator

**Assignable:** yes  
**Wave:** 6  
**Context:** fresh  
**Dependencies:** U6  
**Validation:** integration-validating

### Scope

- Port the 13 frozen Orchestrator sources into `zedflow-orchestrator`.
- Depend on Coding-agent as Pi does.
- Add deterministic Rust tests for source behavior where Pi has no dedicated test package.

### Acceptance

- Orchestrator source manifest closes.
- Package/workspace tests pass.
- Reviews accept the same SHA.

### Subagent prompt

Implement U7 only. Port frozen Pi Orchestrator file-for-file after Coding-agent closure. Keep its dependency direction identical to Pi and add the smallest deterministic tests needed to prove behavior.

<a id="u8"></a>
## U8 — Final Stage-1 checkpoint and promotion

**Assignable:** yes  
**Wave:** 7  
**Context:** fresh  
**Dependencies:** U7  
**Validation:** integration-validating

### Scope

- Run global manifest closure.
- Execute full Cargo format/check/tests, not `--no-run` only.
- Classify every ignored test and placeholder.
- Run independent fidelity and Rust reviews on the same SHA.
- Update `docs/porting/BASELINE.md` and create the Stage-1 attestation.
- Promote the accepted integration SHA to `main`, then rerun the final gate.

### Acceptance

- Every Stage-1 exit criterion is proven on one immutable SHA and its promoted SHA.
- Stage 2 remains unopened until this unit completes.

### Subagent prompt

Execute U8 only after all five package closures. Run all executable gates and independent reviews on one SHA, publish the attestation, promote to `main`, and rerun the gate. Do not begin Stage 2.

<a id="documentation-changes"></a>
## Documentation changes included across units

- Delete `docs/planning/ZEDFLOW_WORKSPACE_ARCHITECTURE.md` after moving the five-package mapping into `PI_RUST_PORTING_RULES.md` and removing active links.
- Keep `BASELINE.md` as the sole current human status generated from controller facts.
- Mark the unlabelled AI/Agent consolidation plan and competing current-status files historical.
- Mark `pi-fidelity-decisions/` as append-only audit evidence.
- Leave LangGraph version discussion deferred.

<a id="pre-finalization-review"></a>
## Pre-finalization review summary

- Feasibility: current integration and latest controller histories share a merge base and can be reconciled without discarding accepted port commits.
- Sequencing: TUI must close before Coding-agent because frozen Coding-agent imports Pi TUI broadly; Orchestrator follows Coding-agent.
- Scope isolation: each writer unit is sequential; read-only reviews may parallelize.
- Accepted human decisions: delete non-Pi Stage-1 crates, enforce one-to-one mapping, require dependency arbitration, use `plan-writer`, add automatic cleanup, and retain one current status.
