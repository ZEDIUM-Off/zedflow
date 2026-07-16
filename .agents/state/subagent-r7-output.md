<!-- migration-document-status: COMPLETED ARTIFACT -->
> [!NOTE]
> **Migration status: COMPLETED ARTIFACT.** Raw historical evidence only; do not use it as current progress. See `.agents/state/zedflow-ai-agent-pi-fidelity-current-status.md`.

Implemented R7 faux provider accounting and typed event parity.

Changed files:
- crates/zedflow-ai/src/providers/faux.rs
- crates/zedflow-ai/tests/faux-provider.rs

What changed:
- Faux provider now emits public `types::AssistantMessageEvent` variants (`start`, `thinking_*`, `text_*`, `toolcall_*`, `done`, `error`) on the typed provider path instead of opaque faux event placeholders.
- Added Pi-style serialized-context token estimation for typed contexts, including system prompt, user text/images, assistant content, tool results, and tools JSON.
- Added per-session prompt cache common-prefix read/write simulation with `cacheRetention: none` opt-out.
- Included cache writes in `totalTokens`; model cost calculation includes input/output/cacheRead/cacheWrite using Pi `calculateCost` semantics.
- Preserved Rust-safe panic-to-error conversion for response factories.
- Did not implement live provider transports and did not edit compat registry.

Reactivated/covered faux tests:
- registers_a_custom_provider_and_estimates_usage
- supports_helper_blocks_for_text_thinking_and_tool_calls
- supports_multiple_models_with_per_model_reasoning_and_model_aware_factories
- rewrites_api_provider_and_model_on_returned_messages
- consumes_queued_responses_in_order_and_errors_when_exhausted
- can_replace_and_append_queued_responses
- emits_an_error_when_a_response_factory_panics
- simulates_prompt_caching_per_session_id
- does_not_simulate_caching_when_cache_retention_is_none
- estimates_prompt_and_output_tokens_from_serialized_context
- includes_cache_write_in_total_tokens_and_cost
- streams_typed_thinking_text_and_toolcall_events_in_order
- streams_multiple_tool_calls_in_one_message
- supports_aborting_before_the_first_chunk
- unregisters_the_provider

Still not representable:
- JS async Promise response factories: Rust faux factories are synchronous closures; panic-to-error is covered.
- Paced mid-stream abort timing: Rust faux streams are in-memory/synchronous, so there is no live scheduler/tokensPerSecond delay point for mid-text/thinking/toolcall aborts.

Validation:
- `cargo test -p zedflow-ai --test faux-provider`: passed, 15 passed, 4 ignored.
- `cargo fmt --all --check`: passed.
- `git diff --cached --name-only`: empty; no staged files.

Residual risks:
- The legacy compat/lazy `Context` remains structurally empty, so exact serialized prompt accounting is exercised through the typed provider/model path.
- Working tree contains many pre-existing unrelated modified/untracked files; R7 edits are limited to the two files above.
