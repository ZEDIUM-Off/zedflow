# Zedflow AI Placeholder Dependency Replacement

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

Finish `crates/zedflow-ai` after the initial Pi-to-Rust package port by replacing every dependency/runtime `PORT PLACEHOLDER` inherited from Pi-ai with Rust implementations that preserve Pi TypeScript behavior as closely as possible. The plan assumes `.agents/plans/pi-to-rust-package-port.md` has already been executed for `pi-ai`: source files and tests exist in Rust, but dependency-backed behavior may still be placeholdered.

`genai` is the preferred internal backend for LLM connection/request handling only where it preserves Pi observable behavior. When Pi depends on raw provider payload mutation, response headers, raw stream validation, Bedrock Smithy/SigV4 middleware semantics, or Mistral direct API semantics, implement the narrow provider-specific Rust fallback instead of forcing `genai`.

<a id="non-goals"></a>
## Non-goals

- Do not change the public `zedflow-ai`/Pi-ai API surface to expose `genai` types.
- Do not replace the Pi provider/model catalog with `genai`'s registry.
- Do not implement Zedflow product features, Flow, Runtime Graph, or LangGraph behavior.
- Do not remove ignored live-network tests unless deterministic local parity is possible.
- Do not preserve JavaScript dynamic import mechanics; preserve only observable lazy API behavior.
- Do not add broad compatibility shims or weaken Pi types to make intermediate code compile.

<a id="review-flags"></a>
## Review Flags

| ID | Type | Severity | Summary | Affects | Follow-up / accepted risk |
|---|---|---:|---|---|---|
| RF-GENAI-HOOKS | R | High | `genai` has no public equivalent for Pi `onPayload` mutation or `onResponse` status/header callbacks. | OpenAI, Anthropic, Google, Bedrock, Mistral | Use `genai` only when hooks are absent or non-observable; otherwise implement narrow `reqwest`/SDK fallback. |
| RF-BEDROCK | R | High | `genai` Bedrock SigV4 has simpler region/profile/bearer/custom-header behavior than Pi's AWS SDK implementation. | U5 | Fallback to `aws-sdk-bedrockruntime` if the Bedrock spike cannot prove exact parity. |
| RF-ANTHROPIC-SSE | R | High | Pi parses raw Anthropic SSE and errors when `message_start` is not followed by `message_stop`; `genai` normalizes streams. | U4 | Use raw `reqwest` SSE fallback for raw-stream/OAuth/beta behavior. |
| RF-MISTRAL | R | Medium | `genai` does not expose direct Mistral as a documented adapter. | U7 | Implement Mistral direct with `reqwest`; reserve `genai` for OpenAI-compatible gateways. |
| RF-TYPEBOX | R | Medium | TypeBox replacement is not just schema representation; Pi also coerces, cleans, caches validators, and formats validation errors. | U8 | Keep schema as `serde_json::Value`; implement validation/coercion/clean locally with `jsonschema`. |
| RF-PROXY | R | Medium | `reqwest::Proxy` does not implement Pi's env/no_proxy resolution. | U2, U3-U7 | Keep a Pi-compatible proxy resolver and inject the resulting proxy into `genai`/fallback clients. |
| RF-OAUTH | OQ | Medium | OAuth/manual login flows may require live/browser interaction. | U10 | Implement deterministic token exchange/device-code helpers; keep manual/live tests ignored with explicit reasons. |

<a id="global-acceptance"></a>
## Global Acceptance Criteria

- `grep -R "PORT PLACEHOLDER" crates/zedflow-ai/src crates/zedflow-ai/tests` returns only accepted live/manual/OAuth/provider-unavailable blockers documented in a final residual-placeholder table.
- Every dependency placeholder has a documented Rust replacement or accepted fallback path tied to a Pi TS behavior reference.
- `genai` is used only behind internal `utils::genai_backend`; no public Rust API exports `genai` types.
- Provider behavior preserves Pi-visible events, stop reasons, usage/cost accounting, errors, hook behavior, cache/session headers, proxy handling, and validation semantics unless a residual review flag explicitly accepts a difference.
- Required docs are used by every implementation subagent: `/home/zedium/workspaces/zedflow/.agents/skills/rust-skills/SKILL.md` and `/tmp/pi-github-repos/jeremychone/rust-genai/docs/for-llm/api-reference-for-llm.md` or the live GitHub URL `https://github.com/jeremychone/rust-genai/blob/main/docs/for-llm/api-reference-for-llm.md`.
- Validation gates pass: `cargo fmt --all --check`, `cargo check -p zedflow-ai --all-targets`, and all deterministic `zedflow-ai` tests that are not live/manual ignored.

<a id="legacy-policy"></a>
## Legacy / workaround policy

- No temporary aliases unless explicitly listed as allowed in a task.
- No compatibility shims unless they are the goal of a task.
- No type weakening to satisfy intermediate compilation.
- No preserving legacy names when the planned change is a removal or rename.
- Do not expand a task scope to fix breakages assigned to later units.
- If blocked, report the breakage and reference the downstream unit responsible.
- Do not replace Pi behavior with `genai` normalized behavior when Pi's TS implementation exposes provider-specific payload, stream, headers, or errors.

<a id="breaking-changes"></a>
## Planned Breaking Changes and Propagation Map

| Change | Introduced by | Expected temporary breakage | Fixed by | Forbidden workaround |
|---|---|---|---|---|
| Add internal `utils/genai-backend.rs` and `genai` dependency | U2 | API modules may still call placeholders until migrated | U3-U7 | Exposing `genai` types in public API |
| Replace selected provider SDK placeholders with `genai`/fallback transports | U3-U7 | Some ignored placeholder tests become stale | U11 | Removing tests instead of unignoring/updating them |
| Replace TypeBox runtime with Rust JSON-schema validation/coercion | U8 | Tool validation tests may need exact error adaptation | U8, U11 | `serde_json::Value` only with no validation/coercion |
| Replace JS dynamic import/lazy behavior with static Rust dispatch | U9 | `api/*.lazy.rs` and `compat.rs` placeholder assumptions change | U9 | Recreating JS dynamic import machinery |

<a id="orchestration"></a>
## Subagent Orchestration Plan

### Wave 1 — Fidelity inventory

- Run U1 alone. It produces the dependency-placeholder matrix and verifies exact scopes before any implementation.

### Wave 2 — Shared backend/utilities foundation

- After U1, run U2 and U8 in parallel if desired. They write disjoint files.

### Wave 3 — Provider implementations

- After U2, run U3, U4, U5, U6, and U7. U3 and U7 both touch OpenAI-compatible helper concepts but not the same primary files; sequence them if conflicts appear.
- U5 may choose `aws-sdk-bedrockruntime`; if it changes Cargo dependencies, coordinate with U2 dependency edits.

### Wave 4 — Lazy/compat/OAuth cleanup

- After provider units and U8, run U9 and U10. U9 depends on APIs existing; U10 is mostly auth/OAuth scoped.

### Wave 5 — Final audit and integration validation

- Run U11 last. It owns global validation and final placeholder audit.

<a id="parallelization-constraints"></a>
## Parallelization Constraints

| Constraint | Reason | Affected units |
|---|---|---|
| U1 must run first | It confirms exact placeholder inventory and final file scopes. | All units |
| U2 before provider units | Provider units depend on `utils::genai_backend` and shared error/proxy hooks. | U3-U7 |
| U5 Cargo edits must coordinate with U2 | Bedrock fallback may add AWS SDK deps. | U2, U5 |
| U9 after provider units | Lazy/compat dispatch must point at final API functions. | U3-U7, U9 |
| U11 last | Owns global compile/test/audit. | All units |

<a id="canonical-line-references"></a>
## Canonical Line References

<!-- CANONICAL_LINE_REFERENCES_START -->
<!-- This block is generated by finalize-plan-lines.mjs. Do not edit manually. -->
| ID | Anchor | Lines | Description |
|---|---|---|---|
| how-to-use | #how-to-use | L3-L15 | How to use this plan |
| legend | #legend | L17-L51 | Legend |
| goal | #goal | L53-L58 | Goal |
| non-goals | #non-goals | L60-L68 | Non-goals |
| review-flags | #review-flags | L70-L81 | Review Flags |
| global-acceptance | #global-acceptance | L83-L91 | Global Acceptance Criteria |
| legacy-policy | #legacy-policy | L93-L102 | Legacy / workaround policy |
| breaking-changes | #breaking-changes | L104-L112 | Planned Breaking Changes and Propagation Map |
| orchestration | #orchestration | L114-L136 | Subagent Orchestration Plan |
| parallelization-constraints | #parallelization-constraints | L138-L147 | Parallelization Constraints |
| canonical-line-references | #canonical-line-references | L149-L156 | Canonical Line References |
| phase-compat-matrix | #phase-compat-matrix | L158-L229 | Phase 1 — Compatibility matrix and scope freeze |
| u1-compat-inventory | #u1-compat-inventory | L161-L229 | U1 — Build final placeholder compatibility inventory |
| phase-foundation | #phase-foundation | L231-L379 | Phase 2 — Shared backend and non-provider dependency foundations |
| u2-genai-backend | #u2-genai-backend | L234-L314 | U2 — Add internal `genai` backend, proxy, and error adapter foundation |
| u8-validation-json | #u8-validation-json | L316-L379 | U8 — Replace TypeBox and partial-json placeholders with local Rust validation/parsing |
| phase-provider-apis | #phase-provider-apis | L381-L633 | Phase 3 — Provider API placeholder fixes |
| u3-openai-family | #u3-openai-family | L384-L435 | U3 — Resolve OpenAI-family placeholders with `genai` or exact `reqwest` fallback |
| u4-anthropic | #u4-anthropic | L437-L485 | U4 — Resolve Anthropic Messages placeholders with raw SSE fallback where required |
| u5-bedrock | #u5-bedrock | L487-L535 | U5 — Resolve Bedrock placeholders with SigV4 parity spike and AWS SDK fallback |
| u6-google | #u6-google | L537-L585 | U6 — Resolve Google Gemini and Vertex placeholders |
| u7-mistral-gateways | #u7-mistral-gateways | L587-L633 | U7 — Resolve Mistral direct and OpenAI-compatible gateway placeholders |
| phase-dispatch-auth | #phase-dispatch-auth | L635-L720 | Phase 4 — Lazy dispatch, compat registry, and OAuth leftovers |
| u9-lazy-compat | #u9-lazy-compat | L638-L677 | U9 — Replace lazy import and compat/provider registry placeholders |
| u10-oauth-auth | #u10-oauth-auth | L679-L720 | U10 — Resolve OAuth/auth helper placeholders and residual manual blockers |
| phase-final-audit | #phase-final-audit | L722-L774 | Phase 5 — Final audit and integration validation |
| u11-final-audit | #u11-final-audit | L725-L774 | U11 — Final placeholder audit, tests, and residual risk ledger |
| pre-finalization-review | #pre-finalization-review | L776-L782 | Pre-finalization Review Summary |
<!-- CANONICAL_LINE_REFERENCES_END -->

<a id="phase-compat-matrix"></a>
## Phase 1 — Compatibility matrix and scope freeze

<a id="u1-compat-inventory"></a>
### U1 — Build final placeholder compatibility inventory

Assignable: yes

Wave: 1

Execution: fresh, sequential

Dependencies: none

Allowed parallelism: none

Prohibited parallelism: all implementation units

Goal: Produce a checked inventory of every `PORT PLACEHOLDER` in `crates/zedflow-ai`, grouped by original dependency/runtime source, Pi TS behavior reference, Rust replacement decision, file scope, and required tests.

Non-goals:
- Do not edit provider behavior.
- Do not add dependencies.
- Do not remove placeholders.

Files:
- read: `crates/zedflow-ai/src/**/*.rs`
- read: `crates/zedflow-ai/tests/**/*.rs`
- read: `references/pi/packages/ai/src/**/*.ts`
- create: `.agents/state/zedflow-ai-placeholder-compat-inventory.md`

Context package:
- Plan references: goal, review flags, global acceptance, this unit.
- Required docs/skills: read `/home/zedium/workspaces/zedflow/.agents/skills/rust-skills/SKILL.md`; read `https://github.com/jeremychone/rust-genai/blob/main/docs/for-llm/api-reference-for-llm.md` or local clone `/tmp/pi-github-repos/jeremychone/rust-genai/docs/for-llm/api-reference-for-llm.md` if present.
- Required files/symbols: all placeholder comments from `grep -R "PORT PLACEHOLDER" crates/zedflow-ai`.

Implementation outline:
1. Grep all placeholders in source and tests.
2. For each placeholder, identify original TS dependency and exact TS behavior lines.
3. Classify replacement: `genai`, `reqwest fallback`, `aws-sdk-bedrockruntime fallback`, `local helper`, `static dispatch`, or `accepted live/manual blocker`.
4. Write the inventory with exact file scopes for U2-U11.

Major snippets:

[CANONICAL] Required inventory columns:

```markdown
| Placeholder file:line | Original dependency/runtime | Pi TS reference | Required behavior | Rust replacement | Unit | Test/validation |
```

Validation responsibility:
- Type: locally-validating
- Must run: `grep -R "PORT PLACEHOLDER" crates/zedflow-ai/src crates/zedflow-ai/tests`
- Must NOT run: cargo global validation

Output contract:
- Path to `.agents/state/zedflow-ai-placeholder-compat-inventory.md`.
- List of any placeholder whose TS source behavior could not be located.
- Updated recommended scopes if this plan's scopes are wrong.

Acceptance criteria:
- Every placeholder is assigned to exactly one later unit or marked accepted residual.
- Every dependency replacement has a Pi TS reference.

Handoff:
- U2-U11 must read the inventory before editing.

Subagent prompt:

```text
You are executing U1 from .agents/plans/zedflow-ai-placeholder-deps-replacement.md. Run in fresh context. Read the plan sections for goal, review flags, global acceptance, and U1. Read /home/zedium/workspaces/zedflow/.agents/skills/rust-skills/SKILL.md. Read genai API docs at /tmp/pi-github-repos/jeremychone/rust-genai/docs/for-llm/api-reference-for-llm.md if present, otherwise fetch https://github.com/jeremychone/rust-genai/blob/main/docs/for-llm/api-reference-for-llm.md. Build .agents/state/zedflow-ai-placeholder-compat-inventory.md with every PORT PLACEHOLDER in crates/zedflow-ai, exact Pi TS behavior references from references/pi/packages/ai/src, replacement decision, owning unit, and validation. Do not edit implementation files. Do not run cargo global validation.
```

<a id="phase-foundation"></a>
## Phase 2 — Shared backend and non-provider dependency foundations

<a id="u2-genai-backend"></a>
### U2 — Add internal `genai` backend, proxy, and error adapter foundation

Assignable: yes

Wave: 2

Execution: fresh, parallel with U8 after U1

Dependencies: U1

Allowed parallelism: U8

Prohibited parallelism: U3-U7 until U2 completes

Goal: Add an internal `genai` backend helper under Pi-style file naming and shared transport utilities used by provider units.

Non-goals:
- Do not migrate provider API modules.
- Do not expose `genai` types publicly.
- Do not implement Bedrock fallback.

Files:
- modify: `crates/zedflow-ai/Cargo.toml`
- modify: `crates/zedflow-ai/src/lib.rs`
- create: `crates/zedflow-ai/src/utils/genai-backend.rs`
- modify: `crates/zedflow-ai/src/utils/node-http-proxy.rs`
- modify: `crates/zedflow-ai/src/utils/error-body.rs`
- read: `.agents/state/zedflow-ai-placeholder-compat-inventory.md`

Context package:
- Plan references: goal, RF-GENAI-HOOKS, RF-PROXY, global acceptance, U2.
- Required docs/skills: read rust-skills; read genai API reference; inspect `genai` source for `ClientBuilder`, `WebConfig`, `ChatOptions`, `Error`, and `webc::Error`.
- Required Pi TS references: `references/pi/packages/ai/src/utils/node-http-proxy.ts`, `references/pi/packages/ai/src/utils/error-body.ts`, `references/pi/packages/ai/src/types.ts` StreamOptions.

Implementation outline:
1. Add `genai` dependency with minimal features: `rustls-tls`; add `bedrock-sigv4` only if U5 does not own it in current Cargo state.
2. Expose module with Pi-style file naming: `#[path = "genai-backend.rs"] pub mod genai_backend;` inside `utils` in `lib.rs`.
3. Implement internal request config structs that consume Pi options without leaking `genai` publicly.
4. Implement provider/model mapping helpers and `genai::Client` builder with auth, endpoint, proxy, timeout, extra headers, and error conversion.
5. Complete Pi-compatible proxy resolver behavior before creating `reqwest::Proxy`.
6. Extend error normalization to preserve status/body/header data from `genai` and `reqwest`, with Pi's 4000-character truncation.

Major snippets:

[CANONICAL] Module naming:

```rust
#[path = "genai-backend.rs"]
pub mod genai_backend;
```

[CANONICAL] Public boundary:

```rust
// genai types stay inside utils::genai_backend and API modules.
// Do not add genai types to public zedflow_ai::types structs or public function signatures.
```

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; `cargo check -p zedflow-ai --all-targets`
- Must NOT run: live provider tests

Output contract:
- List dependencies added.
- List helper functions/types created.
- List any `genai` incompatibility discovered that changes U3-U7.

Acceptance criteria:
- `zedflow-ai` compiles with the new internal module.
- Proxy and error helpers have deterministic unit tests or existing tests updated.

Handoff:
- U3-U7 use `utils::genai_backend`; U11 audits no public `genai` exposure.

Subagent prompt:

```text
You are executing U2 from .agents/plans/zedflow-ai-placeholder-deps-replacement.md. Fresh context only. Read U1 inventory, rust-skills, genai API docs, and Pi TS references for node-http-proxy.ts, error-body.ts, and StreamOptions. Add internal crates/zedflow-ai/src/utils/genai-backend.rs with #[path = "genai-backend.rs"] pub mod genai_backend in lib.rs. Add minimal Cargo deps. Implement internal genai client/config/error/proxy helpers without exposing genai types publicly. Preserve Pi proxy env/no_proxy and error body formatting. Run cargo fmt --all --check and cargo check -p zedflow-ai --all-targets. Do not migrate provider API modules.
```

<a id="u8-validation-json"></a>
### U8 — Replace TypeBox and partial-json placeholders with local Rust validation/parsing

Assignable: yes

Wave: 2

Execution: fresh, parallel with U2 after U1

Dependencies: U1

Allowed parallelism: U2

Prohibited parallelism: U11

Goal: Resolve non-LLM placeholders for `typebox` and `partial-json` while preserving Pi schema representation, coercion, cleaning, validation, error formatting, and streaming partial JSON behavior.

Non-goals:
- Do not add `schemars` unless a deterministic test proves schema generation is required.
- Do not change provider streaming logic except validation/parser call sites.

Files:
- modify: `crates/zedflow-ai/src/types.rs`
- modify: `crates/zedflow-ai/src/utils/typebox-helpers.rs`
- modify: `crates/zedflow-ai/src/utils/validation.rs`
- modify: `crates/zedflow-ai/src/utils/json-parse.rs`
- read: `references/pi/packages/ai/src/utils/validation.ts`
- read: `references/pi/packages/ai/src/utils/json-parse.ts`
- read: `references/pi/packages/ai/src/utils/typebox-helpers.ts`
- read: `.agents/state/zedflow-ai-placeholder-compat-inventory.md`

Context package:
- Plan references: RF-TYPEBOX, global acceptance, U8.
- Required docs/skills: read rust-skills; genai docs are required by plan but only needed here to confirm no provider schema assumptions are changed.
- Required crates: existing `jsonschema`, `serde_json`.

Implementation outline:
1. Keep `ToolParametersSchema = serde_json::Value` unless inventory proves otherwise.
2. Port TypeBox validation semantics that Pi uses: validator cache if useful, primitive coercion, object/array clean, subschema validation, and helpful error messages.
3. Confirm partial JSON parser matches Pi's `partial-json` use: parse incomplete streamed tool args, retry with repair, return empty object fallback where Pi does.
4. Add deterministic tests from TS edge cases.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; targeted tests for `utils::validation`, `utils::json_parse`, `utils::typebox_helpers`
- Must NOT run: live provider tests

Output contract:
- List placeholders removed/reclassified.
- List tests added/updated.
- State whether `schemars` was avoided or why it was necessary.

Acceptance criteria:
- TypeBox placeholder in `types.rs` is replaced with a documented Rust decision, not an unresolved placeholder.
- Partial JSON tests cover incomplete object/tool args and repaired invalid escapes/control chars.

Handoff:
- Provider units may rely on validation and parser helpers.

Subagent prompt:

```text
You are executing U8 from .agents/plans/zedflow-ai-placeholder-deps-replacement.md. Fresh context only. Read U1 inventory, rust-skills, genai API docs, Pi validation.ts, json-parse.ts, typebox-helpers.ts, and the Rust files listed in U8. Replace TypeBox and partial-json placeholders with local Rust behavior preserving Pi coercion/clean/validation/error and partial JSON parsing. Prefer serde_json::Value + jsonschema; do not add schemars unless a test proves it is needed. Run fmt and targeted tests only.
```

<a id="phase-provider-apis"></a>
## Phase 3 — Provider API placeholder fixes

<a id="u3-openai-family"></a>
### U3 — Resolve OpenAI-family placeholders with `genai` or exact `reqwest` fallback

Assignable: yes

Wave: 3

Execution: fresh, parallel after U2

Dependencies: U1, U2

Allowed parallelism: U4, U5, U6, U7 if no file conflicts

Prohibited parallelism: U9, U11

Goal: Replace OpenAI, Azure OpenAI, OpenAI Codex, and OpenRouter image placeholders while preserving Pi cache/session/reasoning/hooks semantics.

Files:
- modify: `crates/zedflow-ai/src/api/openai-responses.rs`
- modify: `crates/zedflow-ai/src/api/openai-completions.rs`
- modify: `crates/zedflow-ai/src/api/openai-codex-responses.rs`
- modify: `crates/zedflow-ai/src/api/azure-openai-responses.rs`
- modify: `crates/zedflow-ai/src/api/openrouter-images.rs`
- modify if needed: `crates/zedflow-ai/src/api/openai-responses-shared.rs`
- read TS: `references/pi/packages/ai/src/api/openai-responses.ts`, `openai-completions.ts`, `openai-codex-responses.ts`, `azure-openai-responses.ts`, `openrouter-images.ts`

Required Pi behavior:
- `onPayload` before send and `onResponse` with status/headers.
- timeout and `maxRetries ?? 0`.
- session headers `session_id` and `x-client-request-id`.
- prompt cache key/retention and `cacheRetention === "none"` behavior.
- `max_output_tokens` clamped to at least 16.
- `store: false` unless Pi explicitly differs.
- encrypted reasoning include where Pi requests it.
- service-tier pricing multiplier and Copilot dynamic headers.

Implementation outline:
1. Use `genai` for standard calls only when hooks are absent and all required body/header fields can be represented.
2. Implement narrow `reqwest` fallback path for any call with `onPayload`, `onResponse`, or body fields not expressible through `genai`.
3. Keep Pi event conversion and usage/cost code in Rust API modules.
4. Add local payload tests comparing key fields to TS behavior.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; targeted OpenAI-family tests
- Must NOT run: live OpenAI/Azure/OpenRouter calls

Subagent prompt:

```text
Execute U3. Fresh context. Read U1 inventory, rust-skills, genai docs, U2 genai-backend, and Pi TS OpenAI-family files. Replace OpenAI-family PORT PLACEHOLDERs. Use genai only when Pi onPayload/onResponse and exact body/header behavior are not required; otherwise implement a narrow reqwest fallback. Preserve max_output_tokens>=16, store=false, prompt cache/session headers, reasoning encrypted_content, Copilot dynamic headers, service-tier pricing, timeout, no implicit retries, and Pi event/error formatting. Run fmt and targeted deterministic tests; no live provider tests.
```

<a id="u4-anthropic"></a>
### U4 — Resolve Anthropic Messages placeholders with raw SSE fallback where required

Assignable: yes

Wave: 3

Execution: fresh, parallel after U2

Dependencies: U1, U2, U8

Allowed parallelism: U3, U5, U6, U7 if no file conflicts

Prohibited parallelism: U9, U11

Goal: Replace Anthropic SDK placeholders while preserving Pi's raw SSE, thinking, cache-control, OAuth-client, beta-header, tool streaming, and usage behavior.

Files:
- modify: `crates/zedflow-ai/src/api/anthropic-messages.rs`
- modify: `crates/zedflow-ai/src/api/anthropic-messages.lazy.rs`
- modify if needed: `crates/zedflow-ai/src/utils/oauth/anthropic.rs`
- read TS: `references/pi/packages/ai/src/api/anthropic-messages.ts`

Required Pi behavior:
- raw SSE parsing from `.asResponse()` equivalent.
- ignore unknown SSE events; throw on `event: error`.
- error if `message_start` occurs without `message_stop`.
- usage captured from `message_start`, `message_delta`, including cache read/write and 1h details.
- thinking blocks and signatures preserved.
- interleaved/fine-grained tool streaming beta headers.
- Claude Code tool name casing compatibility.
- `onPayload`, `onResponse`, timeout, no implicit retries.

Implementation outline:
1. Use `genai` only for standard Anthropic streaming if it passes local fixture parity and hooks/beta/OAuth are not required.
2. Implement raw `reqwest` SSE fallback for Pi-exact behavior.
3. Keep local SSE decoder/fixtures so message_stop validation is deterministic.
4. Preserve existing ignored live/OAuth tests unless U10 resolves them.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; Anthropic fixture/unit tests
- Must NOT run: live Anthropic calls

Subagent prompt:

```text
Execute U4. Fresh context. Read U1 inventory, rust-skills, genai docs/source for Anthropic streamer, U2 backend, U8 parser helpers, and Pi anthropic-messages.ts. Replace Anthropic PORT PLACEHOLDERs. Preserve raw SSE semantics, message_stop validation, usage/cache details, thinking signatures, beta headers, tool name casing, onPayload/onResponse, timeout, and no implicit retries. Use genai only for proven standard-compatible path; otherwise implement narrow reqwest raw SSE fallback. Run fmt and deterministic fixture tests only.
```

<a id="u5-bedrock"></a>
### U5 — Resolve Bedrock placeholders with SigV4 parity spike and AWS SDK fallback

Assignable: yes

Wave: 3

Execution: fresh, parallel after U2

Dependencies: U1, U2

Allowed parallelism: U3, U4, U6, U7 if Cargo dependency edits are coordinated

Prohibited parallelism: U11

Goal: Replace Bedrock SDK/Smithy placeholders while preserving Pi AWS configuration, proxy, custom-header signing, payload hooks, response hooks, and Converse stream event mapping.

Files:
- modify: `crates/zedflow-ai/src/api/bedrock-converse-stream.rs`
- modify: `crates/zedflow-ai/src/api/bedrock-converse-stream.lazy.rs`
- modify: `crates/zedflow-ai/src/bedrock-provider.rs`
- modify if fallback chosen: `crates/zedflow-ai/Cargo.toml`
- modify tests: `crates/zedflow-ai/tests/bedrock-custom-headers.rs`
- read TS: `references/pi/packages/ai/src/api/bedrock-converse-stream.ts`, `references/pi/packages/ai/src/bedrock-provider.ts`

Required Pi behavior:
- AWS profile/env region rules, ARN region extraction, endpoint-region handling, fallback `us-east-1`.
- `AWS_BEDROCK_SKIP_AUTH`, dummy credentials, explicit credentials, and `AWS_BEARER_TOKEN_BEDROCK`.
- Pi proxy resolver and optional force HTTP/1 equivalent where possible.
- custom headers included in signed request while reserved SigV4 headers are ignored.
- `onPayload`, `onResponse`, error body/status formatting.
- interleaved thinking additional model request fields.

Implementation outline:
1. Spike `genai` `bedrock_sigv4` against deterministic request-building tests.
2. If it cannot prove all required behavior, add/use `aws-sdk-bedrockruntime` fallback scoped only to Bedrock.
3. Preserve custom headers test and add tests for reserved headers, region resolution, bearer/skip auth decisions.
4. Keep stream event mapping Pi-compatible.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; targeted Bedrock tests including custom headers
- Must NOT run: live AWS calls

Subagent prompt:

```text
Execute U5. Fresh context. Read U1 inventory, rust-skills, genai docs/source for bedrock_sigv4, U2 backend, Pi bedrock-converse-stream.ts, and bedrock-provider.ts. First prove whether genai Bedrock can preserve Pi region/profile/bearer/skip-auth/proxy/custom-signed-headers/onPayload/onResponse behavior. If not, implement a narrow aws-sdk-bedrockruntime fallback for Bedrock only. Preserve Pi stream events and error formatting. Run fmt and deterministic Bedrock tests; no live AWS calls.
```

<a id="u6-google"></a>
### U6 — Resolve Google Gemini and Vertex placeholders

Assignable: yes

Wave: 3

Execution: fresh, parallel after U2

Dependencies: U1, U2, U8

Allowed parallelism: U3, U4, U5, U7 if no file conflicts

Prohibited parallelism: U9, U11

Goal: Replace Google GenAI placeholders while preserving Pi Gemini/Vertex messages, tool schemas, thought signatures, usage, stop reasons, and hooks.

Files:
- modify: `crates/zedflow-ai/src/api/google-generative-ai.rs`
- modify: `crates/zedflow-ai/src/api/google-generative-ai.lazy.rs`
- modify: `crates/zedflow-ai/src/api/google-vertex.rs`
- modify: `crates/zedflow-ai/src/api/google-vertex.lazy.rs`
- modify if needed: `crates/zedflow-ai/src/api/google-shared.rs`
- read TS: `references/pi/packages/ai/src/api/google-generative-ai.ts`, `google-vertex.ts`, `google-shared.ts`

Required Pi behavior:
- `onPayload` support.
- thoughtSignature retention for thinking/text/tool parts.
- unique fallback tool call IDs for missing/duplicate provider IDs.
- usage metadata including cached token accounting.
- stop reason mapped to `toolUse` if tool calls exist.
- schema/tool conversion behavior from `google-shared.ts`.

Implementation outline:
1. Use `genai` for standard Gemini/Vertex where payload hook is absent and thought signatures are preserved.
2. Use `reqwest` fallback for exact `onPayload` behavior or missing signature/usage fields.
3. Keep Pi-specific tool ID fallback and stop reason overrides outside `genai`.
4. Add deterministic conversion/stream fixture tests.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; targeted Google/Vertex tests
- Must NOT run: live Google calls

Subagent prompt:

```text
Execute U6. Fresh context. Read U1 inventory, rust-skills, genai docs/source for Gemini/Vertex, U2 backend, U8 validation helpers, and Pi google-generative-ai.ts, google-vertex.ts, google-shared.ts. Replace Google placeholders preserving onPayload fallback, thought signatures, unique tool IDs, usage/cache metadata, toolUse stop reason, and schema/tool conversion. Use genai only where fidelity is proven; otherwise narrow reqwest fallback. Run fmt and deterministic tests only.
```

<a id="u7-mistral-gateways"></a>
### U7 — Resolve Mistral direct and OpenAI-compatible gateway placeholders

Assignable: yes

Wave: 3

Execution: fresh, parallel after U2

Dependencies: U1, U2, U8

Allowed parallelism: U3, U4, U5, U6 if no file conflicts

Prohibited parallelism: U9, U11

Goal: Replace Mistral and remaining OpenAI-compatible gateway placeholders with direct `reqwest` for Mistral and `genai` where gateway behavior is compatible.

Files:
- modify: `crates/zedflow-ai/src/api/mistral-conversations.rs`
- modify: `crates/zedflow-ai/src/api/mistral-conversations.lazy.rs`
- modify gateway providers only if inventory requires: `crates/zedflow-ai/src/providers/openrouter.rs`, `opencode.rs`, `groq.rs`, related provider files
- read TS: `references/pi/packages/ai/src/api/mistral-conversations.ts`

Required Pi behavior:
- direct Mistral API semantics, not only OpenAI-compatible approximation.
- 9-character normalized Mistral tool call IDs.
- promptMode/reasoningEffort mapping.
- partial streamed tool-args JSON parsing.
- `onPayload`, timeout, no retries.
- Mistral error formatting from `statusCode` and `body`.

Implementation outline:
1. Implement Mistral direct with `reqwest`; do not force `genai`.
2. Use U8 partial JSON parser for streamed tool args.
3. For gateways with documented `genai` adapters, use `genai` only if Pi behavior does not require final payload hooks or direct SDK semantics.
4. Add deterministic tests for tool ID normalization and error formatting.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; targeted Mistral/gateway tests
- Must NOT run: live Mistral/gateway calls

Subagent prompt:

```text
Execute U7. Fresh context. Read U1 inventory, rust-skills, genai docs for supported adapters/custom endpoints, U2 backend, U8 json parser, and Pi mistral-conversations.ts. Replace Mistral placeholders with a direct reqwest transport preserving 9-char tool IDs, reasoning mapping, partial JSON tool args, onPayload, no retries, timeout, and status/body errors. Use genai only for compatible gateway providers, not Mistral direct. Run fmt and deterministic tests only.
```

<a id="phase-dispatch-auth"></a>
## Phase 4 — Lazy dispatch, compat registry, and OAuth leftovers

<a id="u9-lazy-compat"></a>
### U9 — Replace lazy import and compat/provider registry placeholders

Assignable: yes

Wave: 4

Execution: fresh, parallel with U10 after provider units

Dependencies: U1, U3, U4, U5, U6, U7, U8

Allowed parallelism: U10

Prohibited parallelism: U11

Goal: Replace placeholders caused by JavaScript dynamic imports, lazy API wrappers, and provider/model registry compatibility after concrete APIs are available.

Files:
- modify: `crates/zedflow-ai/src/api/lazy.rs`
- modify: `crates/zedflow-ai/src/api/*.lazy.rs`
- modify: `crates/zedflow-ai/src/compat.rs`
- modify provider registry files identified by U1 inventory under `crates/zedflow-ai/src/providers/`
- read TS: `references/pi/packages/ai/src/api/lazy.ts`, `references/pi/packages/ai/src/compat.ts`, `references/pi/packages/ai/src/providers/**/*.ts`

Implementation outline:
1. Replace dynamic import placeholders with static Rust dispatch and `LazyLock`/`OnceLock` only where observable caching exists.
2. Wire `compat::get_providers`, `compat::get_models`, and lazy API construction to concrete Rust modules.
3. Remove placeholders only when provider/API behavior is implemented or mark live/manual residuals.
4. Do not change model catalog semantics.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; targeted compat/lazy tests
- Must NOT run: live provider tests

Subagent prompt:

```text
Execute U9. Fresh context. Read U1 inventory, rust-skills, genai docs, completed provider modules, and Pi lazy.ts/compat.ts/provider files. Replace JS dynamic import/lazy/compat placeholders with static Rust dispatch preserving observable Pi lazy API behavior and provider/model registry semantics. Use LazyLock/OnceLock only for observable caching. Run fmt and targeted tests; no live provider calls.
```

<a id="u10-oauth-auth"></a>
### U10 — Resolve OAuth/auth helper placeholders and residual manual blockers

Assignable: yes

Wave: 4

Execution: fresh, parallel with U9 after U1 and relevant provider units

Dependencies: U1, U4, U3

Allowed parallelism: U9

Prohibited parallelism: U11

Goal: Replace deterministic OAuth/auth helper placeholders while keeping live/browser/manual flows explicitly ignored when they cannot be tested locally.

Files:
- modify: `crates/zedflow-ai/src/cli.rs`
- modify: `crates/zedflow-ai/src/oauth.rs`
- modify: `crates/zedflow-ai/src/utils/oauth/anthropic.rs`
- modify: `crates/zedflow-ai/src/utils/oauth/openai-codex.rs`
- modify: `crates/zedflow-ai/src/utils/oauth/device-code.rs`
- modify tests as needed: `crates/zedflow-ai/tests/anthropic-oauth.rs`, Codex OAuth tests identified by U1
- read TS: `references/pi/packages/ai/src/oauth.ts`, `references/pi/packages/ai/src/utils/oauth/**/*.ts`, `references/pi/packages/ai/src/cli.ts`

Implementation outline:
1. Port deterministic token exchange, refresh, device-code polling, PKCE, and credential-store behavior with `reqwest`.
2. Preserve env/key resolution and error messages used by Pi.
3. Keep manual/browser login tests ignored with explicit residual reason if automation would require external interaction.
4. Ensure Anthropic OAuth client paths used by U4 have a concrete Rust representation or explicit accepted residual.

Validation responsibility:
- Type: locally-validating
- Must run: `cargo fmt --all --check`; deterministic OAuth/helper tests
- Must NOT run: live browser/manual OAuth tests

Subagent prompt:

```text
Execute U10. Fresh context. Read U1 inventory, rust-skills, genai docs as required by plan, Pi OAuth/CLI/auth files, and Rust OAuth modules. Replace deterministic OAuth/auth placeholders using reqwest and existing credential-store helpers. Preserve Pi env/key resolution and error messages. Keep live/manual/browser tests ignored with exact reasons. Run fmt and deterministic tests only.
```

<a id="phase-final-audit"></a>
## Phase 5 — Final audit and integration validation

<a id="u11-final-audit"></a>
### U11 — Final placeholder audit, tests, and residual risk ledger

Assignable: yes

Wave: 5

Execution: fresh, sequential last

Dependencies: U1, U2, U3, U4, U5, U6, U7, U8, U9, U10

Allowed parallelism: none

Prohibited parallelism: all other write units

Goal: Verify `zedflow-ai` placeholder replacement is complete, compile/test gates pass, and remaining placeholders are only accepted live/manual/provider gaps with explicit reasons.

Files:
- modify if needed: `crates/zedflow-ai/tests/**/*.rs`
- create: `.agents/state/zedflow-ai-placeholder-residuals.md`
- read: all changed files from U2-U10
- read: `.agents/state/zedflow-ai-placeholder-compat-inventory.md`

Implementation outline:
1. Run final placeholder grep and classify residuals.
2. Unignore tests whose blockers were resolved.
3. Ensure no public `genai` types leaked into `zedflow-ai` public API.
4. Run formatting, compile, and deterministic tests.
5. Write final residual ledger.

Validation responsibility:
- Type: integration-validating
- Must run: `cargo fmt --all --check`; `cargo check -p zedflow-ai --all-targets`; deterministic `cargo test -p zedflow-ai --all-targets` excluding live/manual ignored tests if necessary by normal Rust ignore semantics
- Must NOT run: live provider/network/OAuth tests unless already non-live deterministic

Output contract:
- Validation commands and results.
- Path to `.agents/state/zedflow-ai-placeholder-residuals.md`.
- Final list of remaining placeholders and accepted reasons.
- List of tests unignored or still ignored.

Acceptance criteria:
- Global acceptance criteria satisfied.
- No unresolved dependency placeholder remains without accepted residual reason.

Subagent prompt:

```text
Execute U11 last from .agents/plans/zedflow-ai-placeholder-deps-replacement.md. Fresh context. Read U1 inventory, rust-skills, genai docs, all changed modules, and all prior unit outputs. Run the final PORT PLACEHOLDER audit, unignore deterministic tests whose blockers were resolved, verify no public genai API leak, run cargo fmt --all --check, cargo check -p zedflow-ai --all-targets, and deterministic zedflow-ai tests. Do not run live provider/network/manual OAuth tests. Write .agents/state/zedflow-ai-placeholder-residuals.md with every remaining accepted placeholder/test ignore and reason.
```

<a id="pre-finalization-review"></a>
## Pre-finalization Review Summary

- Feasibility: `genai` documentation and source confirm support for multi-provider chat/streaming, custom auth/endpoints, proxy injection, headers, raw error bodies, prompt cache controls, tools, reasoning chunks, thought signatures, and Bedrock SigV4. They do not confirm Pi `onPayload`/`onResponse`, Mistral direct, raw Anthropic SSE contract, or full Bedrock Smithy parity, so those are planned as fallback paths.
- Scope isolation: shared utilities are separated from provider modules; provider units write disjoint primary files; final audit runs last.
- Naming: the backend file follows repo Pi-style naming as `utils/genai-backend.rs`, exposed internally as `genai_backend` via `#[path]`.
- Required context: every implementation unit requires rust-skills and `genai` API documentation before editing.
