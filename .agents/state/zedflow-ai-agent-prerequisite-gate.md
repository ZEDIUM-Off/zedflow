<!-- migration-document-status: ACTIVE COMPLETE GATE -->
> [!IMPORTANT]
> **Migration status: ACTIVE COMPLETE GATE — GO.** This report is the final AI prerequisite decision. Earlier NO-GO sections are retained below as historical diagnostic evidence only.

# AI-V1 prerequisite gate

**Status: GO.** Complete deterministic `zedflow-ai` acceptance passes. The two prior OpenAI transport hangs are closed, the 98-row test manifest is complete, all remaining ignores are capability/JS/upstream dispositions rather than deterministic implementation gaps, Rust quality gates are clean, and the only `zedflow-agent` check errors are the pre-assigned fallible-hook propagation items.

## 2026-07-13 final AI-V1 gate

### Decision

- **AI-V1: GO.** `cargo test -p zedflow-ai --all-targets` completed normally after the single-pass Responses correction: 845 passed, 0 failed, 51 ignored across 107 harness summaries.
- AI manifest equality is 98/98 with zero missing targets. AI-M1 classifies the 51 ignores as 46 live-capability, 3 JS-only, 2 upstream-skipped, and 0 deterministic implementation-gap.
- No live call ran: relevant environment credentials and `~/.pi/agent/oauth.json` were absent.
- R-AI fmt/check/doc/no-deps-clippy gates pass. The unused compiled `genai` backend/dependency was removed from the crate build; direct Reqwest/Tokio features are explicit.
- `zedflow-agent` remains intentionally unmodified. Its all-target check reports only the two planned AG-H2 result-propagation mismatches, duplicated for lib/lib-test.

### Post-gate Pi stream-fidelity correction

A direct comparison with Pi found that `openai-responses.ts` and `azure-openai-responses.ts` each pass the provider async iterable once to `processResponsesStream`, whose slot/tool-call state persists for the full stream. Rust had been replaying every accumulated event after each SSE frame. The shared Rust processor is now stateful and both registered transports process each provider event once; the accumulated-event clones and `emitted_count` replay bookkeeping are gone. A regression test proves incremental processing produces exactly the same output and event sequence as the batch wrapper. The full AI gate was rerun after this production correction.

### Accepted public boundary

- Canonical identity is `zedflow_ai::types::{Model, Context, StreamOptions, SimpleStreamOptions, AssistantMessageEvent, AssistantMessageEventStream, StreamFunction, ProviderStreams}`. `models.rs`, `index.rs`, the crate root, compat, provider registration, and `zedflow-agent/src/types.rs` reuse that boundary.
- `api::lazy` defines no duplicate public struct, enum, or alias. It accepts canonical types, returns the actual canonical stream immediately, preserves delayed incremental delivery, and settles setup failures with exactly one terminal `Error`.
- Provider-specific API modules retain wire/request DTOs, but they do not form a second registry/event/stream universe and are not used by Agent as canonical contracts.
- `StreamFunction` remains immediate; asynchronous workers produce events. Fallible async payload/response hooks preserve `ProviderHookError` sources.

### Final commands

All final commands used `CARGO_TARGET_DIR=/tmp/zedflow-ai-v1-final-target` and `TMPDIR=/tmp/zedflow-ai-v1-final-tmp`.

| Command | Exit/status | Evidence |
|---|---:|---|
| `cargo fmt --all --check` | 0 | No formatting diff. |
| `cargo check -p zedflow-ai --all-targets` | 0 | Clean build. |
| `cargo test -p zedflow-ai --all-targets` | 0 | 845 passed, 0 failed, 51 ignored; both historically hanging targets complete. |
| `cargo doc -p zedflow-ai --no-deps` | 0 | Documentation builds. |
| `cargo clippy -p zedflow-ai --all-targets --no-deps -- -D warnings` | 0 | No issues. |
| manifest/ignore audit | 0 | 98/98 targets; 51 fully dispositioned ignores; 0 deterministic gaps. |
| `cargo check -p zedflow-agent --all-targets` | 101, planned | Only four emitted `E0271` diagnostics from two AG-H2 mismatches at `agent-harness.rs:1222` and `:1234`. |
| `git diff --check` and staged-file audit | 0 | No whitespace errors; 0 staged files. |

### Agent propagation list

1. `crates/zedflow-agent/src/harness/agent-harness.rs:1222`: payload hook returns `Option<Value>` instead of `Result<Option<Value>, ProviderHookError>`.
2. `crates/zedflow-agent/src/harness/agent-harness.rs:1234`: response hook returns `()` instead of `Result<(), ProviderHookError>`.

These are assigned Agent propagation work; they do not require an AI compatibility alias or AI production change.

### Handoff

`zedflow-ai` is frozen and accepted for the planned Agent phase. **AG-C1 may start.**

## Historical prior NO-GO evidence

The sections below are preserved unchanged. Their `NO-GO`, legacy-lazy, warning, ignore, and hang statements are superseded by the final gate above.

## 2026-07-13 conclusive AI-V1 gate

### Decision

- **AI-V1: NO-GO.** `cargo test -p zedflow-ai --all-targets` again had no completion. Inspection of the still-running Cargo process found its child blocked in the `openai-completions-response-model` target. Bounded single-target execution reproduced the failure, and exhaustive bounded target execution found a second equivalent blocker in `responseid`.
- **AI-C1-C3 prerequisite behavior is accepted in isolation.** The owned faux/abort suites pass 26/26 with zero owned ignores; public hook, stream, model, provider, and facade targets also pass in bounded execution.
- **AG-C1 handoff is stable.** Agent compilation exposes only the two planned AG-H2 fallible-hook propagation mismatches described below. No duplicate AI primitive or compatibility alias is needed.
- The NO-GO is caused by required broad-gate blockers, not by planned agent propagation and not by the legacy-lazy limitation.

### Accepted public boundary

- Canonical identity is `zedflow_ai::types::{Model, Context, StreamOptions, SimpleStreamOptions, AssistantMessageEvent, AssistantMessageEventStream, StreamFunction}`. `models.rs` re-exports those exact types, `index.rs`/the crate root re-export the same public types, provider registration stores canonical `types::ProviderStreams`, and `zedflow-agent/src/types.rs` directly re-exports/reuses them. The agent does not define a second message/model/stream universe.
- `StreamFunction` remains immediate, returning the canonical `AssistantMessageEventStream`; worker setup and event production may continue asynchronously. The stream queues/wakes incremental events, resolves its result from exactly one `Done` or `Error`, ignores post-terminal pushes, and `end(Some(message))` emits the terminal variant matching `StopReason`.
- Payload and response hooks are async and fallible: they return source-preserving `Result` values using `ProviderHookError`. Rejection is represented by one terminal error with no later event.
- The canonical faux provider path receives real `Context`, `Model`, and `StreamOptions`, supports async/fallible factories, yields paced deltas incrementally, maintains usage/cache/session accounting, aborts immediately or after the current partial delta, emits one terminal abort, and permits the next request with a fresh signal.

### Legacy `api::lazy` disposition

- **Not an AI-V1 acceptance blocker.** `api::lazy::{Context, StreamOptions, SimpleStreamOptions}` remain zero-sized synchronous legacy facade types, so that facade cannot provide Pi async factory or request context/options fidelity. AI-C3's strict three-file ownership could not replace that separate legacy API, and the limitation is explicitly surfaced by the legacy faux path (async factories report that the typed event stream is required).
- AI-V1 accepts only the canonical typed boundary consumed by `Models` and `zedflow-agent`; all AI-C3 fidelity assertions run through that boundary. The legacy facade remains a documented compatibility limitation and must not be presented as canonical or full Pi fidelity.

### Agent propagation list

`cargo check -p zedflow-agent --all-targets` exits 101 with exactly two distinct errors, each duplicated for the lib and lib-test targets:

1. `crates/zedflow-agent/src/harness/agent-harness.rs:1222`: payload-hook async block returns `Option<Value>` but canonical AI requires `Result<Option<Value>, ProviderHookError>`.
2. `crates/zedflow-agent/src/harness/agent-harness.rs:1234`: response-hook async block returns `()` but canonical AI requires `Result<(), ProviderHookError>`.

These are the exact planned AI-C1-to-AG-H2 propagation items. There are no unexpected agent errors.

### Unexpected blockers

1. `tests/openai-completions-response-model.rs::live_http_sse_transport_preserves_response_id_usage_and_hooks` hangs. Bounded execution prints a panic from the unnamed worker at Tokio `runtime/blocking/shutdown.rs:51`: `Cannot drop a runtime in a context where blocking is not allowed`, then times out with exit 124 because the worker never emits a terminal event and `stream.result()` remains pending. This is real OpenAI Completions transport/runtime behavior, explicitly outside AI-C1's provider-transport non-goal and owned by later provider unit AI-P8.
2. `tests/responseid.rs::openai_responses_live_transport_exposes_response_id` fails identically and times out with exit 124. This is later OpenAI Responses transport behavior owned by AI-P9.

The first blocker explains the exact broad-command hang trajectory; the second was found only after bounded per-target isolation. No source or test behavior was edited because both are outside AI-V1.

### Current commands and results

All Cargo commands used `CARGO_TARGET_DIR=/tmp/zedflow-ai-v1-target` and `TMPDIR=/tmp/zedflow-ai-v1-tmp`.

| Command | Exit/status | Evidence |
|---|---:|---|
| `cargo fmt --all --check` | 0 | Passed with no formatting diff. |
| `cargo check -p zedflow-ai --all-targets` | 0 | Passed; warnings only. |
| `cargo test -p zedflow-ai --test faux-provider --test abort -- --nocapture` | 0 | 23 faux + 3 abort = 26 passed, 0 failed, 0 ignored. |
| `cargo test -p zedflow-ai --all-targets` | no final status | Reproduced the prior non-completing trajectory; diagnostically interrupted rather than claiming success. Process inspection identified `openai_completions_response_model` as the active child. |
| `cargo test -p zedflow-ai --all-targets --no-run` | 0 | Enumerated 75 test executables. |
| `timeout 90s /tmp/zedflow-ai-v1-target/debug/deps/zedflow_ai-186d12678eb11008 --test-threads=1 --nocapture` | 0 | 391 passed, 0 failed, 0 ignored. |
| bounded direct execution of the other compiled targets with `--test-threads=1` and a 45-second per-target timeout | mixed | 72 targets completed: 345 passed, 60 ignored by the default harness, 0 failed. No ignored test was requested. Two targets timed out as isolated below. Including completed/partial targets, 742 passing tests were observed before blockers. |
| `timeout 45s .../openai_completions_response_model-9a34d2220f5e2df2 --test-threads=1 --nocapture` | 124 | 3 tests passed, then the named live-local HTTP test panicked its worker and hung. |
| `timeout 45s .../responseid-0e8566a06056a0e2 --exact openai_responses_live_transport_exposes_response_id --nocapture` | 124 | Worker runtime-drop panic followed by unresolved stream result. |
| `cargo check -p zedflow-agent --all-targets` | 101, planned | Only the two distinct AG-H2 hook-result mismatches; duplicated for lib/lib-test. |
| `git diff --check && git diff --cached --quiet` | 0 | No whitespace errors and no staged files. |

### Blocker disposition and next gate

- Do not begin AG-C1 under an accepted AI-V1 banner while the required AI broad gate lacks a final status.
- AI-P8 and AI-P9 must make their local deterministic transport workers terminate without Tokio blocking-runtime drop panic and always resolve the public stream on worker failure. Then rerun the exact required all-target command and record its final exit status.
- The planned AG-H2 propagation remains documented and must not be patched in AI-V1.

## Historical evidence

The sections below are retained unchanged from earlier attempts. Their stale ignore/timing observations are superseded by the conclusive report above.

## 2026-07-13 verification refresh

- `cargo test -p zedflow-ai --test faux-provider --test abort`: 20 passed, 3 ignored.
- `cargo check -p zedflow-agent --all-targets`: expected propagation failure at `harness/agent-harness.rs:1222` and `:1234`.
- Next: close or explicitly reconcile the AI-C3 abort ignores, then rerun a diagnosable AI all-target gate.

The command table below is retained as historical evidence from the original AI-V1 attempt.

## Remediation boundary

- `compat::tests::builtin_catalog_models_short_circuit_through_models_when_registry_is_unchanged` now expects AI-C2 builtin selected-API dispatch to return the terminal error result and error event.
- The Mistral Tokio worker awaits fallible payload hooks, then uses `tokio::task::spawn_blocking` for all blocking Reqwest client/request/response work. It returns that normal or error outcome to the worker, which emits exactly one terminal event.
- Public `types::AssistantMessageEventStream::end(Some(message))` now translates the result into one canonical `Done` or `Error` event based on `StopReason`; its result remains the terminal message. `end(None)` remains an explicit result-less close.
- No provider transports, compatibility aliases, or agent code were added.

## Commands

| Command | Status | Evidence |
|---|---|---|
| `cargo fmt --all --check` | passed | No formatting diff. |
| `cargo check -p zedflow-ai --all-targets` | passed | Completed with inherited warnings only. |
| `cargo test -p zedflow-ai --lib utils::event_stream::tests -- --nocapture` | passed | 3 passed. |
| `cargo test -p zedflow-ai --test stream-events -- --nocapture` | passed | 6 passed, including direct `end(Some(_))` normal/error terminal-event coverage. |
| `cargo test -p zedflow-ai --lib mistral_stream_error_uses_status_and_body_without_sdk_validation_text -- --nocapture` | passed | 1 passed; status error reached a terminal result without Tokio Reqwest panic. |
| `cargo test -p zedflow-ai --lib stream_uses_reqwest_transport_payload_hook_partial_json_and_usage -- --nocapture` | passed | 1 passed; normal SSE result and exactly one terminal event. |
| `cargo test -p zedflow-ai --lib builtin_catalog_models_short_circuit_through_models_when_registry_is_unchanged -- --nocapture` | passed | 1 passed; builtin selected API yields terminal error/result. |
| `cargo test -p zedflow-ai --all-targets` | timed out | Runner timeout after 900 seconds; no final suite result available. |
| `cargo check -p zedflow-agent --all-targets` | expected planned failure | Only AG-H2 hook result propagation errors at `src/harness/agent-harness.rs:1222` and `:1234` (duplicated for target builds). |
| `git diff --check && git diff --cached --quiet` | passed | No whitespace errors; no staged files. |

## Residual boundary

The required full AI all-target gate needs rerun in an environment that permits the complete suite to finish. The only agent check failures remain the explicitly planned AG-H2 hook-result propagation work; they were not modified.
