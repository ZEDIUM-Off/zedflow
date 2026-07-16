<!-- migration-document-status: SUPERSEDED -->
> [!CAUTION]
> **Migration status: SUPERSEDED.** Historical plan only. Use `.agents/plans/zedflow-ai-agent-pi-fidelity-consolidation.md` and `.agents/state/zedflow-ai-agent-pi-fidelity-current-status.md` for current work.

# Zedflow AI Pi-AI Parity Finalization

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

Finalize `crates/zedflow-ai` as a faithful Rust port of `references/pi/packages/ai` behavior after dependency replacement. The remaining test placeholders and ignored parity tests must be treated as Pi behavior specifications, not cleanup labels. Port the observable Pi behavior into Rust: request payloads, headers, auth/env resolution, provider/model metadata, stream event order, error formatting, usage/cost/cache/session accounting, OAuth flows, and live provider integration where credentials are available.

The Rust test suite must align with the Pi test suite under `references/pi/packages/ai/test/*.test.ts`. Source-file Rust unit tests are acceptable only for pure local helper logic. Any mock, fake, simulated provider, fake transport, fake OAuth endpoint, fake timer, or fixture-driven behavior must live in dedicated files under `crates/zedflow-ai/tests/` or `crates/zedflow-ai/tests/common/`, not in source modules.

<a id="non-goals"></a>
## Non-goals

- Do not rename `PORT PLACEHOLDER` text away without implementing the corresponding Pi behavior or proving the test is truly live/manual-only.
- Do not weaken tests to match current Rust behavior when Pi behavior is stricter.
- Do not replace Pi test shape with a new Rust-only convention unless the observable behavior remains equivalent and the mapping is documented.
- Do not expose TypeScript dependency types or `genai` types in public `zedflow-ai` APIs.
- Do not add broad compatibility shims just to pass compile gates.
- Do not put mock/fake provider implementations in production source modules unless the production API itself requires a Pi-equivalent faux provider.
- Do not run live provider tests that require unavailable credentials; do not log secrets.

<a id="review-flags"></a>
## Review Flags

| ID | Type | Severity | Summary | Affects | Follow-up / accepted risk |
|---|---|---:|---|---|---|
| RF-LIVE-CREDENTIALS | OQ | Medium | Available live credentials are partial: user stated OpenAI Codex subscription and OpenRouter key are available, other providers may not be. | P7, P8 | Detect capabilities exactly from Pi-equivalent env/credential store; run live tests only when present and skip with explicit message when absent. |
| RF-PI-TEST-SHAPE | R | High | Pi tests use Vitest/JS mocks; Rust must reproduce observable behavior, not necessarily mock mechanics. | All units | Each reactivated Rust test must cite its Pi TS test/source behavior reference. |
| RF-MOCK-LOCATION | R | Medium | User requires mocks/simulations in dedicated test files, while simple pure unit tests may remain inline. | P2-P6 | Move fixture/fake transport tests to `tests/` or `tests/common/`; keep source tests only for pure helpers. |
| RF-NO-RENAMING-AS-FIX | R | High | Removing placeholder wording without behavior parity would hide incomplete porting. | All units | Acceptance requires behavior implementation or explicit live/manual gating. |
| RF-LIVE-NOT-OPTIONAL | R | Medium | Live tests should not be blanket-ignored; Codex/OpenRouter should run when credentials exist. | P7, P8 | Add capability-gated live execution and document skipped providers by missing credential only. |

<a id="global-acceptance"></a>
## Global Acceptance Criteria

- `grep -R "PORT PLACEHOLDER" crates/zedflow-ai/src crates/zedflow-ai/tests` returns zero matches.
- Every remaining ignored test is either a real manual/browser interaction or requires credentials/network for a provider that is unavailable in the current environment; no ignored test says local behavior is unported.
- For every formerly placeholdered deterministic test, the Rust behavior is mapped to and checked against the corresponding Pi TS test/source behavior.
- Mock/fake/simulated behavior lives under `crates/zedflow-ai/tests/` or `crates/zedflow-ai/tests/common/`, not production source modules, except for the Pi-equivalent production faux provider API.
- OpenRouter and OpenAI Codex live tests run when their Pi-equivalent credentials are available; otherwise they skip with explicit missing-capability messages and no secret logging.
- `cargo fmt --all --check`, `cargo check -p zedflow-ai --all-targets`, and deterministic `cargo test -p zedflow-ai --all-targets` pass.
- The final live test report lists executed, skipped, and unavailable provider suites with reasons.

<a id="legacy-policy"></a>
## Legacy / workaround policy

- No temporary aliases unless explicitly listed as allowed in a task.
- No compatibility shims unless they are the goal of a task.
- No type weakening to satisfy intermediate compilation.
- No preserving legacy names when the planned change is a removal or rename.
- Do not expand a task scope to fix breakages assigned to later units.
- If blocked, report the breakage and reference the downstream unit responsible.
- Do not delete or weaken Pi parity assertions; implement the missing Rust seam or mark a true live/manual requirement.

<a id="breaking-changes"></a>
## Planned Breaking Changes and Propagation Map

| Change | Introduced by | Expected temporary breakage | Fixed by | Forbidden workaround |
|---|---|---|---|---|
| Add dedicated Pi-parity test harness under `crates/zedflow-ai/tests/common/` | P2 | Existing ignored tests still point at old placeholder reasons until migrated | P3-P7 | Keeping mocks inline in source modules |
| Replace placeholder ignored tests with active deterministic parity tests | P3-P6 | Intermediate targeted tests may fail until provider seams are complete | Same owning unit | Rewriting expectations to current Rust behavior instead of Pi behavior |
| Add capability-gated live OpenRouter/Codex tests | P7 | Live tests skip when credentials are unavailable | P8 | Blanket `#[ignore]` for available live providers |

<a id="orchestration"></a>
## Subagent Orchestration Plan

Wave W1:
- P1 — Build Pi parity test matrix.

Wave W2:
- P2 — Add dedicated Pi-style Rust test harness and capability detection.

Wave W3:
- P3A — Anthropic deterministic payload/header parity.
- P3B — Bedrock deterministic payload/header parity.
- P3C — OpenAI/OpenRouter deterministic payload/header/error parity.

Wave W4:
- P4 — Stream/event parity for SSE/WebSocket/abort/error ordering.

Wave W5:
- P5 — Compat, Models, provider registry, and faux provider parity.

Wave W6:
- P6 — OAuth deterministic parity with fake HTTP and fake timing.

Wave W7:
- P7 — Live OpenRouter and OpenAI Codex activation.

Wave W8:
- P8 — Final audit, deterministic validation, and live capability report.

<a id="parallelization-constraints"></a>
## Parallelization Constraints

| Constraint | Reason | Affected units |
|---|---|---|
| P1 runs first | All later units need exact TS/Rust test mapping. | P2-P8 |
| P2 runs before P3-P7 | Fake transport/capability helpers must exist before migrations. | P3-P7 |
| P3A/P3B/P3C may run in parallel | They write disjoint provider test files and provider modules. | P3A, P3B, P3C |
| P4 after P3 | Stream parity depends on provider request/capture seams. | P4 |
| P5 after P3/P4 | Compat/models/faux depend on provider and stream shapes. | P5 |
| P6 after P2, parallel with P5 only if no shared auth files are touched | OAuth has separate files but may interact with Models auth. | P5, P6 |
| P7 after P3/P4/P6 | Live Codex/OpenRouter require deterministic request/stream/OAuth seams first. | P7 |
| P8 last | Owns global final validation and placeholder audit. | P8 |

<a id="canonical-line-references"></a>
## Canonical Line References

<!-- CANONICAL_LINE_REFERENCES_START -->
<!-- This block is generated by finalize-plan-lines.mjs. Do not edit manually. -->
| ID | Anchor | Lines | Description |
|---|---|---|---|
| how-to-use | #how-to-use | L3-L15 | How to use this plan |
| legend | #legend | L17-L51 | Legend |
| goal | #goal | L53-L58 | Goal |
| non-goals | #non-goals | L60-L69 | Non-goals |
| review-flags | #review-flags | L71-L80 | Review Flags |
| global-acceptance | #global-acceptance | L82-L91 | Global Acceptance Criteria |
| legacy-policy | #legacy-policy | L93-L102 | Legacy / workaround policy |
| breaking-changes | #breaking-changes | L104-L111 | Planned Breaking Changes and Propagation Map |
| orchestration | #orchestration | L113-L140 | Subagent Orchestration Plan |
| parallelization-constraints | #parallelization-constraints | L142-L154 | Parallelization Constraints |
| canonical-line-references | #canonical-line-references | L156-L163 | Canonical Line References |
| phases-and-tasks | #phases-and-tasks | L165-L166 | Phases and Tasks |
| phase-matrix | #phase-matrix | L168-L240 | Phase 1 — Pi parity matrix |
| P1 | #P1 | L171-L240 | Task P1 — Build Pi-AI parity test matrix |
| phase-harness | #phase-harness | L242-L317 | Phase 2 — Dedicated test harness aligned with Pi tests |
| P2 | #P2 | L245-L317 | Task P2 — Add dedicated Pi-style Rust test harness and live capability detection |
| phase-payloads | #phase-payloads | L319-L508 | Phase 3 — Deterministic payload/header/error parity |
| P3A | #P3A | L322-L380 | Task P3A — Anthropic deterministic payload/header parity |
| P3B | #P3B | L382-L441 | Task P3B — Bedrock deterministic payload/header parity |
| P3C | #P3C | L443-L508 | Task P3C — OpenAI, OpenRouter, and image deterministic payload/error parity |
| phase-streams | #phase-streams | L510-L575 | Phase 4 — Stream and event parity |
| P4 | #P4 | L513-L575 | Task P4 — SSE/WebSocket/abort/error stream event parity |
| phase-compat-models | #phase-compat-models | L577-L647 | Phase 5 — Compat, Models, provider registry, and faux provider parity |
| P5 | #P5 | L580-L647 | Task P5 — Compat/Models/Faux provider parity |
| phase-oauth | #phase-oauth | L649-L715 | Phase 6 — OAuth parity with fake HTTP and timing |
| P6 | #P6 | L652-L715 | Task P6 — OAuth deterministic parity |
| phase-live | #phase-live | L717-L782 | Phase 7 — Live OpenRouter and OpenAI Codex activation |
| P7 | #P7 | L720-L782 | Task P7 — Capability-gated live OpenRouter and OpenAI Codex tests |
| phase-final-audit | #phase-final-audit | L784-L846 | Phase 8 — Final audit and validation |
| P8 | #P8 | L787-L846 | Task P8 — Final parity audit, deterministic gates, and live report |
| pre-finalization-review | #pre-finalization-review | L848-L855 | Pre-finalization Review Summary |
<!-- CANONICAL_LINE_REFERENCES_END -->

<a id="phases-and-tasks"></a>
## Phases and Tasks

<a id="phase-matrix"></a>
## Phase 1 — Pi parity matrix

<a id="P1"></a>
### Task P1 — Build Pi-AI parity test matrix

Assignable: yes

Execution metadata:
- Wave: W1
- Context: fresh
- Depends on: none
- Can run in parallel with: none
- Must not run in parallel with: all implementation units

Scope boundaries:
- Goal: Create a complete mapping from remaining Rust ignored/placeholder tests to Pi TS tests and source behavior.
- Non-goals: Do not modify implementation or tests except the matrix artifact.
- Forbidden work: Do not remove ignores/placeholders; do not run live provider tests.

Files:
| Action | Path | Purpose |
|---|---|---|
| read | `.agents/state/zedflow-ai-placeholder-residuals.md` | Current residual ledger. |
| read | `crates/zedflow-ai/tests/**/*.rs` | Rust tests to classify. |
| read | `references/pi/packages/ai/test/**/*.test.ts` | Canonical Pi test behavior. |
| read | `references/pi/packages/ai/src/**/*.ts` | Source behavior references when tests rely on implementation details. |
| create | `.agents/state/zedflow-ai-pi-ai-parity-test-matrix.md` | Execution matrix for P2-P8. |

Required context package:
- Plan references: goal, non-goals, review flags, global acceptance, P1.
- Required skills: `/home/zedium/workspaces/zedflow/.agents/skills/rust-skills/SKILL.md`.
- Required files/symbols to read: every Rust test file containing `PORT PLACEHOLDER` or `#[ignore]`; matching Pi TS tests.
- Dependency outputs to read: none.
- Neighboring out-of-scope units: P2-P8 implementation.

Implementation outline:
1. Grep all `PORT PLACEHOLDER`, `#[ignore]`, and `ignore =` occurrences in `crates/zedflow-ai/tests`.
2. Match each Rust test/file to a Pi TS test and source behavior reference.
3. Classify each as `deterministic`, `fixture`, `live-openrouter`, `live-codex`, `live-other`, or `manual-browser`.
4. Identify missing Rust seams and owning future unit.
5. Write the matrix with exact files and test names.

Major snippets:

#### [CANONICAL] Matrix columns
```markdown
| Rust test | Pi TS test/source reference | Expected Pi behavior | Type | Missing Rust seam | Owning unit | Credentials/env if live | Status |
```

Validation responsibility:
- Type: locally-validating
- Must run: `grep -R -n "PORT PLACEHOLDER\|#\[ignore\|ignore =" crates/zedflow-ai/tests`
- Must NOT run: cargo global validation; live tests.
- Expected temporary breakage: none.
- Forbidden fixes/workarounds: editing tests to reduce matrix size.

Output contract:
- Path to `.agents/state/zedflow-ai-pi-ai-parity-test-matrix.md`.
- Count of placeholder/ignored tests by type and owning unit.
- List of any Rust test with no Pi TS/source mapping.

Acceptance criteria:
- Every residual `PORT PLACEHOLDER` test is assigned to a unit or true live/manual category.
- Every deterministic/fixture test has a Pi TS/source reference.

Handoff to dependent units:
- P2-P8 must read the matrix before editing.

Subagent prompt:
```text
You are implementing only P1 from .agents/plans/zedflow-ai-pi-ai-parity-finalization.md. Run in fresh context. Read the plan goal, review flags, global acceptance, and P1. Read .agents/state/zedflow-ai-placeholder-residuals.md, all Rust tests containing PORT PLACEHOLDER or #[ignore], and matching references/pi/packages/ai/test/*.test.ts plus source files as needed. Create .agents/state/zedflow-ai-pi-ai-parity-test-matrix.md with the required columns. Do not edit implementation or tests. Do not run cargo global validation or live tests.
```

<a id="phase-harness"></a>
## Phase 2 — Dedicated test harness aligned with Pi tests

<a id="P2"></a>
### Task P2 — Add dedicated Pi-style Rust test harness and live capability detection

Assignable: yes

Execution metadata:
- Wave: W2
- Context: fresh
- Depends on: P1
- Can run in parallel with: none
- Must not run in parallel with: P3-P7

Scope boundaries:
- Goal: Add reusable test-only helpers that let Rust tests reproduce Pi's mocked HTTP/SSE/WebSocket/OAuth/timer behavior and live credential gating.
- Non-goals: Do not migrate provider-specific tests yet.
- Forbidden work: Do not add mock-only code to production source modules.

Files:
| Action | Path | Purpose |
|---|---|---|
| create | `crates/zedflow-ai/tests/common/mod.rs` | Test helper module root. |
| create | `crates/zedflow-ai/tests/common/http_capture.rs` | Fake HTTP request/response capture. |
| create | `crates/zedflow-ai/tests/common/sse_fixture.rs` | SSE fixture utilities. |
| create | `crates/zedflow-ai/tests/common/ws_fixture.rs` | WebSocket fixture utilities for Codex. |
| create | `crates/zedflow-ai/tests/common/oauth_fixture.rs` | Fake OAuth/device-code endpoints and token fixtures. |
| create | `crates/zedflow-ai/tests/common/live_credentials.rs` | Pi-equivalent live capability detection and skip messages. |
| read | `.agents/state/zedflow-ai-pi-ai-parity-test-matrix.md` | Harness requirements. |
| read | `references/pi/packages/ai/test/**/*.test.ts` | Pi mock conventions. |

Required context package:
- Plan references: RF-MOCK-LOCATION, RF-LIVE-CREDENTIALS, P2.
- Required skills: rust-skills.
- Required files/symbols to read: Pi tests using `vi.fn`, fetch mocks, SSE fixtures, OAuth device-code mocks, Codex WebSocket probes.
- Dependency outputs to read: P1 matrix.
- Neighboring out-of-scope units: provider-specific test migrations.

Implementation outline:
1. Add test-only helper modules under `tests/common`.
2. Mirror Pi test capabilities: captured method/url/headers/body, sequenced responses, SSE frames, WebSocket frames, fake OAuth polling, and explicit live capability checks.
3. Provide helpers that redact secret-bearing headers in assertion failures/logs.
4. Add minimal self-tests in dedicated test files if needed; do not put mock tests in source modules.

Major snippets:

#### [CANONICAL] Live capability policy
```rust
// Live tests must call capability helpers and skip only when the required Pi-equivalent
// credential is unavailable. Skips must name the missing provider capability and must not
// log credential values.
```

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; targeted compile/check for test helpers; any helper self-tests.
- Must NOT run: live provider tests.
- Expected temporary breakage: none.
- Forbidden fixes/workarounds: moving mocks into source files.

Output contract:
- List created helper modules and supported capabilities.
- List any Pi mock behavior not represented yet and owning unit.

Acceptance criteria:
- Helpers compile and are usable from integration tests.
- No new mock/fake code is added to production source modules.

Handoff to dependent units:
- P3-P7 must reuse these helpers instead of local ad hoc mocks.

Subagent prompt:
```text
You are implementing only P2 from .agents/plans/zedflow-ai-pi-ai-parity-finalization.md. Fresh context. Read P1 matrix, plan review flags RF-MOCK-LOCATION/RF-LIVE-CREDENTIALS, rust-skills, and Pi tests that use mocked HTTP/SSE/WebSocket/OAuth behavior. Create dedicated Rust test helpers under crates/zedflow-ai/tests/common/ for HTTP capture, SSE fixtures, WebSocket fixtures, OAuth fixtures, and live capability detection. Do not add mock-only code to production source modules. Run fmt and targeted helper validation only; no live provider tests.
```

<a id="phase-payloads"></a>
## Phase 3 — Deterministic payload/header/error parity

<a id="P3A"></a>
### Task P3A — Anthropic deterministic payload/header parity

Assignable: yes

Execution metadata:
- Wave: W3
- Context: fresh
- Depends on: P1, P2
- Can run in parallel with: P3B, P3C
- Must not run in parallel with: P4, P5, P8

Scope boundaries:
- Goal: Reactivate Anthropic deterministic tests so they assert Pi-equivalent request payloads, headers, thinking/cache/tool behavior, and payload capture.
- Non-goals: Do not run live Anthropic provider tests.
- Forbidden work: Do not weaken Anthropic assertions or keep placeholder ignores for local deterministic behavior.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `crates/zedflow-ai/src/api/anthropic-messages.rs` | Expose/complete production request-building seams if needed. |
| modify | `crates/zedflow-ai/tests/anthropic-empty-thinking-signature-compat.rs` | Reactivate parity tests. |
| modify | `crates/zedflow-ai/tests/anthropic-temperature-compat.rs` | Reactivate parity tests. |
| modify | `crates/zedflow-ai/tests/anthropic-thinking-disable.rs` | Reactivate parity tests. |
| modify | `crates/zedflow-ai/tests/anthropic-eager-tool-input-e2e.rs` | Split deterministic payload assertions from live provider calls. |
| read | `references/pi/packages/ai/test/anthropic-*.test.ts` | Canonical behavior. |
| read | `references/pi/packages/ai/src/api/anthropic-messages.ts` | Source behavior. |

Required context package:
- Plan references: P3A, RF-PI-TEST-SHAPE, RF-MOCK-LOCATION.
- Required skills: rust-skills.
- Dependency outputs to read: P1 matrix, P2 helpers.

Implementation outline:
1. Move any mock/capture logic into dedicated test helpers or test files.
2. Wire deterministic payload capture through production-equivalent request-building seams.
3. Compare payload/header JSON to Pi behavior for temperature, thinking disabled, empty signatures, eager tool input, cache headers, and beta headers.
4. Remove `PORT PLACEHOLDER` ignores only after assertions are active.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; targeted Anthropic deterministic tests modified by this unit.
- Must NOT run: live Anthropic tests.

Output contract:
- List reactivated tests and remaining true live Anthropic skips.
- List Pi TS references used for each test family.

Acceptance criteria:
- No `PORT PLACEHOLDER` remains in P3A-owned deterministic Anthropic test files except true live-only sections with non-placeholder wording.
- Targeted deterministic Anthropic tests pass.

Handoff to dependent units:
- P4 uses the same Anthropic stream event shapes for stream parity.

Subagent prompt:
```text
You are implementing only P3A from .agents/plans/zedflow-ai-pi-ai-parity-finalization.md. Fresh context. Read P1 matrix, P2 test helpers, rust-skills, Pi anthropic tests and api/anthropic-messages.ts. Reactivate deterministic Anthropic payload/header parity tests using dedicated test helpers, not production mocks. Preserve Pi observable behavior exactly. Do not run live Anthropic tests. Run fmt and targeted Anthropic deterministic tests.
```

<a id="P3B"></a>
### Task P3B — Bedrock deterministic payload/header parity

Assignable: yes

Execution metadata:
- Wave: W3
- Context: fresh
- Depends on: P1, P2
- Can run in parallel with: P3A, P3C
- Must not run in parallel with: P4, P5, P8

Scope boundaries:
- Goal: Reactivate Bedrock deterministic payload/header tests for Converse conversion, thinking payloads, region/auth/header decisions, and error formatting.
- Non-goals: Do not run live AWS tests.
- Forbidden work: Do not substitute genai-normalized behavior for Pi Bedrock behavior.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `crates/zedflow-ai/src/api/bedrock-converse-stream.rs` | Complete production request-planning/conversion seams. |
| modify | `crates/zedflow-ai/src/bedrock-provider.rs` | Region/auth behavior if needed. |
| modify | `crates/zedflow-ai/tests/bedrock-convert-messages.rs` | Reactivate conversion tests. |
| modify | `crates/zedflow-ai/tests/bedrock-thinking-payload.rs` | Reactivate thinking payload tests. |
| modify | `crates/zedflow-ai/tests/bedrock-custom-headers.rs` | Ensure signed header parity. |
| read | `references/pi/packages/ai/test/bedrock-*.test.ts` | Canonical tests. |
| read | `references/pi/packages/ai/src/api/bedrock-converse-stream.ts` | Source behavior. |
| read | `references/pi/packages/ai/src/bedrock-provider.ts` | Region/auth behavior. |

Required context package:
- Plan references: P3B and review flags.
- Required skills: rust-skills.
- Dependency outputs to read: P1 matrix, P2 helpers.

Implementation outline:
1. Use P2 helpers for deterministic request capture, not live AWS.
2. Port exact message-to-Converse payload conversion including thinking fields, tool use, image/tool result behavior, custom header filtering, and auth/region decisions.
3. Reactivate ignored Bedrock fixture tests.
4. Keep only true live AWS credential tests skipped with explicit missing capability wording.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; targeted Bedrock deterministic tests.
- Must NOT run: live AWS tests.

Output contract:
- List reactivated Bedrock tests and live-only AWS skips.
- List any AWS SDK/live behavior still requiring P7/P8 classification.

Acceptance criteria:
- No local deterministic Bedrock placeholder remains.
- Targeted Bedrock deterministic tests pass.

Handoff to dependent units:
- P4 may rely on Bedrock event conversion shapes.

Subagent prompt:
```text
You are implementing only P3B from .agents/plans/zedflow-ai-pi-ai-parity-finalization.md. Fresh context. Read P1 matrix, P2 helpers, rust-skills, Pi Bedrock tests, api/bedrock-converse-stream.ts, and bedrock-provider.ts. Reactivate deterministic Bedrock payload/header/conversion tests with Pi-equivalent behavior. Do not run live AWS tests. Run fmt and targeted Bedrock deterministic tests.
```

<a id="P3C"></a>
### Task P3C — OpenAI, OpenRouter, and image deterministic payload/error parity

Assignable: yes

Execution metadata:
- Wave: W3
- Context: fresh
- Depends on: P1, P2
- Can run in parallel with: P3A, P3B
- Must not run in parallel with: P4, P5, P8

Scope boundaries:
- Goal: Reactivate deterministic OpenAI-family and OpenRouter image request/error parity tests.
- Non-goals: Do not run live OpenRouter or Codex tests; P7 owns live activation.
- Forbidden work: Do not ignore payload/tool-choice/reasoning tests just because provider transport is not live.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `crates/zedflow-ai/src/api/openai-completions.rs` | Request/stream helper parity if needed. |
| modify | `crates/zedflow-ai/src/api/openai-responses.rs` | Responses request parity if needed. |
| modify | `crates/zedflow-ai/src/api/openai-codex-responses.rs` | Codex request capture parity if needed. |
| modify | `crates/zedflow-ai/src/api/openrouter-images.rs` | Image request/error parity if needed. |
| modify | `crates/zedflow-ai/tests/openai-completions-cache-control-format.rs` | Reactivate cache header tests. |
| modify | `crates/zedflow-ai/tests/openai-completions-empty-tools.rs` | Reactivate request params tests. |
| modify | `crates/zedflow-ai/tests/openai-completions-tool-choice.rs` | Reactivate tool/reasoning tests. |
| modify | `crates/zedflow-ai/tests/provider-error-body-passthrough.rs` | Reactivate image/provider error tests. |
| modify | `crates/zedflow-ai/tests/provider-error-body-regression.rs` | Reactivate error formatting tests. |
| read | `references/pi/packages/ai/test/openai-*.test.ts` | Pi OpenAI tests. |
| read | `references/pi/packages/ai/test/openrouter-*.test.ts` | Pi OpenRouter tests. |
| read | `references/pi/packages/ai/src/api/openai-*.ts` | Source behavior. |
| read | `references/pi/packages/ai/src/api/openrouter-images.ts` | Image source behavior. |

Required context package:
- Plan references: P3C and review flags.
- Required skills: rust-skills.
- Dependency outputs to read: P1 matrix, P2 helpers.

Implementation outline:
1. Use P2 HTTP capture to assert request payloads/options/headers.
2. Port Pi tool-choice, reasoning, cache-control, empty tools, image tool result, and error body behavior.
3. Keep live network tests for P7 only.
4. Remove placeholder wording only where behavior is implemented or true live gating remains.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; targeted OpenAI/OpenRouter deterministic tests.
- Must NOT run: live OpenRouter/Codex tests.

Output contract:
- List reactivated deterministic tests and true live-only tests moved to P7.
- List Pi TS references used.

Acceptance criteria:
- No deterministic OpenAI/OpenRouter placeholder remains in P3C-owned tests.
- Targeted deterministic tests pass.

Handoff to dependent units:
- P4 uses stream fixtures for OpenAI/Codex stream event parity.
- P7 uses live OpenRouter/Codex paths after deterministic request parity is green.

Subagent prompt:
```text
You are implementing only P3C from .agents/plans/zedflow-ai-pi-ai-parity-finalization.md. Fresh context. Read P1 matrix, P2 helpers, rust-skills, Pi OpenAI/OpenRouter tests and source files. Reactivate deterministic OpenAI-family/OpenRouter payload, tool-choice, reasoning, cache, image, and error parity tests. Use test helpers for capture; do not run live provider tests. Run fmt and targeted deterministic tests.
```

<a id="phase-streams"></a>
## Phase 4 — Stream and event parity

<a id="P4"></a>
### Task P4 — SSE/WebSocket/abort/error stream event parity

Assignable: yes

Execution metadata:
- Wave: W4
- Context: fresh
- Depends on: P3A, P3B, P3C
- Can run in parallel with: none
- Must not run in parallel with: P5, P7, P8

Scope boundaries:
- Goal: Reproduce Pi stream event order and shapes for SSE, WebSocket, abort, error, usage, tool-call deltas, and terminal events.
- Non-goals: Do not implement model/provider metadata; P5 owns that.
- Forbidden work: Do not replace event parity assertions with coarse success checks.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `crates/zedflow-ai/src/api/openai-codex-responses.rs` | Codex SSE/WebSocket stream parity. |
| modify | `crates/zedflow-ai/src/api/openai-completions.rs` | Chat Completions stream chunk parity. |
| modify | `crates/zedflow-ai/src/api/anthropic-messages.rs` | Anthropic terminal event parity if needed. |
| modify | `crates/zedflow-ai/tests/openai-codex-stream.rs` | Reactivate Codex fake SSE/WebSocket tests. |
| modify | `crates/zedflow-ai/tests/openai-completions-response-model.rs` | Reactivate chunk parsing tests. |
| modify | `crates/zedflow-ai/tests/openai-completions-tool-choice.rs` | Reactivate stream event cases not covered by P3C. |
| modify | `crates/zedflow-ai/tests/context-overflow.rs` | Reactivate deterministic overflow/error tests if fixtureable. |
| modify | `crates/zedflow-ai/tests/stream.rs` | Reactivate deterministic stream facade tests. |
| read | `references/pi/packages/ai/test/*stream*.test.ts` | Pi stream tests. |
| read | `references/pi/packages/ai/src/utils/event-stream.ts` | Event stream behavior. |

Required context package:
- Plan references: P4 and RF-PI-TEST-SHAPE.
- Required skills: rust-skills.
- Dependency outputs to read: P1 matrix, P2 helpers, P3 outputs.

Implementation outline:
1. Use P2 SSE/WebSocket fixtures to feed deterministic streams.
2. Port Pi event ordering and terminal behavior for completed/incomplete/error/abort cases.
3. Reactivate fixture stream tests and keep true live-only stream tests for P7/P8.
4. Validate that tool-call delta coalescing, reasoning deltas, null chunks, usage chunks, retry/backoff, and cached WebSocket behavior match Pi tests.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; targeted stream tests.
- Must NOT run: live provider tests.

Output contract:
- List stream tests reactivated and remaining live-only stream tests.
- List unsupported stream edge cases, if any, with Pi reference and blocker.

Acceptance criteria:
- Deterministic stream placeholders are removed.
- Targeted stream tests pass.

Handoff to dependent units:
- P5 uses stable event/content shapes for compat/faux/model APIs.
- P7 uses live stream paths after fixture parity is green.

Subagent prompt:
```text
You are implementing only P4 from .agents/plans/zedflow-ai-pi-ai-parity-finalization.md. Fresh context. Read P1 matrix, P2 helpers, P3 outputs, rust-skills, Pi stream tests, and utils/event-stream.ts. Implement Pi-equivalent deterministic stream event behavior for SSE/WebSocket/abort/error/usage/tool-call cases and reactivate corresponding Rust tests. Do not run live provider tests. Run fmt and targeted stream tests.
```

<a id="phase-compat-models"></a>
## Phase 5 — Compat, Models, provider registry, and faux provider parity

<a id="P5"></a>
### Task P5 — Compat/Models/Faux provider parity

Assignable: yes

Execution metadata:
- Wave: W5
- Context: fresh
- Depends on: P3A, P3B, P3C, P4
- Can run in parallel with: P6 only if no shared auth/model files are edited
- Must not run in parallel with: P7, P8

Scope boundaries:
- Goal: Bring Rust compat/model/provider/faux behavior in line with Pi observable behavior.
- Non-goals: Do not handle OAuth device-code internals; P6 owns OAuth flows.
- Forbidden work: Do not keep placeholder metadata or unordered provider behavior when Pi specifies insertion order.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `crates/zedflow-ai/src/compat.rs` | Pi compat behavior. |
| modify | `crates/zedflow-ai/src/providers/all.rs` | Builtin provider order/registry. |
| modify | `crates/zedflow-ai/src/providers/static_catalog.rs` | Static catalog metadata. |
| modify | `crates/zedflow-ai/src/providers/**/*.rs` | Provider auth/model metadata where P1 matrix assigns. |
| modify | `crates/zedflow-ai/tests/faux-provider.rs` | Reactivate faux provider parity tests. |
| modify | `crates/zedflow-ai/tests/models-runtime.rs` | Reactivate Models behavior tests. |
| modify | `crates/zedflow-ai/tests/providers.rs` | Reactivate provider registry/auth tests. |
| modify | `crates/zedflow-ai/tests/supports-xhigh.rs` | Reactivate thinking-level metadata tests. |
| modify | `crates/zedflow-ai/tests/xhigh.rs` | Split deterministic metadata from live provider calls. |
| modify | `crates/zedflow-ai/tests/lazy-module-load.rs` | Reactivate Rust-equivalent lazy behavior tests. |
| read | `references/pi/packages/ai/test/faux-provider.test.ts` | Faux provider behavior. |
| read | `references/pi/packages/ai/test/models-runtime.test.ts` | Models behavior. |
| read | `references/pi/packages/ai/test/providers.test.ts` | Provider behavior. |
| read | `references/pi/packages/ai/src/compat.ts` | Compat source behavior. |
| read | `references/pi/packages/ai/src/providers/**/*.ts` | Provider source behavior. |

Required context package:
- Plan references: P5 and review flags.
- Required skills: rust-skills.
- Dependency outputs to read: P1 matrix, P2 helpers, P3/P4 outputs.

Implementation outline:
1. Port provider insertion order and static metadata exactly enough for tests.
2. Port `Models` auth/env resolution and refresh behavior, including in-flight dedupe if specified by Pi tests.
3. Complete production faux provider behavior: typed content/events, async factories if Pi exposes them, errors/panics as assistant error events, abort/pacing/cache/session accounting.
4. Reactivate deterministic compat/model/provider/faux tests.
5. Keep dynamic import/load observability tests only where Rust static lazy behavior can have an equivalent; otherwise document as JS-only without `PORT PLACEHOLDER`.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; targeted compat/models/providers/faux tests.
- Must NOT run: live provider tests.

Output contract:
- List reactivated compat/model/faux tests.
- List JS-only lazy import observability cases with exact rationale.

Acceptance criteria:
- No local deterministic compat/model/faux placeholder remains.
- Targeted tests pass.

Handoff to dependent units:
- P7 live tests depend on provider/auth/model registry correctness.

Subagent prompt:
```text
You are implementing only P5 from .agents/plans/zedflow-ai-pi-ai-parity-finalization.md. Fresh context. Read P1 matrix, P2 helpers, P3/P4 outputs, rust-skills, Pi compat/models/providers/faux tests and source files. Implement Pi-equivalent compat, Models, provider registry/order/metadata, and faux provider behavior. Keep mocks in tests/common or test files. Do not run live provider tests. Run fmt and targeted compat/models/providers/faux tests.
```

<a id="phase-oauth"></a>
## Phase 6 — OAuth parity with fake HTTP and timing

<a id="P6"></a>
### Task P6 — OAuth deterministic parity

Assignable: yes

Execution metadata:
- Wave: W6
- Context: fresh
- Depends on: P2
- Can run in parallel with: P5 only if file scopes do not overlap
- Must not run in parallel with: P7, P8

Scope boundaries:
- Goal: Reactivate deterministic OAuth tests with fake HTTP/timing while preserving Pi token exchange, refresh, polling, cancellation, timeout, and credential persistence behavior.
- Non-goals: Do not require browser/manual interaction for deterministic tests; live/manual browser remains capability-gated.
- Forbidden work: Do not leave OAuth tests ignored because HTTP/timer injection is missing.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `crates/zedflow-ai/src/oauth.rs` | OAuth API parity if needed. |
| modify | `crates/zedflow-ai/src/auth/types.rs` | Auth/cancellation surfaces if needed. |
| modify | `crates/zedflow-ai/src/utils/oauth/anthropic.rs` | Anthropic OAuth behavior. |
| modify | `crates/zedflow-ai/src/utils/oauth/github-copilot.rs` | GitHub Copilot OAuth behavior. |
| modify | `crates/zedflow-ai/src/utils/oauth/openai-codex.rs` | OpenAI Codex OAuth behavior. |
| modify | `crates/zedflow-ai/src/utils/oauth/device-code.rs` | Device-code polling if present. |
| modify | `crates/zedflow-ai/tests/anthropic-oauth.rs` | Reactivate deterministic OAuth tests. |
| modify | `crates/zedflow-ai/tests/github-copilot-oauth.rs` | Reactivate deterministic OAuth tests. |
| modify | `crates/zedflow-ai/tests/openai-codex-oauth.rs` | Reactivate deterministic OAuth tests. |
| modify | `crates/zedflow-ai/tests/oauth-auth.rs` | Reactivate auth resolution tests. |
| read | `references/pi/packages/ai/test/*oauth*.test.ts` | Pi OAuth tests. |
| read | `references/pi/packages/ai/src/utils/oauth/**/*.ts` | OAuth source behavior. |

Required context package:
- Plan references: P6, RF-MOCK-LOCATION, RF-LIVE-CREDENTIALS.
- Required skills: rust-skills.
- Dependency outputs to read: P1 matrix, P2 OAuth helper.

Implementation outline:
1. Add minimal production injection seams only where needed for deterministic HTTP/time behavior; do not add broad test-only production shims.
2. Use P2 OAuth fixtures in integration tests.
3. Port device-code polling, 403/404 pending behavior, timeout, cancellation, refresh error body passthrough, credential persistence, and manual/browser local callback behavior where deterministic.
4. Leave only true browser/manual interactions capability-gated with exact reasons.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; targeted OAuth tests.
- Must NOT run: real browser/manual OAuth or live provider endpoints.

Output contract:
- List reactivated OAuth tests and remaining manual/browser-only tests.
- List exact capability/env requirements for Codex live auth consumed by P7.

Acceptance criteria:
- No deterministic OAuth placeholder remains.
- Targeted OAuth tests pass with fake HTTP/timing.

Handoff to dependent units:
- P7 uses Codex credential detection and auth behavior for live tests.

Subagent prompt:
```text
You are implementing only P6 from .agents/plans/zedflow-ai-pi-ai-parity-finalization.md. Fresh context. Read P1 matrix, P2 OAuth fixtures, rust-skills, Pi OAuth tests, and utils/oauth source files. Reactivate deterministic OAuth tests using fake HTTP/timing; preserve Pi token exchange, refresh, polling, timeout, cancellation, and credential persistence behavior. Do not run browser/manual/live OAuth endpoints. Run fmt and targeted OAuth tests.
```

<a id="phase-live"></a>
## Phase 7 — Live OpenRouter and OpenAI Codex activation

<a id="P7"></a>
### Task P7 — Capability-gated live OpenRouter and OpenAI Codex tests

Assignable: yes

Execution metadata:
- Wave: W7
- Context: fresh
- Depends on: P3C, P4, P5, P6
- Can run in parallel with: none
- Must not run in parallel with: P8

Scope boundaries:
- Goal: Enable live OpenRouter and OpenAI Codex tests when the user's available credentials/capabilities are present, while keeping unavailable providers explicitly skipped.
- Non-goals: Do not add live credentials to files or logs; do not enable providers without credentials.
- Forbidden work: Do not blanket-ignore OpenRouter/Codex live tests when credentials are available.

Files:
| Action | Path | Purpose |
|---|---|---|
| modify | `crates/zedflow-ai/tests/common/live_credentials.rs` | Capability detection and skip helpers. |
| modify | `crates/zedflow-ai/tests/openrouter-cache-write-repro.rs` | OpenRouter live activation. |
| modify | `crates/zedflow-ai/tests/provider-error-body-passthrough.rs` | OpenRouter live/fake split. |
| modify | `crates/zedflow-ai/tests/images.rs` | OpenRouter images live activation. |
| modify | `crates/zedflow-ai/tests/responseid.rs` | OpenRouter/Codex live cases. |
| modify | `crates/zedflow-ai/tests/openai-codex-cache-affinity-e2e.rs` | Codex live activation. |
| modify | `crates/zedflow-ai/tests/codex-websocket-cached-probe.rs` | Codex WebSocket live activation. |
| modify | `crates/zedflow-ai/tests/openai-codex-stream.rs` | Codex live/fake split if needed. |
| read | `references/pi/packages/ai/test/openrouter-*.test.ts` | OpenRouter live behavior. |
| read | `references/pi/packages/ai/test/openai-codex-*.test.ts` | Codex live behavior. |

Required context package:
- Plan references: P7, RF-LIVE-CREDENTIALS, RF-LIVE-NOT-OPTIONAL.
- Required skills: rust-skills.
- Dependency outputs to read: P1 matrix, P2 live credentials, P3/P4/P5/P6 outputs.

Implementation outline:
1. Detect OpenRouter and Codex capability using Pi-equivalent env/auth/credential-store behavior.
2. Convert OpenRouter/Codex live tests from blanket ignore to capability-gated execution.
3. Ensure skip messages identify missing capability without printing secrets.
4. Run live tests only when capability helpers report available credentials in the current environment.
5. Keep other providers skipped with exact missing credential requirements.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; deterministic compile of live tests; OpenRouter/Codex live tests only if credentials are detected.
- Must NOT run: live Anthropic/Bedrock/Google/Mistral/etc. unless credentials are detected and assigned by P1/P8.

Output contract:
- Live capability report: available, executed, skipped, failed.
- Commands run and redaction confirmation.

Acceptance criteria:
- OpenRouter/Codex tests are not blanket ignored if credentials are present.
- Missing credentials produce clear skips, not placeholder wording.

Handoff to dependent units:
- P8 includes this report in final ledger.

Subagent prompt:
```text
You are implementing only P7 from .agents/plans/zedflow-ai-pi-ai-parity-finalization.md. Fresh context. Read P1 matrix, P2 live credential helpers, P3-P6 outputs, rust-skills, and Pi OpenRouter/Codex tests. Enable capability-gated live OpenRouter and OpenAI Codex tests. Do not log secrets. Run live OpenRouter/Codex tests only if helpers detect credentials; otherwise record explicit skips. Do not run unrelated live providers.
```

<a id="phase-final-audit"></a>
## Phase 8 — Final audit and validation

<a id="P8"></a>
### Task P8 — Final parity audit, deterministic gates, and live report

Assignable: yes

Execution metadata:
- Wave: W8
- Context: fresh
- Depends on: P1, P2, P3A, P3B, P3C, P4, P5, P6, P7
- Can run in parallel with: none
- Must not run in parallel with: all other write units

Scope boundaries:
- Goal: Verify no port placeholders remain, deterministic gates pass, and live/manual residuals are justified by actual unavailable capabilities.
- Non-goals: Do not implement new behavior except small test metadata/ledger fixes found during audit.
- Forbidden work: Do not hide failures by adding ignores without matrix-backed justification.

Files:
| Action | Path | Purpose |
|---|---|---|
| create | `.agents/state/zedflow-ai-pi-ai-final-parity-report.md` | Final parity and live capability report. |
| modify if needed | `crates/zedflow-ai/tests/**/*.rs` | Remove stale placeholder wording or fix final deterministic test issues. |
| read | `.agents/state/zedflow-ai-pi-ai-parity-test-matrix.md` | Expected final state. |
| read | `.agents/state/zedflow-ai-placeholder-residuals.md` | Previous residual baseline. |

Required context package:
- Plan references: global acceptance, P8, all review flags.
- Required skills: rust-skills.
- Dependency outputs to read: all prior unit outputs.

Implementation outline:
1. Run final placeholder grep.
2. Audit ignored tests and ensure every ignore is live/manual/capability-only with exact reason.
3. Verify no public `genai` type leak regressed.
4. Run deterministic cargo gates.
5. Run OpenRouter/Codex live tests if capabilities are detected; record skip reasons otherwise.
6. Write final report.

Validation responsibility:
- Type: integration-validating
- Must run: `grep -R "PORT PLACEHOLDER" crates/zedflow-ai/src crates/zedflow-ai/tests`; `cargo fmt --all --check`; `cargo check -p zedflow-ai --all-targets`; `cargo test -p zedflow-ai --all-targets`; capability-gated OpenRouter/Codex live commands when available.
- Must NOT run: unavailable live provider suites.

Output contract:
- Path to `.agents/state/zedflow-ai-pi-ai-final-parity-report.md`.
- Validation command results.
- Live capability report.
- Remaining ignored tests and reasons.

Acceptance criteria:
- Global acceptance criteria satisfied.
- No unresolved local deterministic Pi behavior remains placeholdered or ignored.

Handoff to dependent units:
- None. This is final.

Subagent prompt:
```text
You are implementing only P8 from .agents/plans/zedflow-ai-pi-ai-parity-finalization.md. Fresh context. Read all prior unit outputs, P1 matrix, residual ledgers, rust-skills, and this plan's global acceptance. Run final PORT PLACEHOLDER audit, ignore audit, public genai leak audit, deterministic cargo gates, and capability-gated OpenRouter/Codex live tests when credentials are detected. Do not run unavailable live provider suites. Write .agents/state/zedflow-ai-pi-ai-final-parity-report.md with command results, live report, and remaining manual/live-only ignores.
```

<a id="pre-finalization-review"></a>
## Pre-finalization Review Summary

| Reviewer | Status | Required changes applied | Remaining concerns |
|---|---|---|---|
| Feasibility / file references | Passed | File scopes are tied to current Rust tests and Pi TS test/source paths. | Exact per-test ownership must be confirmed by P1 matrix before implementation. |
| Sequencing / dependency graph | Passed | Matrix and harness run before provider/test migrations; final audit runs last. | P5/P6 parallelism is conditional on no shared auth/model writes. |
| Scope isolation / prompt quality | Passed | Each assignable unit has fresh prompt, file scope, validation responsibility, and forbidden work. | Live credential availability remains runtime-dependent and must not leak secrets. |
