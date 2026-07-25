<!-- migration-document-status: SUPERSEDED -->
> [!CAUTION]
> **Migration status: SUPERSEDED.** Historical plan only. Use `.agents/plans/zedflow-ai-agent-pi-fidelity-consolidation.md` and `docs/porting/BASELINE.md` for current work.

# Zedflow Agent Pi Agent Port

<a id="how-to-use"></a>
## How to use this plan

This plan is self-contained for orchestration by a fresh agent session.

- All implementation subagents must run in fresh context.
- Execute only assignable unit IDs listed in the orchestration waves.
- Before launching a unit, pass its full `Subagent prompt` plus the relevant plan references from `Canonical Line References`.
- Do not infer requirements from outside this plan and the listed references.
- Do not execute neighboring task scopes.
- If a unit is marked `non-validating`, do not run global validation or add compatibility workarounds to make the repo compile.
- Only units marked `integration-validating` own broader validation gates.
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
- `integration-validating`: owns broader compile/lint/test/integration gates.

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

Port `references/pi/packages/agent` to `crates/zedflow-agent` with Pi-observable behavior preserved, Rust APIs documented, dependency replacements selected up front, and foundations sequenced before leaf modules so row subagents do not duplicate types, schemas, file-system abstractions, or provider contracts.

<a id="non-goals"></a>
## Non-goals

- Do not implement Zedflow Flow, Runtime Graph, LangGraph, coding-agent CLI, or TUI behavior.
- Do not re-port or redesign `zedflow-ai`; consume its public Pi-compatible facade from the completed `zedflow-ai` drift work.
- Do not launch one subagent per manifest row in raw `ls` order.
- Do not add compatibility shims for the old monolithic Rust port.
- Do not preserve JavaScript-only runtime mechanics when Rust has no equivalent; use explicit placeholders or ignored tests with exact reasons.
- Do not run live provider/network/browser tests unless a task explicitly declares a capability-gated live check.

<a id="review-flags"></a>
## Review Flags

| ID | Type | Severity | Summary | Affects | Follow-up / accepted risk |
|---|---|---:|---|---|---|
| RF-AGENT-TYPES-FIRST | R | High | Porting leaf modules before `types.rs`/`harness/types.rs` causes duplicate AgentMessage, Tool, Result, Env, and Session shapes. | A1-A8 | A1 owns canonical public/private type foundations before leaf work. |
| RF-DEPS-REPLACEMENT | R | High | Pi dependencies (`ignore`, `yaml`, `typebox`, Node builtins) need deliberate Rust replacements, not ad-hoc row decisions. | A1-A4 | A0/A1 encode approved replacements: `ignore`, `yaml_serde`, `uuid`, `wait-timeout`, `serde_json`/`jsonschema`. |
| RF-SESSION-FORMAT | R | High | Session tree and JSONL persistence must remain stable for harness, compaction, and future coding-agent package. | A2, AT1 | A2 owns storage/session before harness integration. |
| RF-ASYNC-STREAM | R | Medium | Pi agent loop uses async streams/events; Rust must align with `zedflow-ai` canonical event streams without spawning incompatible stream universes. | A6-A7 | Reuse `zedflow-ai` stream/event contracts and document any synchronous adapter. |
| RF-NODE-ENV | R | Medium | Node process/fs/readline behavior has platform edges such as process-tree kill and shell discovery. | A4, AT3 | Use stdlib + `wait-timeout`; document unsupported process-tree edge cases instead of hiding them. |
| RF-HARNESS-LATE | R | Medium | `agent-harness.ts` integrates almost every subsystem and should not define foundations. | A7 | A7 runs after types/session/messages/templates/skills/compaction/loop. |

<a id="global-acceptance"></a>
## Global Acceptance Criteria

- Every row in `.agents/port-manifests/agent-src.tsv` is represented by a Rust target file or a documented file-level `PORT PLACEHOLDER` explaining the Rust blocker.
- Every row in `.agents/port-manifests/agent-tests.tsv` is represented by Rust tests, or by ignored tests with exact live/JS-only/blocker reasons.
- Approved dependency replacements are used consistently: `zedflow-ai` for Pi AI, `ignore` for gitignore-style skill filtering, `yaml_serde` for frontmatter, `serde_json`/`jsonschema` for TypeBox-like schemas, `uuid` for UUID v4, and `wait-timeout` with `std::process` for shell timeout.
- Public Rust items added by the port have rustdoc; fallible public functions document errors or use error types whose variants make failure modes clear.
- No public `zedflow-agent` API duplicates `zedflow-ai` model/message/stream/tool types when re-exporting or wrapping is sufficient.
- `cargo fmt --all --check`, `cargo check -p zedflow-agent --all-targets`, `cargo test -p zedflow-agent --all-targets --no-run`, and targeted subsystem tests pass or have documented blockers.
- No Zedflow product-specific behavior is introduced.

<a id="legacy-policy"></a>
## Legacy / workaround policy

- No temporary aliases unless explicitly listed as allowed in a task.
- No compatibility shims unless they are the goal of a task.
- No type weakening to satisfy intermediate compilation.
- No preserving legacy names when the planned change is a removal or rename.
- Do not expand scope to fix breakages assigned to later units.
- If blocked, report the breakage and reference the downstream unit responsible.
- The only allowed placeholder is a documented `PORT PLACEHOLDER` with exact missing Rust dependency or JS-only reason.

<a id="breaking-changes"></a>
## Planned Breaking Changes and Propagation Map

| Change | Introduced by | Expected temporary breakage | Fixed by | Forbidden workaround |
|---|---|---|---|---|
| `crates/zedflow-agent` receives canonical type/env/session module layout before leaf ports. | A1 | Leaf manifest targets may not compile until A2-A7 fill referenced modules. | A8 and AV1 | Local duplicate structs in leaf files. |
| Tool schemas are represented as JSON schema values, not Rust TypeBox equivalents. | A1 | Tests expecting typed TS `Static<T>` need serde_json argument assertions. | AT5/AT6 | Adding Rust `typebox` without demonstrated parity need. |
| Node execution environment is a Rust `ExecutionEnv` seam over stdlib + `wait-timeout`. | A4 | Exact process-tree kill semantics may be documented placeholder. | AT3/AV1 | Pulling in broad process supervisor crates before tests require them. |
| Harness integration waits until foundations are complete. | A7 | `agent-harness.rs` remains temporarily absent or placeholdered. | A7 | Implementing harness-specific private copies of session/messages/skills. |

<a id="orchestration"></a>
## Subagent Orchestration Plan

- W0: Run A0 once.
- W1: Run A1 after A0.
- W2: Run A2 and A3 in parallel after A1 if file scopes remain isolated.
- W3: Run A4 after A1; A4 may run in parallel with A2/A3 only if the orchestrator confirms no shared file writes.
- W4: Run A5 after A2 and A3.
- W5: Run A6 after A1, A3, and relevant `zedflow-ai` contracts are available.
- W6: Run A7 after A2-A6.
- W7: Run A8 after A7.
- W8: Run AT1-AT7 test units after their source dependencies complete; test units with disjoint target files may run in parallel.
- W9: Run AV1 final validation after A1-A8 and AT1-AT7 complete.

<a id="parallelization-constraints"></a>
## Parallelization Constraints

| Constraint | Reason | Affected units |
|---|---|---|
| Do not run leaf source row-subagents before A1. | Prevent duplicate foundational types and dependency choices. | A2-A8 |
| Same target file writes are sequential. | Multiple Pi tests map to one Rust integration file or source module. | AT1-AT7 |
| `agent-harness.rs` is late and exclusive. | It integrates all foundations and is high-conflict. | A7 |
| Test units must not add source APIs beyond their subsystem unless blocked and approved. | Avoid tests driving ad-hoc public surface. | AT1-AT7 |
| Only AV1 runs broad package gates. | Earlier units may intentionally leave downstream compile gaps. | A1-A8, AT1-AT7 |

<a id="canonical-line-references"></a>
## Canonical Line References

<!-- CANONICAL_LINE_REFERENCES_START -->
<!-- This block is generated by finalize-plan-lines.mjs. Do not edit manually. -->
| ID | Anchor | Lines | Description |
|---|---|---|---|
| how-to-use | #how-to-use | L3-L15 | How to use this plan |
| legend | #legend | L17-L51 | Legend |
| goal | #goal | L53-L56 | Goal |
| non-goals | #non-goals | L58-L66 | Non-goals |
| review-flags | #review-flags | L68-L78 | Review Flags |
| global-acceptance | #global-acceptance | L80-L89 | Global Acceptance Criteria |
| legacy-policy | #legacy-policy | L91-L100 | Legacy / workaround policy |
| breaking-changes | #breaking-changes | L102-L110 | Planned Breaking Changes and Propagation Map |
| orchestration | #orchestration | L112-L124 | Subagent Orchestration Plan |
| parallelization-constraints | #parallelization-constraints | L126-L135 | Parallelization Constraints |
| canonical-line-references | #canonical-line-references | L137-L174 | Canonical Line References |
| phases-and-tasks | #phases-and-tasks | L176-L1212 | Phases and Tasks |
| A0 | #A0 | L179-L238 | Task A0 — Dependency replacement and ownership map |
| A1 | #A1 | L240-L321 | Task A1 — Canonical types, errors, and dependency scaffold |
| A2 | #A2 | L323-L386 | Task A2 — Session and storage foundations |
| A3 | #A3 | L388-L465 | Task A3 — Messages, templates, skills, and text utilities |
| A4 | #A4 | L467-L542 | Task A4 — Node execution environment and proxy seam |
| A5 | #A5 | L544-L600 | Task A5 — Compaction and branch summarization |
| A6 | #A6 | L602-L660 | Task A6 — Agent loop and agent facade behavior |
| A7 | #A7 | L662-L716 | Task A7 — Agent harness integration |
| A8 | #A8 | L718-L775 | Task A8 — Root facade and module closure |
| AT1 | #AT1 | L777-L834 | Task AT1 — Session/storage tests |
| AT2 | #AT2 | L836-L892 | Task AT2 — Prompt, skill, system-prompt, and utility tests |
| AT3 | #AT3 | L894-L948 | Task AT3 — Environment and proxy tests |
| AT4 | #AT4 | L950-L998 | Task AT4 — Compaction tests |
| AT5 | #AT5 | L1000-L1050 | Task AT5 — Agent-loop and agent tests |
| AT6 | #AT6 | L1052-L1102 | Task AT6 — Agent harness stream/integration tests |
| AT7 | #AT7 | L1104-L1154 | Task AT7 — Scratch/e2e/live samples |
| AV1 | #AV1 | L1156-L1212 | Task AV1 — Final package validation and report |
| pre-finalization-review | #pre-finalization-review | L1214-L1220 | Pre-finalization Review Summary |
<!-- CANONICAL_LINE_REFERENCES_END -->

<a id="phases-and-tasks"></a>
## Phases and Tasks

<a id="A0"></a>
### Task A0 — Dependency replacement and ownership map

Assignable: yes

Execution metadata:
- Wave: W0
- Context: fresh
- Depends on: none
- Can run in parallel with: none
- Must not run in parallel with: A1-A8, AT1-AT7

Scope boundaries:
- Goal: Verify dependency usage and write the source ownership/dependency map that later units must follow.
- Non-goals: Do not edit Rust source files.
- Forbidden work: Do not select new dependencies beyond the approved set unless reporting a blocker.

Files:
| Action | Path | Purpose |
|---|---|---|
| read | `references/pi/packages/agent/package.json` | Approved Pi runtime dependencies. |
| read | `references/pi/packages/agent/src/**/*.ts` | Import graph and dependency usage. |
| read | `.agents/port-manifests/agent-src.tsv` | Source target mapping. |
| read | `.agents/port-manifests/agent-tests.tsv` | Test target mapping. |
| create/modify | `.agents/state/zedflow-agent-port-ownership-map.md` | Ownership map and ordered execution manifest. |
| read | `.agents/state/zedflow-agent-dependency-recon.md` | Prior dependency reconnaissance. |

Required context package:
- Plan references: goal, review flags, orchestration, A0.
- Required skills: `/home/zedium/workspaces/zedflow/.agents/skills/rust-skills/SKILL.md`.
- Required files/symbols to read: package.json, all local imports in `references/pi/packages/agent/src`.
- Required URLs/docs: docs.rs pages for `ignore`, `yaml_serde`, `jsonschema`, `uuid`, `wait-timeout` only if the prior recon is insufficient.
- Dependency outputs to read: none.
- Neighboring out-of-scope units: all implementation tasks.

Implementation outline:
1. Confirm the approved dependency replacements and note any contradiction with current code reality.
2. Produce a topological ownership map: foundational types, session/storage, messages/templates/skills, env/proxy, compaction, loop, harness, facade, tests.
3. Group test manifest rows by subsystem instead of raw order.

Validation responsibility:
- Type: non-validating
- Must run: no cargo commands; optional grep/read scripts only.
- Must NOT run: cargo check/test; source edits.

Output contract:
- Ordered source groups with exact Pi source and Rust target files.
- Test groups with exact Pi tests and Rust targets.
- Any dependency replacement blocker.

Acceptance criteria:
- Later units can identify which files they own without redoing dependency discovery.

Handoff to dependent units:
- A1-A8 and AT1-AT7 must read `.agents/state/zedflow-agent-port-ownership-map.md`.

Subagent prompt:
```text
You are implementing only A0 from .agents/plans/zedflow-agent-pi-agent-port.md. Fresh context. Read the plan sections goal/review-flags/orchestration/A0, rust-skills, references/pi/packages/agent/package.json, .agents/port-manifests/agent-src.tsv, .agents/port-manifests/agent-tests.tsv, .agents/state/zedflow-agent-dependency-recon.md, and inspect imports under references/pi/packages/agent/src. Do not edit Rust source. Write .agents/state/zedflow-agent-port-ownership-map.md with ordered implementation groups, exact file ownership, test groups, and dependency replacement blockers. Do not run cargo.
```

<a id="A1"></a>
### Task A1 — Canonical types, errors, and dependency scaffold

Assignable: yes

Execution metadata:
- Wave: W1
- Context: fresh
- Depends on: A0
- Can run in parallel with: none
- Must not run in parallel with: A2-A8, AT1-AT7

Scope boundaries:
- Goal: Establish `zedflow-agent` module layout, approved dependencies, canonical type/error/env contracts, and Pi AI type reuse.
- Non-goals: Do not implement session storage, skills traversal, prompt loading, compaction, or loop behavior.
- Forbidden work: Do not duplicate `zedflow-ai` message/model/stream/tool types.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `crates/zedflow-agent/Cargo.toml` | Add approved dependency replacements. |
| modify | `crates/zedflow-agent/src/lib.rs` | Module declarations and crate-level docs. |
| create/modify | `crates/zedflow-agent/src/index.rs` | Root facade skeleton. |
| create/modify | `crates/zedflow-agent/src/types.rs` | Agent loop/tool/context types. |
| create/modify | `crates/zedflow-agent/src/harness/types.rs` | Result helpers, errors, env/session/skill/template/harness contracts. |
| read | `references/pi/packages/agent/src/types.ts` | Canonical root agent types. |
| read | `references/pi/packages/agent/src/harness/types.ts` | Canonical harness contracts. |
| read | `crates/zedflow-ai/src/index.rs` | Public Pi-compatible AI facade. |

Required context package:
- Plan references: goal, RF-AGENT-TYPES-FIRST, RF-DEPS-REPLACEMENT, breaking changes, A1.
- Required skills: rust-skills.
- Required files/symbols to read: A0 output; Pi `types.ts`; Pi `harness/types.ts`; zedflow-ai public facade.
- Dependency outputs to read: `.agents/state/zedflow-agent-port-ownership-map.md`.
- Neighboring out-of-scope units: A2-A8 implementation logic.

Implementation outline:
1. Add approved dependencies: `serde`, `serde_json`, `jsonschema`, `ignore`, `yaml_serde`, `uuid` with `v4`/`serde`, and `wait-timeout`.
2. Define module tree matching Pi package layout.
3. Re-export or wrap `zedflow-ai` public types rather than duplicating them.
4. Define `ToolSchema = serde_json::Value`, validation error types, file/execution/session/compaction error types, and public env traits.
5. Add rustdoc to public items; keep implementation bodies minimal where later tasks own behavior.

Major snippets:

#### [CANONICAL] Approved dependency scaffold
```toml
serde = { version = "1", features = ["derive"] }
serde_json = "1"
jsonschema = { version = "0.42", default-features = false }
ignore = "0.4"
yaml_serde = "0.9"
uuid = { version = "1", features = ["v4", "serde"] }
wait-timeout = "0.2"
```

#### [CANONICAL] TypeBox replacement
```rust
pub type ToolSchema = serde_json::Value;
```

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; `cargo check -p zedflow-agent`.
- Must NOT run: package tests or workspace gates.

Output contract:
- List dependency additions.
- List public type/error contracts created.
- List intentionally deferred behavior for A2-A8.

Acceptance criteria:
- `zedflow-agent` has a stable type foundation and compiles as far as owned stubs allow.
- No duplicate `zedflow-ai` runtime type universe is introduced.

Handoff to dependent units:
- A2-A8 must use these types and may extend only within their file scope.

Subagent prompt:
```text
You are implementing only A1 from .agents/plans/zedflow-agent-pi-agent-port.md. Fresh context. Read A1, A0 output, rust-skills, references/pi/packages/agent/src/types.ts, references/pi/packages/agent/src/harness/types.ts, crates/zedflow-ai/src/index.rs, and current crates/zedflow-agent files. Add the approved dependency scaffold, module layout, canonical agent/harness type and error contracts, and reuse zedflow-ai public types. Do not implement session, skills, env, compaction, loop, or harness behavior. Run fmt and cargo check -p zedflow-agent only.
```

<a id="A2"></a>
### Task A2 — Session and storage foundations

Assignable: yes

Execution metadata:
- Wave: W2
- Context: fresh
- Depends on: A1
- Can run in parallel with: A3
- Must not run in parallel with: AT1 or any task editing `harness/session/*`

Scope boundaries:
- Goal: Port Pi session tree, in-memory storage, JSONL storage/repo, UUID helper, and context reconstruction.
- Non-goals: Do not implement compaction algorithms beyond session entry support.
- Forbidden work: Do not alter A1 public types outside narrow missing fields with handoff note.

Files:
| Action | Path | Purpose |
|---|---|---|
| create/modify | `crates/zedflow-agent/src/harness/session/uuid.rs` | UUID v4 helper. |
| create/modify | `crates/zedflow-agent/src/harness/session/session.rs` | Session tree and context reconstruction. |
| create/modify | `crates/zedflow-agent/src/harness/session/memory-storage.rs` | In-memory session storage. |
| create/modify | `crates/zedflow-agent/src/harness/session/memory-repo.rs` | In-memory session repo. |
| create/modify | `crates/zedflow-agent/src/harness/session/repo-utils.rs` | Repo helper functions. |
| create/modify | `crates/zedflow-agent/src/harness/session/jsonl-storage.rs` | JSONL storage. |
| create/modify | `crates/zedflow-agent/src/harness/session/jsonl-repo.rs` | JSONL repo. |
| read | `references/pi/packages/agent/src/harness/session/*.ts` | Canonical behavior. |
| read | `references/pi/packages/agent/src/harness/messages.ts` | Message constructors used during context reconstruction. |

Required context package:
- Plan references: RF-SESSION-FORMAT, A2.
- Required skills: rust-skills.
- Dependency outputs to read: A0 output and A1 output if present.
- Required files/symbols to read: all Pi session files.
- Neighboring out-of-scope units: A3 message constructors, A5 compaction behavior, AT1 tests.

Implementation outline:
1. Port session entry enums and metadata persistence with serde.
2. Implement `build_session_context` using A3 message constructors if available; otherwise add minimal calls with handoff notes.
3. Implement memory storage/repo with deterministic ordering.
4. Implement JSONL storage with line-by-line parse diagnostics matching Pi behavior.
5. Use `uuid::Uuid::new_v4()` for UUIDs.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; `cargo check -p zedflow-agent`.
- Must NOT run: session tests until AT1.

Output contract:
- List session files implemented.
- List JSONL format assumptions.
- List blockers for AT1.

Acceptance criteria:
- Session modules compile and expose Pi-compatible contracts for harness/compaction.

Handoff to dependent units:
- A5 and A7 consume session context and storage APIs; AT1 ports tests.

Subagent prompt:
```text
You are implementing only A2 from .agents/plans/zedflow-agent-pi-agent-port.md. Fresh context. Read A2, A0/A1 outputs, rust-skills, references/pi/packages/agent/src/harness/session/*.ts, and relevant message constructors. Port session tree/context, memory storage/repo, JSONL storage/repo, repo utils, and UUID helper. Use uuid crate from A1. Do not implement compaction or harness integration. Run fmt and cargo check -p zedflow-agent only.
```

<a id="A3"></a>
### Task A3 — Messages, templates, skills, and text utilities

Assignable: yes

Execution metadata:
- Wave: W2
- Context: fresh
- Depends on: A1
- Can run in parallel with: A2
- Must not run in parallel with: AT2 or tasks editing `harness/messages.rs`, `harness/skills.rs`, `harness/prompt-templates.rs`, `harness/system-prompt.rs`, `harness/utils/*`

Scope boundaries:
- Goal: Port message conversion/constructors, prompt templates, skills loading, system prompt formatting, shell-output formatting, and truncation utilities.
- Non-goals: Do not implement env filesystem backend; use `ExecutionEnv` trait from A1.
- Forbidden work: Do not hand-roll ignore or YAML behavior if approved crates are available.

Files:
| Action | Path | Purpose |
|---|---|---|
| create/modify | `crates/zedflow-agent/src/harness/messages.rs` | AgentMessage constructors and LLM conversion. |
| create/modify | `crates/zedflow-agent/src/harness/prompt-templates.rs` | Markdown/frontmatter prompt templates. |
| create/modify | `crates/zedflow-agent/src/harness/skills.rs` | Skill loading and invocation formatting. |
| create/modify | `crates/zedflow-agent/src/harness/system-prompt.rs` | System prompt assembly. |
| create/modify | `crates/zedflow-agent/src/harness/utils/truncate.rs` | Truncation helpers. |
| create/modify | `crates/zedflow-agent/src/harness/utils/shell-output.rs` | Shell output formatting. |
| read | `references/pi/packages/agent/src/harness/messages.ts` | Canonical message behavior. |
| read | `references/pi/packages/agent/src/harness/prompt-templates.ts` | YAML frontmatter and substitution. |
| read | `references/pi/packages/agent/src/harness/skills.ts` | Skill traversal and diagnostics. |
| read | `references/pi/packages/agent/src/harness/system-prompt.ts` | Prompt assembly. |
| read | `references/pi/packages/agent/src/harness/utils/*.ts` | Utility behavior. |

Required context package:
- Plan references: RF-DEPS-REPLACEMENT, A3.
- Required skills: rust-skills.
- Dependency outputs to read: A0/A1 outputs.
- Required docs: docs.rs `ignore::WalkBuilder`/`GitignoreBuilder`, `yaml_serde::from_str` if implementation detail is unclear.
- Neighboring out-of-scope units: A4 env backend; AT2 tests.

Implementation outline:
1. Implement shared `parse_frontmatter<T: DeserializeOwned + Default>` using Pi delimiter semantics and `yaml_serde::from_str`.
2. Implement prompt template argument parsing/substitution for `$1`, `$@`, `$ARGUMENTS`, `${@:N}`, `${@:N:L}`.
3. Implement skill loading with `ignore` crate plus Pi behavior: `SKILL.md` wins for a directory, direct root `.md` files only at input root, skip hidden entries and `node_modules`, diagnostics on read/parse/metadata errors.
4. Implement message conversion using `zedflow-ai` content/message types.
5. Implement truncation and shell-output helpers exactly from Pi tests.

Major snippets:

#### [CANONICAL] Frontmatter parser behavior
```rust
let normalized = content.replace("\r\n", "\n").replace('\r', "\n");
if !normalized.starts_with("---") { /* no frontmatter */ }
let Some(end_index) = normalized[3..].find("\n---").map(|idx| idx + 3) else { /* no frontmatter */ };
let yaml = &normalized[4..end_index];
let body = normalized[end_index + 4..].trim().to_string();
let frontmatter = yaml_serde::from_str(yaml)?;
```

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; `cargo check -p zedflow-agent`.
- Must NOT run: AT2 tests until source unit complete.

Output contract:
- List dependency APIs used (`ignore`, `yaml_serde`).
- List any exact Pi skill traversal behavior not represented.
- List tests ready for AT2.

Acceptance criteria:
- Modules compile and have no custom YAML/ignore replacement drift beyond documented limitations.

Handoff to dependent units:
- A5/A7 consume messages/templates/skills; AT2 ports tests.

Subagent prompt:
```text
You are implementing only A3 from .agents/plans/zedflow-agent-pi-agent-port.md. Fresh context. Read A3, A0/A1 outputs, rust-skills, Pi messages/prompt-templates/skills/system-prompt/utils files, and docs for ignore/yaml_serde as needed. Port these modules using yaml_serde and ignore, preserving Pi frontmatter, substitution, skill traversal, diagnostics, and message conversion. Do not implement NodeExecutionEnv. Run fmt and cargo check -p zedflow-agent only.
```

<a id="A4"></a>
### Task A4 — Node execution environment and proxy seam

Assignable: yes

Execution metadata:
- Wave: W3
- Context: fresh
- Depends on: A1
- Can run in parallel with: A2/A3 only if no shared file edits are needed
- Must not run in parallel with: AT3

Scope boundaries:
- Goal: Port Node environment file/process/path behavior and proxy event seam using Rust stdlib plus approved dependencies.
- Non-goals: Do not implement coding-agent terminal UI or broad process supervisor behavior.
- Forbidden work: Do not add a Tokio runtime or process-tree crate unless a test proves std + `wait-timeout` cannot satisfy required behavior.

Files:
| Action | Path | Purpose |
|---|---|---|
| create/modify | `crates/zedflow-agent/src/harness/env/nodejs.rs` | Rust backend for Pi Node env abstraction. |
| create/modify | `crates/zedflow-agent/src/proxy.rs` | Proxy assistant event bridge. |
| create/modify | `crates/zedflow-agent/src/node.rs` | Node-specific facade. |
| read | `references/pi/packages/agent/src/harness/env/nodejs.ts` | Canonical env behavior. |
| read | `references/pi/packages/agent/src/proxy.ts` | Proxy event behavior. |
| read | `references/pi/packages/agent/src/node.ts` | Node export behavior. |

Required context package:
- Plan references: RF-NODE-ENV, A4.
- Required skills: rust-skills.
- Dependency outputs to read: A0/A1 outputs.
- Required docs: docs.rs `wait_timeout::ChildExt`, `uuid::Uuid::new_v4` if needed.
- Neighboring out-of-scope units: A7 harness; AT3 tests.

Implementation outline:
1. Implement `NodeExecutionEnv` over `std::fs`, `std::path`, `std::env::temp_dir`, and `uuid`.
2. Map `std::io::ErrorKind` into stable `FileErrorCode` and `ExecutionErrorCode`.
3. Implement shell selection, env merging, cwd resolution, timeout validation, stdout/stderr capture, and timeout kill using `wait-timeout`.
4. Document process-tree kill limitations as `PORT PLACEHOLDER` only if exact Pi behavior is not implemented.
5. Port proxy JSON event parsing using `serde_json` and `zedflow-ai` event types.

Major snippets:

#### [CANONICAL] Timeout process replacement
```rust
use wait_timeout::ChildExt;
match child.wait_timeout(timeout)? {
    Some(status) => status,
    None => {
        let _ = child.kill();
        let status = child.wait()?;
        // return timeout error with captured output
    }
}
```

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; `cargo check -p zedflow-agent`.
- Must NOT run: env/proxy tests until AT3; workspace gates.

Output contract:
- List Node APIs mapped to Rust.
- List process/platform limitations.
- List proxy behavior implemented.

Acceptance criteria:
- Env/proxy modules compile and expose the contracts tests need.

Handoff to dependent units:
- A3 skills/templates use `ExecutionEnv`; A7 harness uses node facade; AT3 ports tests.

Subagent prompt:
```text
You are implementing only A4 from .agents/plans/zedflow-agent-pi-agent-port.md. Fresh context. Read A4, A0/A1 outputs, rust-skills, Pi harness/env/nodejs.ts, proxy.ts, node.ts, and wait-timeout/uuid docs as needed. Port the Node execution environment using stdlib, uuid, and wait-timeout. Port proxy event parsing. Do not add Tokio or broad process supervisor crates unless blocked and reported. Run fmt and cargo check -p zedflow-agent only.
```

<a id="A5"></a>
### Task A5 — Compaction and branch summarization

Assignable: yes

Execution metadata:
- Wave: W4
- Context: fresh
- Depends on: A2, A3
- Can run in parallel with: none
- Must not run in parallel with: AT4

Scope boundaries:
- Goal: Port compaction helpers, compaction preparation/execution, and branch summarization using `zedflow-ai` models and session/messages foundations.
- Non-goals: Do not add Zedflow graph memory or new summarization product behavior.
- Forbidden work: Do not change session JSONL format to make compaction easier.

Files:
| Action | Path | Purpose |
|---|---|---|
| create/modify | `crates/zedflow-agent/src/harness/compaction/utils.rs` | Conversation serialization/token helpers. |
| create/modify | `crates/zedflow-agent/src/harness/compaction/compaction.rs` | Compaction settings/prepare/compact. |
| create/modify | `crates/zedflow-agent/src/harness/compaction/branch-summarization.rs` | Branch summary collection/generation. |
| read | `references/pi/packages/agent/src/harness/compaction/*.ts` | Canonical compaction behavior. |

Required context package:
- Plan references: RF-SESSION-FORMAT, A5.
- Required skills: rust-skills.
- Dependency outputs to read: A2 and A3 outputs.
- Required files/symbols to read: Pi compaction files, Rust session/messages modules.
- Neighboring out-of-scope units: A7 harness integration; AT4 tests.

Implementation outline:
1. Port conversation serialization and token estimation semantics.
2. Port compaction settings and prompt construction.
3. Use `zedflow-ai::Models`/`Model` interfaces for summarization without inventing provider behavior.
4. Return typed `Result`/errors rather than panics.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; `cargo check -p zedflow-agent`.
- Must NOT run: compaction tests until AT4; live model calls.

Output contract:
- List compaction APIs implemented.
- List model-call paths left capability-gated or ignored for tests.

Acceptance criteria:
- Compaction modules compile and preserve Pi prompt/session behavior.

Handoff to dependent units:
- A7 uses compaction; AT4 ports tests.

Subagent prompt:
```text
You are implementing only A5 from .agents/plans/zedflow-agent-pi-agent-port.md. Fresh context. Read A5, A2/A3 outputs, rust-skills, Pi harness/compaction files, and Rust session/messages modules. Port compaction and branch summarization using zedflow-ai model APIs. Do not add Zedflow graph behavior or live provider tests. Run fmt and cargo check -p zedflow-agent only.
```

<a id="A6"></a>
### Task A6 — Agent loop and agent facade behavior

Assignable: yes

Execution metadata:
- Wave: W5
- Context: fresh
- Depends on: A1, A3
- Can run in parallel with: none
- Must not run in parallel with: AT5

Scope boundaries:
- Goal: Port `agent-loop.ts` and `agent.ts` behavior: prompt injection, event sequence, provider stream consumption, tool execution modes, queue modes, hooks, continuation, and stop conditions.
- Non-goals: Do not implement full harness session/resource integration; A7 owns harness.
- Forbidden work: Do not create another assistant event stream abstraction.

Files:
| Action | Path | Purpose |
|---|---|---|
| create/modify | `crates/zedflow-agent/src/agent-loop.rs` | Core agent loop. |
| create/modify | `crates/zedflow-agent/src/agent.rs` | Higher-level agent API. |
| read | `references/pi/packages/agent/src/agent-loop.ts` | Canonical loop behavior. |
| read | `references/pi/packages/agent/src/agent.ts` | Canonical agent API. |
| read | `references/pi/packages/agent/src/types.ts` | Config/hook semantics. |

Required context package:
- Plan references: RF-ASYNC-STREAM, A6.
- Required skills: rust-skills.
- Dependency outputs to read: A1 and A3 outputs.
- Required files/symbols to read: Pi agent-loop/agent/types; `zedflow-ai` stream contracts.
- Neighboring out-of-scope units: A7 harness; AT5 tests.

Implementation outline:
1. Port event sequence exactly: `agent_start`, `turn_start`, message start/end, provider stream events, tool execution events, turn end, agent end.
2. Convert `AgentMessage` to LLM `Message` only at LLM call boundary.
3. Implement sequential and parallel tool execution semantics; if true parallel execution is deferred, add exact placeholder/ignored tests and do not fake ordering.
4. Preserve queue drain modes and continuation preconditions.
5. Map provider failures into stream/error events, not panics.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; `cargo check -p zedflow-agent`.
- Must NOT run: agent-loop tests until AT5; live provider calls.

Output contract:
- List event sequence behaviors implemented.
- List tool/queue/hook behaviors implemented or blocked.

Acceptance criteria:
- Agent loop compiles and can be exercised by deterministic fake stream/tool tests.

Handoff to dependent units:
- A7 wraps loop in harness; AT5 ports loop tests.

Subagent prompt:
```text
You are implementing only A6 from .agents/plans/zedflow-agent-pi-agent-port.md. Fresh context. Read A6, A1/A3 outputs, rust-skills, Pi agent-loop.ts, agent.ts, types.ts, and zedflow-ai stream contracts. Port the core loop and agent facade preserving Pi event order, tool execution, queue modes, hooks, continuation errors, and provider stream error semantics. Do not implement harness integration. Run fmt and cargo check -p zedflow-agent only.
```

<a id="A7"></a>
### Task A7 — Agent harness integration

Assignable: yes

Execution metadata:
- Wave: W6
- Context: fresh
- Depends on: A2, A3, A4, A5, A6
- Can run in parallel with: none
- Must not run in parallel with: AT6

Scope boundaries:
- Goal: Port `harness/agent-harness.ts` integration across sessions, skills, prompt templates, resources, compaction, stream options, and agent loop.
- Non-goals: Do not implement coding-agent CLI/TUI or product graph behavior.
- Forbidden work: Do not define private duplicate versions of foundation APIs.

Files:
| Action | Path | Purpose |
|---|---|---|
| create/modify | `crates/zedflow-agent/src/harness/agent-harness.rs` | Harness integration. |
| read | `references/pi/packages/agent/src/harness/agent-harness.ts` | Canonical harness behavior. |
| read | `crates/zedflow-agent/src/harness/**/*.rs` | Foundation modules. |

Required context package:
- Plan references: RF-HARNESS-LATE, A7.
- Required skills: rust-skills.
- Dependency outputs to read: A2-A6 outputs.
- Required files/symbols to read: Pi agent-harness.ts and Rust foundation modules.
- Neighboring out-of-scope units: AT6 tests; coding-agent package.

Implementation outline:
1. Wire resource loading, prompt/template/skill invocation, stream option patching, sessions, compaction, and loop calls through existing modules.
2. Preserve Pi error handling: expected failures return typed errors/results, not panics.
3. Keep live provider/model calls capability-gated or test-injected.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; `cargo check -p zedflow-agent`.
- Must NOT run: harness tests until AT6; workspace gates.

Output contract:
- List harness APIs implemented.
- List integration blockers for tests.

Acceptance criteria:
- Harness compiles without redefining foundational contracts.

Handoff to dependent units:
- AT6 ports harness tests; A8 re-exports harness APIs.

Subagent prompt:
```text
You are implementing only A7 from .agents/plans/zedflow-agent-pi-agent-port.md. Fresh context. Read A7, A2-A6 outputs, rust-skills, Pi harness/agent-harness.ts, and Rust foundation modules. Port harness integration using existing session/messages/templates/skills/env/compaction/loop modules. Do not add coding-agent/TUI behavior or duplicate types. Run fmt and cargo check -p zedflow-agent only.
```

<a id="A8"></a>
### Task A8 — Root facade and module closure

Assignable: yes

Execution metadata:
- Wave: W7
- Context: fresh
- Depends on: A7
- Can run in parallel with: none
- Must not run in parallel with: tests editing facade imports

Scope boundaries:
- Goal: Port `index.ts`, `node.ts` exports, close module declarations, and ensure all source manifest rows are represented.
- Non-goals: Do not add behavior beyond re-exports and missing module glue.
- Forbidden work: Do not hide missing implementation behind misleading public exports.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `crates/zedflow-agent/src/lib.rs` | Final module declarations. |
| create/modify | `crates/zedflow-agent/src/index.rs` | Pi root facade exports. |
| create/modify | `crates/zedflow-agent/src/node.rs` | Node facade exports. |
| read | `references/pi/packages/agent/src/index.ts` | Canonical exports. |
| read | `references/pi/packages/agent/src/node.ts` | Node exports. |
| read | `.agents/port-manifests/agent-src.tsv` | Source manifest completion audit. |

Required context package:
- Plan references: global acceptance, A8.
- Required skills: rust-skills.
- Dependency outputs to read: A1-A7 outputs.
- Neighboring out-of-scope units: test implementations.

Implementation outline:
1. Re-export the same conceptual Pi public surface using Rust module conventions.
2. Audit `agent-src.tsv` and add documented placeholders only for truly unresolved files.
3. Ensure docs do not promise unimplemented behavior.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; `cargo check -p zedflow-agent --all-targets`.
- Must NOT run: full tests unless cheap targeted facade tests already exist.

Output contract:
- Source manifest completion summary.
- Public export summary.
- Placeholder list, if any.

Acceptance criteria:
- All source rows are represented and package all-targets check passes or blockers are exact.

Handoff to dependent units:
- AT1-AT7 can rely on facade imports; AV1 validates final package.

Subagent prompt:
```text
You are implementing only A8 from .agents/plans/zedflow-agent-pi-agent-port.md. Fresh context. Read A8, A1-A7 outputs, rust-skills, Pi index.ts/node.ts, and agent-src.tsv. Close module declarations and public facades without adding behavior. Audit source row representation and add only documented PORT PLACEHOLDERs for real blockers. Run fmt and cargo check -p zedflow-agent --all-targets.
```

<a id="AT1"></a>
### Task AT1 — Session/storage tests

Assignable: yes

Execution metadata:
- Wave: W8
- Context: fresh
- Depends on: A2, A8
- Can run in parallel with: AT2-AT7 if target files differ
- Must not run in parallel with: A2

Scope boundaries:
- Goal: Port session/storage/uuid/repo tests.
- Non-goals: Do not implement missing source behavior beyond metadata-only test adjustments; report blockers.
- Forbidden work: Do not weaken session format assertions.

Files:
| Action | Path | Purpose |
|---|---|---|
| create/modify | `crates/zedflow-agent/tests/harness/session.rs` | Session tests. |
| create/modify | `crates/zedflow-agent/tests/harness/storage.rs` | Storage tests. |
| create/modify | `crates/zedflow-agent/tests/harness/repo.rs` | Repo tests. |
| create/modify | `crates/zedflow-agent/tests/harness/session-uuid.rs` | UUID tests. |
| create/modify | `crates/zedflow-agent/tests/harness/session-test-utils.rs` | Test helpers. |
| read | `references/pi/packages/agent/test/harness/session*.ts` | Canonical tests. |
| read | `references/pi/packages/agent/test/harness/repo.test.ts` | Repo tests. |
| read | `references/pi/packages/agent/test/harness/storage.test.ts` | Storage tests. |

Required context package:
- Plan references: AT1, RF-SESSION-FORMAT.
- Required skills: rust-skills.
- Dependency outputs to read: A2/A8 outputs.

Implementation outline:
1. Port assertions directly.
2. Use temp dirs without leaking files.
3. Ignore only environment-specific tests with exact reason.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; targeted session/storage tests.
- Must NOT run: full package tests.

Output contract:
- Tests ported/ignored with reasons.
- Source blockers, if any.

Acceptance criteria:
- Session/storage behavior is represented in Rust tests.

Handoff to dependent units:
- AV1 includes tests in full package gate.

Subagent prompt:
```text
You are implementing only AT1 from .agents/plans/zedflow-agent-pi-agent-port.md. Fresh context. Read AT1, A2/A8 outputs, rust-skills, and assigned Pi session/storage/repo/uuid tests. Port only these tests to the listed Rust targets. Do not add source behavior except metadata-only fixes; report source blockers. Run fmt and targeted zedflow-agent session/storage tests only.
```

<a id="AT2"></a>
### Task AT2 — Prompt, skill, system-prompt, and utility tests

Assignable: yes

Execution metadata:
- Wave: W8
- Context: fresh
- Depends on: A3, A8
- Can run in parallel with: AT1/AT3-AT7 if target files differ
- Must not run in parallel with: A3

Scope boundaries:
- Goal: Port prompt template, skills, system prompt, truncate, shell output, and resource formatting tests.
- Non-goals: Do not change dependency choices.
- Forbidden work: Do not replace `yaml_serde`/`ignore` with ad-hoc test-only behavior.

Files:
| Action | Path | Purpose |
|---|---|---|
| create/modify | `crates/zedflow-agent/tests/harness/prompt-templates.rs` | Prompt template tests. |
| create/modify | `crates/zedflow-agent/tests/harness/skills.rs` | Skill loader tests. |
| create/modify | `crates/zedflow-agent/tests/harness/system-prompt.rs` | System prompt tests. |
| create/modify | `crates/zedflow-agent/tests/harness/truncate.rs` | Truncation tests. |
| create/modify | `crates/zedflow-agent/tests/harness/resource-formatting.rs` | Formatting tests. |
| read | `references/pi/packages/agent/test/harness/*.test.ts` | Canonical tests. |
| read | `references/pi/packages/agent/test/utils/*.ts` | Utility tests. |

Required context package:
- Plan references: AT2, RF-DEPS-REPLACEMENT.
- Required skills: rust-skills.
- Dependency outputs to read: A3/A8 outputs.

Implementation outline:
1. Port deterministic assertions for frontmatter, substitution, skill diagnostics, ignore files, system prompt formatting, truncation, and shell output.
2. Use temp fixture directories.
3. Mark JS-only filesystem edge cases ignored only with exact reason.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; targeted prompt/skill/system/util tests.
- Must NOT run: full package tests.

Output contract:
- Tests ported/ignored with reasons.
- Any dependency parity gaps.

Acceptance criteria:
- Frontmatter/ignore/template behaviors are tested against Rust replacements.

Handoff to dependent units:
- AV1 runs all tests.

Subagent prompt:
```text
You are implementing only AT2 from .agents/plans/zedflow-agent-pi-agent-port.md. Fresh context. Read AT2, A3/A8 outputs, rust-skills, Pi prompt-template/skills/system-prompt/truncate/resource-formatting tests, and Rust modules. Port only this subsystem's tests. Preserve yaml/ignore behavior and use ignored tests only for exact JS-only blockers. Run fmt and targeted tests only.
```

<a id="AT3"></a>
### Task AT3 — Environment and proxy tests

Assignable: yes

Execution metadata:
- Wave: W8
- Context: fresh
- Depends on: A4, A8
- Can run in parallel with: AT1/AT2/AT4-AT7 if target files differ
- Must not run in parallel with: A4

Scope boundaries:
- Goal: Port Node environment, proxy, current-time, and calculate utility tests owned by env/proxy behavior.
- Non-goals: Do not implement CLI/TUI.
- Forbidden work: Do not add broad process-tree dependencies unless a failing test proves need and parent approves.

Files:
| Action | Path | Purpose |
|---|---|---|
| create/modify | `crates/zedflow-agent/tests/harness/nodejs-env.rs` | Node env tests. |
| create/modify | `crates/zedflow-agent/tests/utils/get-current-time.rs` | Time utility tests if applicable. |
| create/modify | `crates/zedflow-agent/tests/utils/calculate.rs` | Calculate utility tests if applicable. |
| read | `references/pi/packages/agent/test/harness/nodejs-env.test.ts` | Canonical env tests. |
| read | `references/pi/packages/agent/test/utils/*.ts` | Utility tests. |

Required context package:
- Plan references: AT3, RF-NODE-ENV.
- Required skills: rust-skills.
- Dependency outputs to read: A4/A8 outputs.

Implementation outline:
1. Port deterministic file/process/path tests.
2. Use temp dirs and small commands only.
3. Ignore platform-specific process-tree tests if not implemented, with exact reason.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; targeted env/proxy tests.
- Must NOT run: full package tests.

Output contract:
- Tests ported/ignored with reasons.
- Platform limitations discovered.

Acceptance criteria:
- Rust env seam is covered by deterministic tests.

Handoff to dependent units:
- AV1 runs all tests.

Subagent prompt:
```text
You are implementing only AT3 from .agents/plans/zedflow-agent-pi-agent-port.md. Fresh context. Read AT3, A4/A8 outputs, rust-skills, Pi nodejs-env/proxy-related tests and Rust env/proxy modules. Port deterministic env/proxy tests using temp dirs and safe commands. Do not add new process dependencies without reporting blocker. Run fmt and targeted tests only.
```

<a id="AT4"></a>
### Task AT4 — Compaction tests

Assignable: yes

Execution metadata:
- Wave: W8
- Context: fresh
- Depends on: A5, A8
- Can run in parallel with: AT1-AT3/AT5-AT7 if target files differ
- Must not run in parallel with: A5

Scope boundaries:
- Goal: Port compaction and branch summarization tests.
- Non-goals: Do not run live model calls.
- Forbidden work: Do not fake summarization success without parity assertions.

Files:
| Action | Path | Purpose |
|---|---|---|
| create/modify | `crates/zedflow-agent/tests/harness/compaction.rs` | Compaction tests. |
| read | `references/pi/packages/agent/test/harness/compaction.test.ts` | Canonical tests. |

Required context package:
- Plan references: AT4.
- Required skills: rust-skills.
- Dependency outputs to read: A5/A8 outputs.

Implementation outline:
1. Port deterministic serialization/preparation/branch collection tests.
2. Use fake models/streams for summarization.
3. Ignore live provider behavior with exact reason.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; targeted compaction tests.
- Must NOT run: live model tests.

Output contract:
- Tests ported/ignored with reasons.
- Compaction source blockers.

Acceptance criteria:
- Compaction deterministic behavior is represented.

Subagent prompt:
```text
You are implementing only AT4 from .agents/plans/zedflow-agent-pi-agent-port.md. Fresh context. Read AT4, A5/A8 outputs, rust-skills, Pi compaction tests, and Rust compaction modules. Port deterministic compaction tests using fake model/stream behavior. Do not run live model calls. Run fmt and targeted tests only.
```

<a id="AT5"></a>
### Task AT5 — Agent-loop and agent tests

Assignable: yes

Execution metadata:
- Wave: W8
- Context: fresh
- Depends on: A6, A8
- Can run in parallel with: AT1-AT4/AT6-AT7 if target files differ
- Must not run in parallel with: A6

Scope boundaries:
- Goal: Port `agent-loop.test.ts` and `agent.test.ts` with fake streams/tools.
- Non-goals: Do not cover harness resource integration.
- Forbidden work: Do not relax event ordering assertions.

Files:
| Action | Path | Purpose |
|---|---|---|
| create/modify | `crates/zedflow-agent/tests/agent-loop.rs` | Agent loop tests. |
| create/modify | `crates/zedflow-agent/tests/agent.rs` | Agent API tests. |
| read | `references/pi/packages/agent/test/agent-loop.test.ts` | Canonical loop tests. |
| read | `references/pi/packages/agent/test/agent.test.ts` | Canonical agent tests. |

Required context package:
- Plan references: AT5, RF-ASYNC-STREAM.
- Required skills: rust-skills.
- Dependency outputs to read: A6/A8 outputs.

Implementation outline:
1. Port fake stream and fake tool harnesses.
2. Assert event order, context updates, tool modes, queue modes, continuation errors, and stop hooks.
3. Ignore only unimplemented async scheduling edges with exact reason.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; targeted agent-loop/agent tests.
- Must NOT run: live provider tests.

Output contract:
- Tests ported/ignored with reasons.
- Source blockers if loop behavior is incomplete.

Acceptance criteria:
- Core agent behavior is covered by deterministic tests.

Subagent prompt:
```text
You are implementing only AT5 from .agents/plans/zedflow-agent-pi-agent-port.md. Fresh context. Read AT5, A6/A8 outputs, rust-skills, Pi agent-loop/agent tests, and Rust loop modules. Port tests with fake streams/tools and preserve event-order assertions. Do not test harness integration or live providers. Run fmt and targeted tests only.
```

<a id="AT6"></a>
### Task AT6 — Agent harness stream/integration tests

Assignable: yes

Execution metadata:
- Wave: W8
- Context: fresh
- Depends on: A7, A8
- Can run in parallel with: AT1-AT5/AT7 if target files differ
- Must not run in parallel with: A7

Scope boundaries:
- Goal: Port harness integration and stream tests.
- Non-goals: Do not run full e2e/live browser/provider tests.
- Forbidden work: Do not patch harness source ad hoc; report source blockers.

Files:
| Action | Path | Purpose |
|---|---|---|
| create/modify | `crates/zedflow-agent/tests/harness/agent-harness.rs` | Harness tests. |
| create/modify | `crates/zedflow-agent/tests/harness/agent-harness-stream.rs` | Harness stream tests. |
| read | `references/pi/packages/agent/test/harness/agent-harness.test.ts` | Harness tests. |
| read | `references/pi/packages/agent/test/harness/agent-harness-stream.test.ts` | Stream tests. |

Required context package:
- Plan references: AT6, RF-HARNESS-LATE.
- Required skills: rust-skills.
- Dependency outputs to read: A7/A8 outputs.

Implementation outline:
1. Port deterministic harness tests using memory env/storage and fake models.
2. Assert resource loading, prompt/skill invocation, stream option snapshots, compaction hooks, and session writes.
3. Ignore only true e2e/live provider behavior.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; targeted harness tests.
- Must NOT run: live provider/e2e tests.

Output contract:
- Tests ported/ignored with reasons.
- Harness source blockers.

Acceptance criteria:
- Harness integration is represented by deterministic tests.

Subagent prompt:
```text
You are implementing only AT6 from .agents/plans/zedflow-agent-pi-agent-port.md. Fresh context. Read AT6, A7/A8 outputs, rust-skills, Pi harness agent-harness tests, and Rust harness modules. Port deterministic harness tests with fake models/env/storage. Do not run live/e2e tests. Run fmt and targeted tests only.
```

<a id="AT7"></a>
### Task AT7 — Scratch/e2e/live samples

Assignable: yes

Execution metadata:
- Wave: W8
- Context: fresh
- Depends on: A8
- Can run in parallel with: AT1-AT6 if target files differ
- Must not run in parallel with: none

Scope boundaries:
- Goal: Represent e2e and scratch test rows with deterministic tests where possible or ignored tests with exact blockers.
- Non-goals: Do not require live credentials or browser sessions.
- Forbidden work: Do not mark e2e behavior as passed without assertions.

Files:
| Action | Path | Purpose |
|---|---|---|
| create/modify | `crates/zedflow-agent/tests/e2e.rs` | E2E representation. |
| create/modify | `crates/zedflow-agent/tests/scratch/simple.rs` | Scratch sample. |
| read | `references/pi/packages/agent/test/e2e.test.ts` | Canonical e2e tests. |
| read | `references/pi/packages/agent/test/scratch/simple.ts` | Canonical sample. |

Required context package:
- Plan references: AT7.
- Required skills: rust-skills.
- Dependency outputs to read: A8 output.

Implementation outline:
1. Port local deterministic portions.
2. Convert live/browser/provider portions to ignored tests with exact capability/blocker reasons.
3. Keep samples compiling if possible.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; targeted e2e/scratch compile/test commands.
- Must NOT run: live provider/browser tests.

Output contract:
- Tests/samples represented.
- Ignore reasons and blockers.

Acceptance criteria:
- Remaining unported e2e behavior is explicit and searchable.

Subagent prompt:
```text
You are implementing only AT7 from .agents/plans/zedflow-agent-pi-agent-port.md. Fresh context. Read AT7, A8 output, rust-skills, Pi e2e and scratch tests. Represent deterministic behavior in Rust and add ignored tests for live/browser/provider behavior with exact reasons. Do not require credentials. Run fmt and targeted e2e/scratch checks only.
```

<a id="AV1"></a>
### Task AV1 — Final package validation and report

Assignable: yes

Execution metadata:
- Wave: W9
- Context: fresh
- Depends on: A1-A8, AT1-AT7
- Can run in parallel with: none
- Must not run in parallel with: all other write units

Scope boundaries:
- Goal: Validate `zedflow-agent` package completion against this plan and update progress state.
- Non-goals: Do not implement new source behavior except metadata-only stale ignore/doc fixes discovered during audit.
- Forbidden work: Do not hide failures with new ignores unless the blocker is exact and belongs outside current scope.

Files:
| Action | Path | Purpose |
|---|---|---|
| create/modify | `.agents/state/zedflow-agent-pi-agent-port-final-report.md` | Final report. |
| modify | `.agents/state/pi-to-rust-package-port-progress.md` | Mark W3/P2 source/test status for continuation. |
| read | `.agents/port-manifests/agent-src.tsv` | Source completion audit. |
| read | `.agents/port-manifests/agent-tests.tsv` | Test completion audit. |
| read | `crates/zedflow-agent/**/*` | Validation/audit. |

Required context package:
- Plan references: global acceptance, AV1.
- Required skills: rust-skills.
- Dependency outputs to read: all prior unit outputs.
- Required files/symbols to read: manifests and package files.

Implementation outline:
1. Audit all manifest rows for represented target files/tests.
2. Audit `PORT PLACEHOLDER` markers and ignored tests for exact reasons.
3. Run final package gates.
4. Write final report and update progress state with next recommended wave.

Validation responsibility:
- Type: integration-validating
- Must run: `cargo fmt --all --check`; `cargo check -p zedflow-agent --all-targets`; `cargo test -p zedflow-agent --all-targets --no-run`; targeted runnable tests; placeholder/ignore audits.
- Must NOT run: live/network/browser tests.

Output contract:
- Final report path and summary.
- Manifest completion counts.
- Validation command results.
- Remaining placeholders/ignored tests with reasons.
- Next-wave recommendation.

Acceptance criteria:
- Package is ready for the next global port wave or blockers are explicit enough for follow-up.

Subagent prompt:
```text
You are implementing only AV1 from .agents/plans/zedflow-agent-pi-agent-port.md. Fresh context. Read AV1, all prior outputs, rust-skills, manifests, and zedflow-agent package files. Audit source/test row representation, placeholders, ignored tests, docs, and dependency usage. Run final zedflow-agent gates but no live/network/browser tests. Write .agents/state/zedflow-agent-pi-agent-port-final-report.md and update .agents/state/pi-to-rust-package-port-progress.md with W3/P2 status and next-wave recommendation.
```

<a id="pre-finalization-review"></a>
## Pre-finalization Review Summary

- Dependency reconnaissance was performed from `references/pi/packages/agent/package.json`, package source imports, and docs for `ignore`, `yaml_serde`, `jsonschema`, `uuid`, and `wait-timeout`.
- User approved the recommended dependency replacements before finalization: `ignore`, `yaml_serde`, `serde_json`/`jsonschema`, `uuid`, and `wait-timeout`.
- The plan deliberately replaces raw manifest-order execution with dependency-first waves to prevent duplicate foundations observed during the `zedflow-ai` port.
- Remaining risks are captured as review flags: process-tree kill parity, JS-only runtime mechanics, session JSONL compatibility, and agent stream/event alignment with `zedflow-ai`.
