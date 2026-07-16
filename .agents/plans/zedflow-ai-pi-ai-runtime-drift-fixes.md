<!-- migration-document-status: SUPERSEDED -->
> [!CAUTION]
> **Migration status: SUPERSEDED.** Historical plan only. Use `.agents/plans/zedflow-ai-agent-pi-fidelity-consolidation.md` and `.agents/state/zedflow-ai-agent-pi-fidelity-current-status.md` for current work.

# Zedflow AI Pi-AI Runtime Drift Fixes

<a id="how-to-use"></a>
## How to use this plan

This plan is self-contained for orchestration by a fresh agent session.

- All implementation subagents must run in fresh context.
- Execute only assignable unit IDs listed in the orchestration waves.
- Before launching a unit, pass its full `Subagent prompt` plus the relevant plan references from `Canonical Line References`.
- Do not infer requirements from outside this plan and the listed references.
- Do not execute neighboring task scopes.
- If a unit is marked `non-validating`, do not run global compile/lint/test gates or add compatibility workarounds to make the repo compile.
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

Correct the structural runtime drifts in the `references/pi/packages/ai` TypeScript to `crates/zedflow-ai` Rust port. The aim is not merely to reduce ignored tests or remove placeholder wording: it is to restore Pi-AI observable runtime semantics in Rust for provider/model/auth dispatch, stream events, live transports, compat/faux behavior, image models, and public API shape where the port intentionally mirrors Pi-AI.

<a id="non-goals"></a>
## Non-goals

- Do not implement unrelated Zedflow product architecture outside `crates/zedflow-ai`.
- Do not add compatibility shims that hide drift while preserving the wrong Rust runtime model.
- Do not port JS-only observability literally when Rust has no equivalent, such as Node `registerHooks` exact dynamic import specifier checks.
- Do not run unavailable live provider suites or log credentials.
- Do not weaken Pi parity tests to fit current Rust behavior.
- Do not expose `genai` dependency types in public APIs.

<a id="review-flags"></a>
## Review Flags

| ID | Type | Severity | Summary | Affects | Follow-up / accepted risk |
|---|---|---:|---|---|---|
| RF-MODELS-CHOKEPOINT | R | High | `crates/zedflow-ai/src/models.rs` currently defines a second minimal provider/model/stream universe instead of using Pi-compatible public types. | R1-R5, R9-R13 | Fix `models.rs` and stream contracts before provider-by-provider live work. |
| RF-ASYNC-RUST-SURFACE | R | High | Pi model/auth/refresh/provider flows are async/fallible; Rust currently has sync facades in key paths. | R3-R5 | Introduce async internals where Pi requires them; sync wrappers may remain only as compatibility adapters. |
| RF-LIVE-CREDENTIALS | OQ | Medium | OpenRouter and OpenAI Codex credentials are present in the environment, but live execution is blocked by missing transports/dispatch. | R9-R11, R14 | Live tests must execute once transports are implemented; until then report implementation blockers, not missing credentials. |
| RF-JS-ONLY | R | Low | Some Pi tests assert Node-specific dynamic import behavior with no exact Rust equivalent. | R13-R14 | Keep documented as JS-only and cover side-effect-free Rust equivalents instead. |
| RF-PUBLIC-API | OQ | Medium | Rust crate may intentionally expose a broader API than Pi root package, but Pi-compatible facade still needs definition. | R13 | Preserve useful Rust exports only if they do not conflict with Pi parity or leak internals. |

<a id="global-acceptance"></a>
## Global Acceptance Criteria

- `grep -R "PORT PLACEHOLDER" crates/zedflow-ai/src crates/zedflow-ai/tests` returns zero matches.
- `cargo fmt --all --check`, `cargo check -p zedflow-ai --all-targets`, and `cargo test -p zedflow-ai --all-targets` pass.
- Public `zedflow-ai` APIs do not expose `genai` types.
- Remaining ignored tests are only live/manual/provider-capability/JS-only/upstream-skipped cases with exact reasons, or explicit accepted product decisions.
- OpenRouter and OpenAI Codex live tests execute when detected credentials and implemented transports are available; otherwise they report implementation blockers without leaking secrets.
- `crates/zedflow-ai/src/models.rs` no longer owns duplicate minimal message/model/stream types that conflict with `crates/zedflow-ai/src/types.rs`.
- Provider/model/auth/stream/faux/image behavior has direct Pi TS reference coverage in deterministic tests.
- Final report exists at `.agents/state/zedflow-ai-pi-ai-runtime-drift-final-report.md`.

<a id="legacy-policy"></a>
## Legacy / workaround policy

- No temporary aliases unless explicitly listed as allowed in a task.
- No compatibility shims unless they are the goal of a task.
- No type weakening to satisfy intermediate compilation.
- No preserving legacy names when the planned change is a removal or rename.
- Do not expand a task scope to fix breakages assigned to later units.
- If blocked, report the breakage and reference the downstream unit responsible.
- Do not keep the current builder-only live-provider behavior when the corresponding Pi API performs network execution.
- Do not keep duplicate Rust runtime types as a shortcut; adapt callers to the canonical public type layer.

<a id="breaking-changes"></a>
## Planned Breaking Changes and Propagation Map

| Change | Introduced by | Expected temporary breakage | Fixed by | Forbidden workaround |
|---|---|---|---|---|
| Public stream type becomes real event stream instead of placeholder | R1 | Provider functions and tests typed against placeholder may fail | R2-R4 | Keeping placeholder and adding parallel stream types |
| `models.rs` switches to canonical `types.rs` model/message/context/event types | R2 | Provider registry and compat callers need adaptation | R3-R7 | Duplicating conversion structs for each provider |
| Provider contract gains Pi auth/baseUrl/headers/streamSimple/API dispatch/fallible sources | R3 | Builtin provider factories need rewiring | R4-R6, R9-R12 | Hardcoding provider auth in `Models` |
| Model/auth/refresh paths become async/fallible internally | R4-R5 | Sync tests/callers need `.await` or wrappers | R5-R7, R14 | `block_on` inside core runtime paths |
| Live API functions execute transports instead of returning prepared request placeholders | R9-R12 | Placeholder-specific tests must be replaced | Same owning unit | Reporting success after only building payloads |

<a id="orchestration"></a>
## Subagent Orchestration Plan

Wave W1:
- R1 — Canonical stream contract.

Wave W2:
- R2 — Replace `models.rs` duplicate minimal types.

Wave W3:
- R3 — Pi-compatible provider contract.
- R4 — Runtime auth through `auth::resolve`.

Wave W4:
- R5 — Fallible/async model source and refresh dedupe.

Wave W5:
- R6 — Compat builtin dispatch and option forwarding.
- R7 — Faux provider accounting/event parity.

Wave W6:
- R8 — Image model/provider registry auth/order parity.

Wave W7:
- R9 — OpenRouter images live transport.
- R10 — OpenAI Responses/Completions live transport.

Wave W8:
- R11 — OpenAI Codex SSE/WebSocket live transport.
- R12 — Bedrock ConverseStream live seam.

Wave W9:
- R13 — Public API/facade parity cleanup.

Wave W10:
- R14 — Final audit and live report.

<a id="parallelization-constraints"></a>
## Parallelization Constraints

| Constraint | Reason | Affected units |
|---|---|---|
| R1 before all other implementation | Public stream placeholder blocks correct provider/model/faux behavior. | R2-R14 |
| R2 before provider/auth/compat work | `models.rs` duplicate types are the central drift. | R3-R14 |
| R3 and R4 may run in parallel only with strict file partitioning | Both touch `models.rs`; default is sequential unless orchestrator uses isolated worktrees and merges carefully. | R3, R4 |
| R5 after R3/R4 | Async/fallible refresh depends on provider/auth shape. | R5 |
| R6 and R7 may run in parallel only after R5 if they do not write the same files | Compat and faux share tests sometimes but core files differ. | R6, R7 |
| R9/R10 may run in parallel after R8 | OpenRouter images and OpenAI chat/responses write separate API files. | R9, R10 |
| R11/R12 may run in parallel after R9/R10 | Codex and Bedrock transports are independent after provider contract is stable. | R11, R12 |
| R13 after runtime work | Public facade should expose the final runtime contract, not interim shapes. | R13 |
| R14 last | Owns global validation and live capability report. | R14 |

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
| review-flags | #review-flags | L68-L77 | Review Flags |
| global-acceptance | #global-acceptance | L79-L89 | Global Acceptance Criteria |
| legacy-policy | #legacy-policy | L91-L101 | Legacy / workaround policy |
| breaking-changes | #breaking-changes | L103-L112 | Planned Breaking Changes and Propagation Map |
| orchestration | #orchestration | L114-L149 | Subagent Orchestration Plan |
| parallelization-constraints | #parallelization-constraints | L151-L164 | Parallelization Constraints |
| canonical-line-references | #canonical-line-references | L166-L173 | Canonical Line References |
| phases-and-tasks | #phases-and-tasks | L175-L176 | Phases and Tasks |
| phase-runtime-types | #phase-runtime-types | L178-L309 | Phase 1 — Runtime type convergence |
| R1 | #R1 | L181-L250 | Task R1 — Canonical stream contract |
| R2 | #R2 | L252-L309 | Task R2 — Replace `models.rs` duplicate minimal runtime types |
| phase-provider-auth | #phase-provider-auth | L311-L501 | Phase 2 — Provider and auth runtime parity |
| R3 | #R3 | L314-L374 | Task R3 — Pi-compatible provider contract and API dispatch |
| R4 | #R4 | L376-L439 | Task R4 — Runtime auth via `auth::resolve` |
| R5 | #R5 | L441-L501 | Task R5 — Fallible/async model source and refresh dedupe |
| phase-compat-faux | #phase-compat-faux | L503-L623 | Phase 3 — Compat and faux provider parity |
| R6 | #R6 | L506-L562 | Task R6 — Compat builtin dispatch and option forwarding |
| R7 | #R7 | L564-L623 | Task R7 — Faux provider accounting and typed event parity |
| phase-images | #phase-images | L625-L687 | Phase 4 — Image registry and provider parity |
| R8 | #R8 | L628-L687 | Task R8 — Image model/provider registry auth/order parity |
| phase-live-transports | #phase-live-transports | L689-L935 | Phase 5 — Live provider transports |
| R9 | #R9 | L692-L750 | Task R9 — OpenRouter images live transport |
| R10 | #R10 | L752-L813 | Task R10 — OpenAI Responses and Chat Completions live transport |
| R11 | #R11 | L815-L875 | Task R11 — OpenAI Codex SSE/WebSocket live transport |
| R12 | #R12 | L877-L935 | Task R12 — Bedrock ConverseStream live seam |
| phase-public-api | #phase-public-api | L937-L1063 | Phase 6 — Public API and final audit |
| R13 | #R13 | L940-L999 | Task R13 — Public API/facade parity cleanup |
| R14 | #R14 | L1001-L1063 | Task R14 — Final runtime drift audit and live report |
| pre-finalization-review | #pre-finalization-review | L1065-L1072 | Pre-finalization Review Summary |
<!-- CANONICAL_LINE_REFERENCES_END -->

<a id="phases-and-tasks"></a>
## Phases and Tasks

<a id="phase-runtime-types"></a>
## Phase 1 — Runtime type convergence

<a id="R1"></a>
### Task R1 — Canonical stream contract

Assignable: yes

Execution metadata:
- Wave: W1
- Context: fresh
- Depends on: none
- Can run in parallel with: none
- Must not run in parallel with: R2-R14

Scope boundaries:
- Goal: Make the public Rust `AssistantMessageEventStream` contract match Pi's event stream semantics: async stream, terminal `done/error`, and `result()` final message aggregation.
- Non-goals: Do not implement provider live transports or provider registry changes.
- Forbidden work: Do not keep `types.rs` placeholder stream and create another parallel public stream type.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `crates/zedflow-ai/src/types.rs` | Replace placeholder stream type and verify event wire names. |
| modify | `crates/zedflow-ai/src/utils/event-stream.rs` | Expose/reuse real stream implementation through public type path. |
| modify | `crates/zedflow-ai/tests/stream.rs` | Reactivate provider-free stream contract tests where possible. |
| create/modify | `crates/zedflow-ai/tests/stream-events.rs` | Serde/order/result/abort contract tests if no existing file fits. |
| read | `references/pi/packages/ai/src/utils/event-stream.ts` | Canonical stream mechanics. |
| read | `references/pi/packages/ai/src/types.ts` | Canonical event/message/usage types. |
| read | `references/pi/packages/ai/test/stream.test.ts` | Expected stream result behavior. |
| read | `references/pi/packages/ai/test/abort.test.ts` | Expected abort/error semantics. |

Required context package:
- Plan references: goal, RF-MODELS-CHOKEPOINT, R1.
- Required skills: `/home/zedium/workspaces/zedflow/.agents/skills/rust-skills/SKILL.md`.
- Dependency outputs to read: `.agents/state/port-audit-stream-events.md`, `.agents/state/zedflow-ai-vs-pi-ai-port-audit-summary.md`.
- Neighboring out-of-scope units: R2 provider/model adaptation, R9-R12 live transports.

Implementation outline:
1. Remove the public placeholder split by making `types::AssistantMessageEventStream` wrap or alias the real stream implementation.
2. Preserve Pi event JSON names: `text_start`, `contentIndex`, `toolCall`, terminal `done`/`error`.
3. Add provider-free tests for iteration order, `result()`, terminal error result, and aborted partial preservation.
4. Keep provider-specific stream E2E ignores until R9-R12.

Major snippets:

#### [CANONICAL] Pi event terminal behavior
```text
AssistantMessageEventStream completes on `done` or `error`; `result()` returns the final message for `done` and the final error assistant message for `error`.
```

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; targeted stream/event tests; `cargo test -p zedflow-ai --test stream-events --no-run` if created.
- Must NOT run: live provider tests.

Output contract:
- List changed public stream type paths.
- List reactivated/added stream contract tests.
- List any callers still using legacy placeholder/minimal streams for R2.

Acceptance criteria:
- Public stream type is not an empty placeholder.
- Provider-free Pi stream contract tests pass.
- No duplicate new public stream abstraction is introduced.

Handoff to dependent units:
- R2 must adapt `models.rs` to this canonical stream type.

Subagent prompt:
```text
You are implementing only R1 from .agents/plans/zedflow-ai-pi-ai-runtime-drift-fixes.md. Fresh context. Read R1, the goal/review flags, rust-skills, .agents/state/port-audit-stream-events.md, Pi event-stream/types/stream/abort tests, and the Rust types/event-stream files. Replace the public placeholder AssistantMessageEventStream with the real Pi-equivalent stream contract, add provider-free serde/order/result/abort contract tests, and do not touch provider live transports or models.rs except if needed for compile with a minimal handoff note. Run fmt and targeted stream tests only.
```

<a id="R2"></a>
### Task R2 — Replace `models.rs` duplicate minimal runtime types

Assignable: yes

Execution metadata:
- Wave: W2
- Context: fresh
- Depends on: R1
- Can run in parallel with: none
- Must not run in parallel with: R3-R14

Scope boundaries:
- Goal: Remove the second minimal provider/model/message/stream universe in `models.rs` and adapt it to canonical `crate::types` equivalents.
- Non-goals: Do not add provider auth, async refresh, or live transports yet.
- Forbidden work: Do not preserve duplicate minimal structs by renaming them.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `crates/zedflow-ai/src/models.rs` | Use canonical model/context/message/stream/usage types. |
| modify | `crates/zedflow-ai/src/types.rs` | Add missing fields needed by Pi model shape, such as `base_url`, only if required for type convergence. |
| modify | `crates/zedflow-ai/tests/models-runtime.rs` | Update deterministic tests to canonical types. |
| modify | `crates/zedflow-ai/tests/providers.rs` | Update provider registry tests to canonical types. |
| read | `references/pi/packages/ai/src/models.ts` | Canonical Models and Provider contracts. |
| read | `references/pi/packages/ai/src/types.ts` | Canonical model/context/message shape. |

Required context package:
- Plan references: RF-MODELS-CHOKEPOINT, breaking changes, R2.
- Required skills: rust-skills.
- Dependency outputs to read: R1 output, `.agents/state/port-audit-provider-model-auth.md`, `.agents/state/port-audit-public-api-types.md`.
- Neighboring out-of-scope units: R3 provider contract expansion, R4 auth.

Implementation outline:
1. Replace `models.rs` local `Model`, `StreamOptions`, `AssistantMessage`, and `AssistantMessageEventStream = Vec<_>` with `crate::types` equivalents or narrow private adapters.
2. Preserve existing passing behavior through adapters only where needed, but do not expose duplicate public shapes.
3. Update tests to assert Pi-like rich messages/events instead of minimal text vectors.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; `cargo test -p zedflow-ai --test models-runtime --test providers --no-run`; targeted tests that compile.
- Must NOT run: full workspace cargo gates or live tests.

Output contract:
- List removed duplicate public types and replacement paths.
- List remaining compile blockers assigned to R3/R4/R5.

Acceptance criteria:
- `models.rs` no longer defines public duplicate minimal runtime types that conflict with `types.rs`.
- Targeted model/provider tests compile or documented blockers are assigned to R3/R4/R5.

Handoff to dependent units:
- R3 adds full Pi provider shape on top of the canonical types.

Subagent prompt:
```text
You are implementing only R2 from .agents/plans/zedflow-ai-pi-ai-runtime-drift-fixes.md. Fresh context. Read R2, R1 output, rust-skills, port-audit-provider-model-auth, port-audit-public-api-types, Pi models/types, and Rust models/types. Remove the duplicate minimal public model/message/stream types from models.rs and adapt to canonical crate::types. Do not implement provider auth, async refresh, or live transports. Run fmt and targeted models/providers compile/tests only.
```

<a id="phase-provider-auth"></a>
## Phase 2 — Provider and auth runtime parity

<a id="R3"></a>
### Task R3 — Pi-compatible provider contract and API dispatch

Assignable: yes

Execution metadata:
- Wave: W3
- Context: fresh
- Depends on: R2
- Can run in parallel with: R4 only if file scopes are isolated; default sequential.
- Must not run in parallel with: R5-R14

Scope boundaries:
- Goal: Extend Rust provider definitions to represent Pi provider fields and per-model API dispatch.
- Non-goals: Do not implement OAuth refresh internals or live network transports.
- Forbidden work: Do not hardcode missing provider auth or API dispatch per test.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `crates/zedflow-ai/src/models.rs` | Provider/CreateProviderOptions shape, streamSimple, API dispatch map, missing API stream error. |
| modify | `crates/zedflow-ai/src/providers/static_catalog.rs` | Adapt static provider helper or retire where Pi runtime provider needed. |
| modify | `crates/zedflow-ai/src/providers/all.rs` | Builtin provider registration shape. |
| modify | `crates/zedflow-ai/src/providers/**/*.rs` | Attach provider base URL/API metadata where in scope. |
| modify | `crates/zedflow-ai/tests/providers.rs` | Reactivate mixed API dispatch/missing API tests. |
| read | `references/pi/packages/ai/src/models.ts` | `Provider`, `createProvider`, stream dispatch. |
| read | `references/pi/packages/ai/src/providers/**/*.ts` | Builtin provider factories. |

Required context package:
- Plan references: R3, RF-MODELS-CHOKEPOINT.
- Required skills: rust-skills.
- Dependency outputs to read: R1/R2 outputs, `.agents/state/port-audit-provider-model-auth.md`.
- Neighboring out-of-scope units: R4 auth resolver, R9-R12 live transports.

Implementation outline:
1. Add provider fields equivalent to Pi: auth placeholder field, base URL, headers, stream, streamSimple, model source, refresh source.
2. Add single implementation vs by-API dispatch representation.
3. Missing model API must produce Pi-shaped terminal stream error, not panic or coarse success.
4. Adapt builtins just enough to carry metadata; live API handlers can remain implementation-blocked until R9-R12.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; targeted `providers` and `models-runtime` tests affected by provider shape.
- Must NOT run: live provider tests.

Output contract:
- List new provider fields and dispatch behavior.
- List providers still static shells for R9-R12.

Acceptance criteria:
- Mixed API provider dispatch is representable and tested.
- Missing API implementation returns Pi-shaped stream error.
- No hardcoded per-test provider dispatch remains.

Handoff to dependent units:
- R4 fills provider auth resolution into this provider shape.

Subagent prompt:
```text
You are implementing only R3 from .agents/plans/zedflow-ai-pi-ai-runtime-drift-fixes.md. Fresh context. Read R3, R2 output, rust-skills, port-audit-provider-model-auth, Pi models.ts createProvider/provider factories, and Rust models/providers. Extend the provider contract to Pi-compatible metadata and per-API dispatch. Do not implement OAuth refresh or live transports. Run fmt and targeted provider/model tests only.
```

<a id="R4"></a>
### Task R4 — Runtime auth via `auth::resolve`

Assignable: yes

Execution metadata:
- Wave: W3
- Context: fresh
- Depends on: R2
- Can run in parallel with: R3 only if file scopes are isolated; default sequential after R3.
- Must not run in parallel with: R5-R14

Scope boundaries:
- Goal: Route `Models::get_auth` and stream auth application through the shared Pi-like `auth::resolve` implementation instead of hardcoded provider branches.
- Non-goals: Do not implement async refresh dedupe; R5 owns that.
- Forbidden work: Do not use `block_on` inside core runtime auth paths as a permanent fix.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `crates/zedflow-ai/src/models.rs` | Use provider auth metadata and `resolve_provider_auth`; apply auth to stream options/model. |
| modify | `crates/zedflow-ai/src/auth/resolve.rs` | Fill missing Pi parity only if resolver itself differs. |
| modify | `crates/zedflow-ai/src/auth/types.rs` | Provider auth trait/type adjustments. |
| modify | `crates/zedflow-ai/src/auth/credential-store.rs` | Store hooks needed for deterministic auth tests. |
| modify | `crates/zedflow-ai/tests/oauth-auth.rs` | Stored credential/OAuth auth parity. |
| modify | `crates/zedflow-ai/tests/models-runtime.rs` | Request auth/env/baseUrl merge parity. |
| read | `references/pi/packages/ai/src/auth/resolve.ts` | Canonical resolver. |
| read | `references/pi/packages/ai/src/auth/types.ts` | Credential/auth contracts. |
| read | `references/pi/packages/ai/src/models.ts` | `Models.getAuth` and stream auth application. |

Required context package:
- Plan references: R4, RF-ASYNC-RUST-SURFACE.
- Required skills: rust-skills.
- Dependency outputs to read: R2/R3 outputs if available, `.agents/state/port-audit-provider-model-auth.md`.
- Neighboring out-of-scope units: R5 async refresh/dedupe.

Implementation outline:
1. Attach provider auth metadata from R3 to `Models::get_auth`.
2. Use `auth::resolve::resolve_provider_auth` as the single auth path for stored credentials, OAuth, env, and request overrides.
3. Apply resolved auth to stream options/model like Pi: explicit request fields win, resolved headers/env merge, resolved base URL mutates cloned request model where applicable.
4. If async signatures are required, introduce them intentionally and report sync callers for R5/R6.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; targeted `oauth-auth`, `models-runtime`, `providers` tests.
- Must NOT run: live provider/OAuth endpoints.

Output contract:
- List hardcoded auth branches removed or retained with reason.
- List auth tests reactivated.
- List remaining async refresh blockers for R5.

Acceptance criteria:
- `Models::get_auth` uses provider auth resolver, not provider-id hardcoding.
- Stored credential precedence and ambient fallback behavior match Pi deterministic tests.
- No secrets are logged in tests.

Handoff to dependent units:
- R5 adds async refresh/dedupe over this resolver path.

Subagent prompt:
```text
You are implementing only R4 from .agents/plans/zedflow-ai-pi-ai-runtime-drift-fixes.md. Fresh context. Read R4, R2/R3 outputs if present, rust-skills, Pi auth/resolve/types/models, and Rust auth/models. Route Models auth and stream auth application through auth::resolve instead of hardcoded provider branches. Do not run live OAuth/provider endpoints. Run fmt and targeted oauth-auth/models-runtime/providers tests.
```

<a id="R5"></a>
### Task R5 — Fallible/async model source and refresh dedupe

Assignable: yes

Execution metadata:
- Wave: W4
- Context: fresh
- Depends on: R3, R4
- Can run in parallel with: none
- Must not run in parallel with: R6-R14

Scope boundaries:
- Goal: Port Pi `getModels` failure handling and async `refreshModels` in-flight dedupe semantics.
- Non-goals: Do not implement live provider transports.
- Forbidden work: Do not model provider source failure as panics.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `crates/zedflow-ai/src/models.rs` | Fallible model source, async refresh, in-flight dedupe, all-settled refresh. |
| modify | `crates/zedflow-ai/tests/models-runtime.rs` | Reactivate source failure, refresh error, OAuth refresh/dedupe tests. |
| modify | `crates/zedflow-ai/tests/providers.rs` | Reactivate dynamic provider refresh/dedupe tests. |
| modify | `crates/zedflow-ai/tests/images-models.rs` | Coordinate similar image registry expectations if shared helpers exist. |
| read | `references/pi/packages/ai/src/models.ts` | `getModels`, `refresh`, createProvider dedupe. |
| read | `references/pi/packages/ai/test/models-runtime.test.ts` | Canonical failure/dedupe tests. |
| read | `references/pi/packages/ai/test/providers.test.ts` | Dynamic provider tests. |

Required context package:
- Plan references: R5, RF-ASYNC-RUST-SURFACE.
- Required skills: rust-skills.
- Dependency outputs to read: R3/R4 outputs.
- Neighboring out-of-scope units: R8 image registry full parity if image files require larger changes.

Implementation outline:
1. Represent provider model sources as `Result<Vec<Model>, ModelsError>` rather than infallible vectors.
2. Make refresh source async/future-backed where needed; preserve a convenient sync wrapper only outside core runtime if required.
3. Implement Pi all-provider refresh all-settled semantics and single-provider error wrapping.
4. Implement in-flight refresh dedupe per provider.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; targeted `models-runtime`, `providers`, relevant `images-models` tests.
- Must NOT run: live provider tests.

Output contract:
- List reactivated model source/refresh tests.
- List any remaining sync API compatibility wrappers and why.

Acceptance criteria:
- Throwing/failing model sources are representable without panics.
- Concurrent refresh dedupe behavior is tested.
- Deterministic OAuth refresh tests use fake hooks, not live endpoints.

Handoff to dependent units:
- R6 compat and R8 images consume stable refresh/model source semantics.

Subagent prompt:
```text
You are implementing only R5 from .agents/plans/zedflow-ai-pi-ai-runtime-drift-fixes.md. Fresh context. Read R5, R3/R4 outputs, rust-skills, Pi models/providers tests and source. Add fallible model sources, async refresh, all-settled refresh, and in-flight dedupe. Do not run live providers. Run fmt and targeted models-runtime/providers/images-models tests.
```

<a id="phase-compat-faux"></a>
## Phase 3 — Compat and faux provider parity

<a id="R6"></a>
### Task R6 — Compat builtin dispatch and option forwarding

Assignable: yes

Execution metadata:
- Wave: W5
- Context: fresh
- Depends on: R5
- Can run in parallel with: R7 only if no same-file writes; default sequential.
- Must not run in parallel with: R8-R14

Scope boundaries:
- Goal: Restore Pi compat behavior: builtin short-circuit through `Models` and option forwarding for builtins/custom providers.
- Non-goals: Do not implement faux accounting; R7 owns that.
- Forbidden work: Do not drop caller options in provider wrappers.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `crates/zedflow-ai/src/compat.rs` | Builtin dispatch, option forwarding, registry behavior. |
| modify | `crates/zedflow-ai/tests/providers.rs` | Compat provider behavior tests if colocated. |
| modify | `crates/zedflow-ai/tests/models-runtime.rs` | Compat/model interaction tests if needed. |
| read | `references/pi/packages/ai/src/compat.ts` | Canonical compat dispatch. |
| read | `references/pi/packages/ai/test/providers.test.ts` | Provider/compat expectations. |

Required context package:
- Plan references: R6.
- Required skills: rust-skills.
- Dependency outputs to read: R3-R5 outputs, `.agents/state/port-audit-compat-faux-accounting.md`.
- Neighboring out-of-scope units: R7 faux internals.

Implementation outline:
1. Implement Pi `shouldUseBuiltinModels` equivalent where applicable.
2. Ensure `stream`, `streamSimple`, `complete`, and `completeSimple` forward options.
3. Keep custom/faux provider registration and unregister source-id semantics intact.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; targeted compat/provider/model tests.
- Must NOT run: live provider tests.

Output contract:
- List compat paths now forwarding options.
- List builtin short-circuit behavior and tests.

Acceptance criteria:
- Caller auth/env/session/cache options are not silently dropped.
- Builtin/custom/faux dispatch matches Pi deterministic tests.

Handoff to dependent units:
- R9-R11 live tests use compat dispatch once transports exist.

Subagent prompt:
```text
You are implementing only R6 from .agents/plans/zedflow-ai-pi-ai-runtime-drift-fixes.md. Fresh context. Read R6, R3-R5 outputs, rust-skills, Pi compat.ts and provider tests, and Rust compat/models. Restore Pi compat builtin dispatch and option forwarding. Do not implement faux accounting or live transports. Run fmt and targeted compat/provider/model tests.
```

<a id="R7"></a>
### Task R7 — Faux provider accounting and typed event parity

Assignable: yes

Execution metadata:
- Wave: W5
- Context: fresh
- Depends on: R5, R1
- Can run in parallel with: R6 only if no same-file writes; default sequential.
- Must not run in parallel with: R8-R14

Scope boundaries:
- Goal: Port Pi faux provider usage/cache/session accounting and typed event output to Rust.
- Non-goals: Do not implement live provider transports.
- Forbidden work: Do not keep opaque faux events when typed public stream events are available.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `crates/zedflow-ai/src/providers/faux.rs` | Serialized-context usage/cache, typed events, panic/error behavior. |
| modify | `crates/zedflow-ai/tests/faux-provider.rs` | Reactivate Pi faux tests. |
| modify | `crates/zedflow-ai/src/types.rs` | Add missing usage/cost fields only if required by Pi faux accounting. |
| read | `references/pi/packages/ai/src/providers/faux.ts` | Canonical faux behavior. |
| read | `references/pi/packages/ai/test/faux-provider.test.ts` | Canonical faux tests. |
| read | `references/pi/packages/ai/src/models.ts` | `calculateCost` and usage/cost semantics. |

Required context package:
- Plan references: R7.
- Required skills: rust-skills.
- Dependency outputs to read: R1/R5 outputs, `.agents/state/port-audit-compat-faux-accounting.md`.
- Neighboring out-of-scope units: R6 compat registry.

Implementation outline:
1. Serialize context like Pi for input token estimation.
2. Implement per-session prompt cache common-prefix read/write simulation.
3. Include cache writes in total tokens and cost where Pi does.
4. Emit typed `thinking_*`, `text_*`, `toolcall_*`, `done`, and `error` events.
5. Preserve Rust-safe behavior for panic-to-error event conversion.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; targeted `faux-provider` tests.
- Must NOT run: live provider tests.

Output contract:
- List reactivated faux tests.
- List any async factory/abort timing behavior still not representable and why.

Acceptance criteria:
- Faux usage/cache accounting matches Pi deterministic tests.
- Faux stream events are typed public events, not opaque placeholders.

Handoff to dependent units:
- R14 includes any remaining JS/async-only faux residuals in final report.

Subagent prompt:
```text
You are implementing only R7 from .agents/plans/zedflow-ai-pi-ai-runtime-drift-fixes.md. Fresh context. Read R7, R1/R5 outputs, rust-skills, Pi faux source/tests, Pi calculateCost, and Rust faux/types. Port faux serialized-context usage/cache/session accounting and typed stream events. Do not implement live transports. Run fmt and targeted faux-provider tests.
```

<a id="phase-images"></a>
## Phase 4 — Image registry and provider parity

<a id="R8"></a>
### Task R8 — Image model/provider registry auth/order parity

Assignable: yes

Execution metadata:
- Wave: W6
- Context: fresh
- Depends on: R5, R6
- Can run in parallel with: none
- Must not run in parallel with: R9-R14

Scope boundaries:
- Goal: Bring Rust image model/provider registry behavior in line with Pi `images-models.ts` and OpenRouter image provider auth/order behavior.
- Non-goals: Do not implement OpenRouter live image transport; R9 owns transport.
- Forbidden work: Do not use unordered `HashMap` iteration where Pi specifies insertion order.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `crates/zedflow-ai/src/images-models.rs` | Ordered providers, auth resolver, refresh/dedupe if applicable. |
| modify | `crates/zedflow-ai/src/providers/all.rs` | Builtin image provider registration. |
| modify | `crates/zedflow-ai/src/api/openrouter-images.rs` | Auth/base URL/request option hooks needed for registry, not network. |
| modify | `crates/zedflow-ai/tests/images-models.rs` | Reactivate image model/provider tests. |
| read | `references/pi/packages/ai/src/images-models.ts` | Canonical image registry. |
| read | `references/pi/packages/ai/src/providers/all.ts` | Image provider registration. |
| read | `references/pi/packages/ai/src/api/openrouter-images.ts` | OpenRouter image behavior. |

Required context package:
- Plan references: R8.
- Required skills: rust-skills.
- Dependency outputs to read: R5/R6 outputs, `.agents/state/port-audit-tests-residuals.md`.
- Neighboring out-of-scope units: R9 live transport.

Implementation outline:
1. Preserve provider insertion order.
2. Route image provider auth through the same provider auth resolver semantics as chat where possible.
3. Implement deterministic env/options/base URL merge for image generation without live network.
4. Keep transport success/failure for R9.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; targeted `images-models` and deterministic `images` tests.
- Must NOT run: live OpenRouter image tests.

Output contract:
- List image registry tests reactivated.
- List transport-only blockers for R9.

Acceptance criteria:
- Image provider ordering/auth/env deterministic tests pass.
- OpenRouter live image tests remain only transport-blocked.

Handoff to dependent units:
- R9 implements OpenRouter image network execution.

Subagent prompt:
```text
You are implementing only R8 from .agents/plans/zedflow-ai-pi-ai-runtime-drift-fixes.md. Fresh context. Read R8, R5/R6 outputs, rust-skills, Pi images-models/providers/all/openrouter-images, and Rust images-models/providers/openrouter-images. Port image provider order/auth/env registry behavior. Do not implement live image network transport. Run fmt and targeted images-models/images deterministic tests.
```

<a id="phase-live-transports"></a>
## Phase 5 — Live provider transports

<a id="R9"></a>
### Task R9 — OpenRouter images live transport

Assignable: yes

Execution metadata:
- Wave: W7
- Context: fresh
- Depends on: R8
- Can run in parallel with: R10
- Must not run in parallel with: R11-R14

Scope boundaries:
- Goal: Replace OpenRouter image request-prepared error path with Pi-equivalent OpenAI-compatible live transport and response parsing.
- Non-goals: Do not implement Codex, Bedrock, or OpenAI chat transports.
- Forbidden work: Do not mark live image tests passed after only building the request.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `crates/zedflow-ai/src/api/openrouter-images.rs` | Live HTTP transport, hooks, response parser wiring, error body passthrough. |
| modify | `crates/zedflow-ai/tests/images.rs` | Capability-gated live OpenRouter image tests. |
| modify | `crates/zedflow-ai/tests/provider-error-body-passthrough.rs` | Deterministic/live split. |
| read | `references/pi/packages/ai/src/api/openrouter-images.ts` | Canonical live image path. |
| read | `references/pi/packages/ai/test/openrouter-*.test.ts` | OpenRouter image/live behavior. |

Required context package:
- Plan references: R9, RF-LIVE-CREDENTIALS.
- Required skills: rust-skills.
- Dependency outputs to read: R8 output, P7 final output `.agents/state/subagent-p7-output.md` if available, `.agents/state/port-audit-api-transports.md`.
- Neighboring out-of-scope units: R10-R12 other transports.

Implementation outline:
1. Wire request builder to actual HTTP client with redacted headers in errors/logs.
2. Run `onPayload`/`onResponse` equivalents if exposed in Rust options.
3. Parse success into `AssistantImages` with text/image/usage/responseId.
4. Preserve provider error body passthrough.
5. Execute live only when OpenRouter capability helper detects credentials.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; deterministic OpenRouter image tests; capability-gated OpenRouter live image tests if credentials exist.
- Must NOT run: unrelated live providers.

Output contract:
- Live report: executed/skipped/failed OpenRouter image tests.
- Redaction confirmation.

Acceptance criteria:
- Public OpenRouter image generation no longer always returns implementation-blocker error when credentials are present.
- Deterministic error-body and parser tests pass.

Handoff to dependent units:
- R14 includes live result.

Subagent prompt:
```text
You are implementing only R9 from .agents/plans/zedflow-ai-pi-ai-runtime-drift-fixes.md. Fresh context. Read R9, R8 output, rust-skills, port-audit-api-transports, Pi openrouter-images source/tests, and Rust openrouter-images/images tests. Implement OpenRouter image live transport and parser wiring. Run deterministic tests and live OpenRouter image tests only if credentials are detected; do not log secrets or run unrelated providers.
```

<a id="R10"></a>
### Task R10 — OpenAI Responses and Chat Completions live transport

Assignable: yes

Execution metadata:
- Wave: W7
- Context: fresh
- Depends on: R8
- Can run in parallel with: R9
- Must not run in parallel with: R11-R14

Scope boundaries:
- Goal: Connect OpenAI Responses and Chat Completions builders/parsers to Pi-equivalent live transport and hooks.
- Non-goals: Do not implement Codex SSE/WebSocket; R11 owns Codex.
- Forbidden work: Do not replace stream event assertions with coarse success checks.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `crates/zedflow-ai/src/api/openai-responses.rs` | Live Responses transport, hooks, event parsing. |
| modify | `crates/zedflow-ai/src/api/openai-completions.rs` | Live Chat Completions transport, chunk processing. |
| modify | `crates/zedflow-ai/tests/openai-completions-response-model.rs` | Response model/chunk tests. |
| modify | `crates/zedflow-ai/tests/openai-completions-tool-choice.rs` | Stream/usage tests if affected. |
| modify | `crates/zedflow-ai/tests/responseid.rs` | OpenAI responseId live/capability tests. |
| read | `references/pi/packages/ai/src/api/openai-responses.ts` | Canonical Responses transport. |
| read | `references/pi/packages/ai/src/api/openai-completions.ts` | Canonical Chat transport. |
| read | `references/pi/packages/ai/test/openai-*.test.ts` | OpenAI tests. |

Required context package:
- Plan references: R10, RF-LIVE-CREDENTIALS.
- Required skills: rust-skills.
- Dependency outputs to read: R1-R8 outputs, `.agents/state/port-audit-api-transports.md`.
- Neighboring out-of-scope units: R11 Codex.

Implementation outline:
1. Preserve existing request builder parity.
2. Add HTTP/SSE transport path with `onPayload`/`onResponse` semantics.
3. Feed provider chunks into canonical stream events from R1.
4. Preserve responseId, usage, reasoning, tool-call delta behavior.
5. Gate live tests by OpenAI/OpenRouter capabilities as appropriate.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; targeted deterministic OpenAI tests; capability-gated live only when credentials exist.
- Must NOT run: Codex/Bedrock live tests.

Output contract:
- List transports wired and tests reactivated.
- Live execution report.

Acceptance criteria:
- OpenAI public stream functions execute provider responses when credentials/network are available.
- Deterministic stream/chunk/responseId tests pass.

Handoff to dependent units:
- R11 may reuse OpenAI auth/session helpers for Codex.

Subagent prompt:
```text
You are implementing only R10 from .agents/plans/zedflow-ai-pi-ai-runtime-drift-fixes.md. Fresh context. Read R10, prior outputs, rust-skills, port-audit-api-transports, Pi OpenAI Responses/Completions source/tests, and Rust OpenAI API files/tests. Wire live OpenAI Responses and Chat Completions transports into canonical streams while preserving builders. Run deterministic tests and capability-gated OpenAI/OpenRouter live tests only when credentials exist.
```

<a id="R11"></a>
### Task R11 — OpenAI Codex SSE/WebSocket live transport

Assignable: yes

Execution metadata:
- Wave: W8
- Context: fresh
- Depends on: R10
- Can run in parallel with: R12
- Must not run in parallel with: R13-R14

Scope boundaries:
- Goal: Implement Pi-equivalent Codex SSE/WebSocket live execution, fallback, cached session behavior, and responseId/cache-affinity paths.
- Non-goals: Do not implement Bedrock or OpenRouter images.
- Forbidden work: Do not treat prepared request envelopes as live execution.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `crates/zedflow-ai/src/api/openai-codex-responses.rs` | SSE/WebSocket transport, fallback, cache/session handling. |
| modify | `crates/zedflow-ai/tests/openai-codex-stream.rs` | Deterministic/live split for Codex stream. |
| modify | `crates/zedflow-ai/tests/openai-codex-cache-affinity-e2e.rs` | Capability-gated live cache affinity. |
| modify | `crates/zedflow-ai/tests/codex-websocket-cached-probe.rs` | Capability-gated live WS cache probe. |
| modify | `crates/zedflow-ai/tests/responseid.rs` | Codex responseId. |
| read | `references/pi/packages/ai/src/api/openai-codex-*.ts` | Canonical Codex behavior. |
| read | `references/pi/packages/ai/test/openai-codex-*.test.ts` | Codex tests. |

Required context package:
- Plan references: R11, RF-LIVE-CREDENTIALS.
- Required skills: rust-skills.
- Dependency outputs to read: R10 output, P6/P7 outputs if available, `.agents/state/port-audit-api-transports.md`.
- Neighboring out-of-scope units: R12 Bedrock.

Implementation outline:
1. Use stored Codex OAuth credentials from auth storage without printing secrets.
2. Execute SSE stream and parse into canonical events.
3. Implement WebSocket connection, open timeout, idle timeout, fallback, reconnect, cache/session behavior matching deterministic P4 cases.
4. Run live tests only when Codex capability helper reports available credentials.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; Codex deterministic tests; Codex live tests if credentials exist.
- Must NOT run: unrelated live providers.

Output contract:
- Codex SSE/WS live report.
- Redaction confirmation.
- Remaining Codex blockers, if any.

Acceptance criteria:
- Codex live network tests execute when credentials are present and network is available.
- Deterministic fallback/cache/session tests still pass.

Handoff to dependent units:
- R14 includes live results.

Subagent prompt:
```text
You are implementing only R11 from .agents/plans/zedflow-ai-pi-ai-runtime-drift-fixes.md. Fresh context. Read R11, R10 output, rust-skills, Pi Codex source/tests, Rust openai-codex-responses and Codex tests, and prior P6/P7 outputs if present. Implement Codex SSE/WebSocket live transport, fallback, cache/session and responseId behavior. Run deterministic Codex tests and live Codex tests only if credentials are detected; do not log secrets.
```

<a id="R12"></a>
### Task R12 — Bedrock ConverseStream live seam

Assignable: yes

Execution metadata:
- Wave: W8
- Context: fresh
- Depends on: R10
- Can run in parallel with: R11
- Must not run in parallel with: R13-R14

Scope boundaries:
- Goal: Replace Bedrock empty/plan-only stream placeholder with Pi-equivalent ConverseStream send and event processing behind capability gates.
- Non-goals: Do not force live AWS tests when credentials are unavailable.
- Forbidden work: Do not substitute genai-normalized behavior for Pi Bedrock behavior.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `crates/zedflow-ai/src/api/bedrock-converse-stream.rs` | AWS SDK transport/event processing. |
| modify | `crates/zedflow-ai/src/bedrock-provider.rs` | Region/auth behavior if needed. |
| modify | `crates/zedflow-ai/tests/bedrock-*.rs` | Deterministic/live split and live gating. |
| read | `references/pi/packages/ai/src/api/bedrock-converse-stream.ts` | Canonical AWS transport. |
| read | `references/pi/packages/ai/src/bedrock-provider.ts` | Region/auth behavior. |
| read | `references/pi/packages/ai/test/bedrock-*.test.ts` | Bedrock tests. |

Required context package:
- Plan references: R12.
- Required skills: rust-skills.
- Dependency outputs to read: R3-R5/R10 outputs, `.agents/state/port-audit-api-transports.md`.
- Neighboring out-of-scope units: R11 Codex.

Implementation outline:
1. Preserve existing deterministic payload/header conversion tests.
2. Add AWS ConverseStream send path with Pi hook/error/event semantics.
3. Map AWS event stream into canonical `AssistantMessageEventStream`.
4. Gate live AWS tests by explicit capability detection.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; targeted Bedrock deterministic tests; live AWS only if capability helper detects credentials and tests are assigned.
- Must NOT run: unavailable live provider suites.

Output contract:
- List Bedrock placeholders removed.
- Live AWS skipped/executed report.

Acceptance criteria:
- Bedrock public stream no longer returns empty placeholder for implemented capability paths.
- Deterministic Bedrock tests still pass.

Handoff to dependent units:
- R14 final audit includes Bedrock live/manual status.

Subagent prompt:
```text
You are implementing only R12 from .agents/plans/zedflow-ai-pi-ai-runtime-drift-fixes.md. Fresh context. Read R12, prior outputs, rust-skills, Pi Bedrock source/tests, and Rust Bedrock files/tests. Replace Bedrock plan-only/empty stream placeholder with ConverseStream transport and event mapping while preserving deterministic payload parity. Run fmt, targeted Bedrock deterministic tests, and live AWS only when capability-gated credentials are present.
```

<a id="phase-public-api"></a>
## Phase 6 — Public API and final audit

<a id="R13"></a>
### Task R13 — Public API/facade parity cleanup

Assignable: yes

Execution metadata:
- Wave: W9
- Context: fresh
- Depends on: R11, R12
- Can run in parallel with: none
- Must not run in parallel with: R14

Scope boundaries:
- Goal: Codify a Pi-compatible public facade and ensure internals such as `genai` remain private.
- Non-goals: Do not remove useful Rust-specific exports unless they conflict with Pi parity or leak internals.
- Forbidden work: Do not hide runtime drift behind facade-only re-exports.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `crates/zedflow-ai/src/lib.rs` | Public module/export surface. |
| modify | `crates/zedflow-ai/src/index.rs` | Pi-style facade or remove marker ambiguity. |
| modify/create | `crates/zedflow-ai/tests/public-api.rs` | Public facade/no-genai/type compile tests if appropriate. |
| read | `references/pi/packages/ai/src/index.ts` | Canonical root exports. |
| read | `references/pi/packages/ai/package.json` | Public subpath exports. |
| read | `crates/zedflow-ai/src/utils/genai-backend.rs` | Internal genai boundary. |

Required context package:
- Plan references: R13, RF-PUBLIC-API, RF-JS-ONLY.
- Required skills: rust-skills.
- Dependency outputs to read: all prior unit outputs, `.agents/state/port-audit-public-api-types.md`.
- Neighboring out-of-scope units: R14 final audit.

Implementation outline:
1. Add or clarify Pi-compatible root facade exports.
2. Keep `genai_backend` and dependency-specific types crate-private.
3. Document or test JS-only dynamic import observability as non-portable while preserving side-effect-free Rust alternatives.
4. Avoid breaking internal modules unless explicitly required for public parity.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; `cargo check -p zedflow-ai --all-targets`; public API/no-genai audit grep.
- Must NOT run: live provider tests.

Output contract:
- List public facade changes.
- Genai leak audit result.
- JS-only cases documented.

Acceptance criteria:
- Public facade is intentional and Pi-compatible where required.
- No public `genai` type/export leak.

Handoff to dependent units:
- R14 performs final global validation.

Subagent prompt:
```text
You are implementing only R13 from .agents/plans/zedflow-ai-pi-ai-runtime-drift-fixes.md. Fresh context. Read R13, prior outputs, rust-skills, port-audit-public-api-types, Pi index.ts/package.json, and Rust lib/index/genai backend. Codify a Pi-compatible public facade and ensure genai types remain private. Do not implement runtime behavior or live transports. Run fmt, cargo check all-targets, and public API/genai audits.
```

<a id="R14"></a>
### Task R14 — Final runtime drift audit and live report

Assignable: yes

Execution metadata:
- Wave: W10
- Context: fresh
- Depends on: R1, R2, R3, R4, R5, R6, R7, R8, R9, R10, R11, R12, R13
- Can run in parallel with: none
- Must not run in parallel with: all other write units

Scope boundaries:
- Goal: Verify runtime drift fixes, deterministic gates, ignore reasons, public API/no-genai status, and capability-gated live results.
- Non-goals: Do not implement new behavior except small metadata fixes found during audit.
- Forbidden work: Do not hide failures by adding ignores without matrix-backed justification.

Files:
| Action | Path | Purpose |
|---|---|---|
| create | `.agents/state/zedflow-ai-pi-ai-runtime-drift-final-report.md` | Final report. |
| modify if needed | `crates/zedflow-ai/tests/**/*.rs` | Remove stale ignore wording only if metadata-only. |
| read | `.agents/state/zedflow-ai-pi-ai-final-parity-report.md` | Previous final baseline. |
| read | `.agents/state/zedflow-ai-vs-pi-ai-port-audit-summary.md` | Drift audit baseline. |
| read | `.agents/state/port-audit-*.md` | Detailed drift reports. |

Required context package:
- Plan references: global acceptance, review flags, R14.
- Required skills: rust-skills.
- Dependency outputs to read: all prior unit outputs.
- Neighboring out-of-scope units: none.

Implementation outline:
1. Run final placeholder grep.
2. Audit ignored tests and ensure every ignore is live/manual/capability/JS-only/upstream-skipped or explicitly accepted.
3. Audit public `genai` type leaks.
4. Run deterministic cargo gates.
5. Run capability-gated OpenRouter/Codex live tests and any implemented provider live tests when credentials are detected.
6. Write final report with before/after drift status.

Validation responsibility:
- Type: integration-validating
- Must run: `grep -R "PORT PLACEHOLDER" crates/zedflow-ai/src crates/zedflow-ai/tests`; ignore audit; genai public leak audit; `cargo fmt --all --check`; `cargo check -p zedflow-ai --all-targets`; `cargo test -p zedflow-ai --all-targets`; capability-gated live commands for implemented live providers.
- Must NOT run: unavailable live provider suites.

Output contract:
- Path to `.agents/state/zedflow-ai-pi-ai-runtime-drift-final-report.md`.
- Validation command results.
- Live capability report.
- Remaining ignored tests and reasons.
- Explicit verdict against global acceptance.

Acceptance criteria:
- Global acceptance criteria are satisfied, or the report explicitly lists remaining blockers without claiming success.
- No unresolved local deterministic Pi behavior remains ignored without a real accepted reason.

Handoff to dependent units:
- None. This is final.

Subagent prompt:
```text
You are implementing only R14 from .agents/plans/zedflow-ai-pi-ai-runtime-drift-fixes.md. Fresh context. Read all prior unit outputs, this plan's global acceptance/review flags, rust-skills, the previous final parity report, and the port audit reports. Run final PORT PLACEHOLDER audit, ignore audit, public genai leak audit, deterministic cargo gates, and capability-gated live tests for implemented providers when credentials are detected. Do not run unavailable live provider suites. Write .agents/state/zedflow-ai-pi-ai-runtime-drift-final-report.md with command results, live report, remaining ignores, and an explicit verdict.
```

<a id="pre-finalization-review"></a>
## Pre-finalization Review Summary

| Reviewer | Status | Required changes applied | Remaining concerns |
|---|---|---|---|
| Feasibility / file references | Passed | Plan uses audited exact files from `.agents/state/port-audit-*.md` and prior parity reports. | Line numbers may drift as implementation changes; subagents must read current files before editing. |
| Sequencing / dependency graph | Passed | Runtime stream and models chokepoints run before provider/auth/compat/live transports; final audit last. | R3/R4 and R6/R7 default to sequential because of likely shared files. |
| Scope isolation / prompt quality | Passed | Each assignable unit has fresh prompt, exact scope, forbidden work, validations, and handoff. | Large units touch central files; if blocked, report rather than widening scope. |
