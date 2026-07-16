# Zedflow AI full-fidelity validation (AI-M1)

**Date:** 2026-07-13
**Result:** PASS — deterministic gate closed; live capabilities absent and therefore not run.

## Manifest closure

- Pi AI test rows: **98**.
- Unique Rust targets present: **98/98**.
- Missing targets: **0**.
- Additional Rust-only targets: `pi_harness_selftest.rs`, `public-api.rs`, `stream-events.rs`.

| # | Pi source | Rust target | Present |
|---:|---|---|:---:|
| 1 | `test/abort.test.ts` | `crates/zedflow-ai/tests/abort.rs` | yes |
| 2 | `test/anthropic-adaptive-thinking-models.test.ts` | `crates/zedflow-ai/tests/anthropic-adaptive-thinking-models.rs` | yes |
| 3 | `test/anthropic-cache-write-1h-cost.test.ts` | `crates/zedflow-ai/tests/anthropic-cache-write-1h-cost.rs` | yes |
| 4 | `test/anthropic-eager-tool-input-compat.test.ts` | `crates/zedflow-ai/tests/anthropic-eager-tool-input-compat.rs` | yes |
| 5 | `test/anthropic-eager-tool-input-e2e.test.ts` | `crates/zedflow-ai/tests/anthropic-eager-tool-input-e2e.rs` | yes |
| 6 | `test/anthropic-empty-thinking-signature-compat.test.ts` | `crates/zedflow-ai/tests/anthropic-empty-thinking-signature-compat.rs` | yes |
| 7 | `test/anthropic-force-adaptive-thinking.test.ts` | `crates/zedflow-ai/tests/anthropic-force-adaptive-thinking.rs` | yes |
| 8 | `test/anthropic-long-cache-retention-e2e.test.ts` | `crates/zedflow-ai/tests/anthropic-long-cache-retention-e2e.rs` | yes |
| 9 | `test/anthropic-oauth.test.ts` | `crates/zedflow-ai/tests/anthropic-oauth.rs` | yes |
| 10 | `test/anthropic-opus-4-8-smoke.test.ts` | `crates/zedflow-ai/tests/anthropic-opus-4-8-smoke.rs` | yes |
| 11 | `test/anthropic-sse-parsing.test.ts` | `crates/zedflow-ai/tests/anthropic-sse-parsing.rs` | yes |
| 12 | `test/anthropic-temperature-compat.test.ts` | `crates/zedflow-ai/tests/anthropic-temperature-compat.rs` | yes |
| 13 | `test/anthropic-thinking-disable.test.ts` | `crates/zedflow-ai/tests/anthropic-thinking-disable.rs` | yes |
| 14 | `test/anthropic-tool-name-normalization.test.ts` | `crates/zedflow-ai/tests/anthropic-tool-name-normalization.rs` | yes |
| 15 | `test/azure-openai-base-url.test.ts` | `crates/zedflow-ai/tests/azure-openai-base-url.rs` | yes |
| 16 | `test/azure-utils.ts` | `crates/zedflow-ai/tests/azure-utils.rs` | yes |
| 17 | `test/bedrock-convert-messages.test.ts` | `crates/zedflow-ai/tests/bedrock-convert-messages.rs` | yes |
| 18 | `test/bedrock-custom-headers.test.ts` | `crates/zedflow-ai/tests/bedrock-custom-headers.rs` | yes |
| 19 | `test/bedrock-endpoint-resolution.test.ts` | `crates/zedflow-ai/tests/bedrock-endpoint-resolution.rs` | yes |
| 20 | `test/bedrock-models.test.ts` | `crates/zedflow-ai/tests/bedrock-models.rs` | yes |
| 21 | `test/bedrock-thinking-payload.test.ts` | `crates/zedflow-ai/tests/bedrock-thinking-payload.rs` | yes |
| 22 | `test/bedrock-utils.ts` | `crates/zedflow-ai/tests/bedrock-utils.rs` | yes |
| 23 | `test/cache-retention.test.ts` | `crates/zedflow-ai/tests/cache-retention.rs` | yes |
| 24 | `test/cloudflare-utils.ts` | `crates/zedflow-ai/tests/cloudflare-utils.rs` | yes |
| 25 | `test/codex-websocket-cached-probe.ts` | `crates/zedflow-ai/tests/codex-websocket-cached-probe.rs` | yes |
| 26 | `test/compat-env.test.ts` | `crates/zedflow-ai/tests/compat-env.rs` | yes |
| 27 | `test/context-overflow.test.ts` | `crates/zedflow-ai/tests/context-overflow.rs` | yes |
| 28 | `test/cross-provider-handoff.test.ts` | `crates/zedflow-ai/tests/cross-provider-handoff.rs` | yes |
| 29 | `test/empty.test.ts` | `crates/zedflow-ai/tests/empty.rs` | yes |
| 30 | `test/env-api-keys.test.ts` | `crates/zedflow-ai/tests/env-api-keys.rs` | yes |
| 31 | `test/error-body.test.ts` | `crates/zedflow-ai/tests/error-body.rs` | yes |
| 32 | `test/faux-provider.test.ts` | `crates/zedflow-ai/tests/faux-provider.rs` | yes |
| 33 | `test/fireworks-models.test.ts` | `crates/zedflow-ai/tests/fireworks-models.rs` | yes |
| 34 | `test/github-copilot-anthropic.test.ts` | `crates/zedflow-ai/tests/github-copilot-anthropic.rs` | yes |
| 35 | `test/github-copilot-oauth.test.ts` | `crates/zedflow-ai/tests/github-copilot-oauth.rs` | yes |
| 36 | `test/google-shared-convert-tools.test.ts` | `crates/zedflow-ai/tests/google-shared-convert-tools.rs` | yes |
| 37 | `test/google-shared-gemini3-unsigned-tool-call.test.ts` | `crates/zedflow-ai/tests/google-shared-gemini3-unsigned-tool-call.rs` | yes |
| 38 | `test/google-shared-image-tool-result-routing.test.ts` | `crates/zedflow-ai/tests/google-shared-image-tool-result-routing.rs` | yes |
| 39 | `test/google-thinking-disable.test.ts` | `crates/zedflow-ai/tests/google-thinking-disable.rs` | yes |
| 40 | `test/google-thinking-signature.test.ts` | `crates/zedflow-ai/tests/google-thinking-signature.rs` | yes |
| 41 | `test/google-vertex-api-key-resolution.test.ts` | `crates/zedflow-ai/tests/google-vertex-api-key-resolution.rs` | yes |
| 42 | `test/image-tool-result.test.ts` | `crates/zedflow-ai/tests/image-tool-result.rs` | yes |
| 43 | `test/images-models.test.ts` | `crates/zedflow-ai/tests/images-models.rs` | yes |
| 44 | `test/images.test.ts` | `crates/zedflow-ai/tests/images.rs` | yes |
| 45 | `test/interleaved-thinking.test.ts` | `crates/zedflow-ai/tests/interleaved-thinking.rs` | yes |
| 46 | `test/lax-message-content.test.ts` | `crates/zedflow-ai/tests/lax-message-content.rs` | yes |
| 47 | `test/lazy-module-load.test.ts` | `crates/zedflow-ai/tests/lazy-module-load.rs` | yes |
| 48 | `test/mistral-reasoning-mode.test.ts` | `crates/zedflow-ai/tests/mistral-reasoning-mode.rs` | yes |
| 49 | `test/mistral-tool-schema.test.ts` | `crates/zedflow-ai/tests/mistral-tool-schema.rs` | yes |
| 50 | `test/models-runtime.test.ts` | `crates/zedflow-ai/tests/models-runtime.rs` | yes |
| 51 | `test/node-http-proxy.test.ts` | `crates/zedflow-ai/tests/node-http-proxy.rs` | yes |
| 52 | `test/oauth-auth.test.ts` | `crates/zedflow-ai/tests/oauth-auth.rs` | yes |
| 53 | `test/oauth-device-code.test.ts` | `crates/zedflow-ai/tests/oauth-device-code.rs` | yes |
| 54 | `test/oauth.ts` | `crates/zedflow-ai/tests/oauth.rs` | yes |
| 55 | `test/openai-codex-cache-affinity-e2e.test.ts` | `crates/zedflow-ai/tests/openai-codex-cache-affinity-e2e.rs` | yes |
| 56 | `test/openai-codex-oauth.test.ts` | `crates/zedflow-ai/tests/openai-codex-oauth.rs` | yes |
| 57 | `test/openai-codex-stream.test.ts` | `crates/zedflow-ai/tests/openai-codex-stream.rs` | yes |
| 58 | `test/openai-completions-cache-control-format.test.ts` | `crates/zedflow-ai/tests/openai-completions-cache-control-format.rs` | yes |
| 59 | `test/openai-completions-empty-tools.test.ts` | `crates/zedflow-ai/tests/openai-completions-empty-tools.rs` | yes |
| 60 | `test/openai-completions-prompt-cache.test.ts` | `crates/zedflow-ai/tests/openai-completions-prompt-cache.rs` | yes |
| 61 | `test/openai-completions-reasoning-details.test.ts` | `crates/zedflow-ai/tests/openai-completions-reasoning-details.rs` | yes |
| 62 | `test/openai-completions-response-model.test.ts` | `crates/zedflow-ai/tests/openai-completions-response-model.rs` | yes |
| 63 | `test/openai-completions-retry.test.ts` | `crates/zedflow-ai/tests/openai-completions-retry.rs` | yes |
| 64 | `test/openai-completions-thinking-as-text.test.ts` | `crates/zedflow-ai/tests/openai-completions-thinking-as-text.rs` | yes |
| 65 | `test/openai-completions-tool-choice.test.ts` | `crates/zedflow-ai/tests/openai-completions-tool-choice.rs` | yes |
| 66 | `test/openai-completions-tool-result-images.test.ts` | `crates/zedflow-ai/tests/openai-completions-tool-result-images.rs` | yes |
| 67 | `test/openai-responses-cache-affinity-e2e.test.ts` | `crates/zedflow-ai/tests/openai-responses-cache-affinity-e2e.rs` | yes |
| 68 | `test/openai-responses-copilot-provider.test.ts` | `crates/zedflow-ai/tests/openai-responses-copilot-provider.rs` | yes |
| 69 | `test/openai-responses-empty-tool-result.test.ts` | `crates/zedflow-ai/tests/openai-responses-empty-tool-result.rs` | yes |
| 70 | `test/openai-responses-foreign-toolcall-id.test.ts` | `crates/zedflow-ai/tests/openai-responses-foreign-toolcall-id.rs` | yes |
| 71 | `test/openai-responses-message-id.test.ts` | `crates/zedflow-ai/tests/openai-responses-message-id.rs` | yes |
| 72 | `test/openai-responses-partial-json-cleanup.test.ts` | `crates/zedflow-ai/tests/openai-responses-partial-json-cleanup.rs` | yes |
| 73 | `test/openai-responses-reasoning-replay-e2e.test.ts` | `crates/zedflow-ai/tests/openai-responses-reasoning-replay-e2e.rs` | yes |
| 74 | `test/openai-responses-terminal-event.test.ts` | `crates/zedflow-ai/tests/openai-responses-terminal-event.rs` | yes |
| 75 | `test/openai-responses-tool-result-images.test.ts` | `crates/zedflow-ai/tests/openai-responses-tool-result-images.rs` | yes |
| 76 | `test/openrouter-cache-write-repro.test.ts` | `crates/zedflow-ai/tests/openrouter-cache-write-repro.rs` | yes |
| 77 | `test/openrouter-images.test.ts` | `crates/zedflow-ai/tests/openrouter-images.rs` | yes |
| 78 | `test/overflow.test.ts` | `crates/zedflow-ai/tests/overflow.rs` | yes |
| 79 | `test/provider-error-body-passthrough.test.ts` | `crates/zedflow-ai/tests/provider-error-body-passthrough.rs` | yes |
| 80 | `test/provider-error-body-regression.test.ts` | `crates/zedflow-ai/tests/provider-error-body-regression.rs` | yes |
| 81 | `test/providers.test.ts` | `crates/zedflow-ai/tests/providers.rs` | yes |
| 82 | `test/responseid.test.ts` | `crates/zedflow-ai/tests/responseid.rs` | yes |
| 83 | `test/retry.test.ts` | `crates/zedflow-ai/tests/retry.rs` | yes |
| 84 | `test/scratch.ts` | `crates/zedflow-ai/tests/scratch.rs` | yes |
| 85 | `test/stream.test.ts` | `crates/zedflow-ai/tests/stream.rs` | yes |
| 86 | `test/supports-xhigh.test.ts` | `crates/zedflow-ai/tests/supports-xhigh.rs` | yes |
| 87 | `test/together-models.test.ts` | `crates/zedflow-ai/tests/together-models.rs` | yes |
| 88 | `test/tokens.test.ts` | `crates/zedflow-ai/tests/tokens.rs` | yes |
| 89 | `test/tool-call-id-normalization.test.ts` | `crates/zedflow-ai/tests/tool-call-id-normalization.rs` | yes |
| 90 | `test/tool-call-without-result.test.ts` | `crates/zedflow-ai/tests/tool-call-without-result.rs` | yes |
| 91 | `test/total-tokens.test.ts` | `crates/zedflow-ai/tests/total-tokens.rs` | yes |
| 92 | `test/transform-messages-copilot-openai-to-anthropic.test.ts` | `crates/zedflow-ai/tests/transform-messages-copilot-openai-to-anthropic.rs` | yes |
| 93 | `test/unicode-surrogate.test.ts` | `crates/zedflow-ai/tests/unicode-surrogate.rs` | yes |
| 94 | `test/validation.test.ts` | `crates/zedflow-ai/tests/validation.rs` | yes |
| 95 | `test/xhigh.test.ts` | `crates/zedflow-ai/tests/xhigh.rs` | yes |
| 96 | `test/xiaomi-models.test.ts` | `crates/zedflow-ai/tests/xiaomi-models.rs` | yes |
| 97 | `test/xiaomi-token-plan-ams-anthropic-empty-signature-smoke.test.ts` | `crates/zedflow-ai/tests/xiaomi-token-plan-ams-anthropic-empty-signature-smoke.rs` | yes |
| 98 | `test/zen.test.ts` | `crates/zedflow-ai/tests/zen.rs` | yes |

## Deterministic gates

| Gate | Outcome |
|---|---|
| `cargo fmt --all --check` | pass |
| `cargo check -p zedflow-ai --all-targets` | pass; warnings deferred to R-AI |
| `cargo test -p zedflow-ai --all-targets` | pass: 848 passed, 0 failed, 51 ignored across 107 harness summaries |
| `cargo doc -p zedflow-ai --no-deps` | pass |
| Public `genai` leak grep | pass; only `pub(crate)` internal occurrence |
| `git diff --check` / staged files | pass / 0 |

AI-U5 uses the production state machine through an injected monotonic clock/sleep seam; its pending, delayed-first-poll, RFC `slow_down`, server interval, expiration, and abort tests complete in 0.00s.

## Capability inventory

No live capability was available: Anthropic, OpenAI, Gemini/Google, Mistral, xAI, AWS/Bedrock, Azure OpenAI, OpenCode, and Xiaomi environment credentials were absent; `~/.pi/agent/oauth.json` was absent. No live endpoint was called.

## Ignore disposition

- Current ignores: **51** = **46 live-capability**, **3 JS-only**, **2 upstream-skipped**, **0 deterministic implementation-gap**.
- Every live group has the named active production-path capture/serializer/parser evidence shown below. Capability-absent commands are recorded as `not-run`; expected outcome with capability is pass.

| Rust function | Class | Exact reason | Named captured/nearest-observable evidence | Live command / outcome |
|---|---|---|---|---|
| `crates/zedflow-ai/tests/anthropic-eager-tool-input-e2e.rs::generated_compat_settings_accept_configured_tool_streaming` | live-capability | live capability: requires ANTHROPIC_API_KEY and network | `anthropic-eager-tool-input-e2e (3 active request/header tests)` | `cargo test -p zedflow-ai --test anthropic-eager-tool-input-e2e -- --ignored --exact generated_compat_settings_accept_configured_tool_streaming`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/anthropic-eager-tool-input-e2e.rs::forced_eager_input_streaming_probe_accepts_forced_eager_input_streaming` | live-capability | live capability: requires ANTHROPIC_API_KEY and network | `anthropic-eager-tool-input-e2e (3 active request/header tests)` | `cargo test -p zedflow-ai --test anthropic-eager-tool-input-e2e -- --ignored --exact forced_eager_input_streaming_probe_accepts_forced_eager_input_streaming`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/anthropic-long-cache-retention-e2e.rs::forced_long_cache_retention_probe_accepts_long_cache_retention` | live-capability | live capability: requires provider credentials and network | `anthropic-long-cache-retention-e2e + anthropic-cache-write-1h-cost` | `cargo test -p zedflow-ai --test anthropic-long-cache-retention-e2e -- --ignored --exact forced_long_cache_retention_probe_accepts_long_cache_retention`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/anthropic-opus-4-8-smoke.rs::streams_claude_opus_4_8_with_reasoning_enabled` | live-capability | live capability: requires ANTHROPIC_API_KEY and network | `anthropic-adaptive-thinking-models + anthropic-sse-parsing` | `cargo test -p zedflow-ai --test anthropic-opus-4-8-smoke -- --ignored --exact streams_claude_opus_4_8_with_reasoning_enabled`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/anthropic-thinking-disable.rs::disables_thinking_for_claude_reasoning_models` | live-capability | live Anthropic API parity test skipped: requires ANTHROPIC_API_KEY and provider network calls | `anthropic-thinking-disable (7 active payload-capture tests)` | `cargo test -p zedflow-ai --test anthropic-thinking-disable -- --ignored --exact disables_thinking_for_claude_reasoning_models`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/bedrock-models.rs::makes_a_simple_request_with_each_bedrock_model_live_parity` | live-capability | live Bedrock provider parity test skipped: missing AWS Bedrock credentials/network capability or BEDROCK_EXTENSIVE_MODEL_TEST | `bedrock-models::gets_all_available_bedrock_models + bedrock-convert-messages` | `cargo test -p zedflow-ai --test bedrock-models -- --ignored --exact makes_a_simple_request_with_each_bedrock_model_live_parity`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/bedrock-thinking-payload.rs::uses_model_max_tokens_cap_instead_of_bedrock_4096_token_default_for_adaptive_claude_models` | live-capability | live Bedrock provider parity test skipped: missing AWS Bedrock credentials/network capability | `bedrock-thinking-payload (10 active payload-capture tests)` | `cargo test -p zedflow-ai --test bedrock-thinking-payload -- --ignored --exact uses_model_max_tokens_cap_instead_of_bedrock_4096_token_default_for_adaptive_claude_models`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/context-overflow.rs::context_overflow_live_provider_matrix_is_blocked` | live-capability | live provider/network/local-LLM parity test skipped; see LIVE_PROVIDER_BLOCKER | `context-overflow (3 active vectors) + overflow` | `cargo test -p zedflow-ai --test context-overflow -- --ignored --exact context_overflow_live_provider_matrix_is_blocked`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/cross-provider-handoff.rs::should_have_at_least_2_fixtures_to_test_handoffs` | live-capability | live provider test skipped; compat catalog/provider dispatch, OAuth resolveApiKey, completeSimple, and real provider streams are request-capture blockers | `transform-messages-copilot-openai-to-anthropic + openai-responses-reasoning-replay-e2e active captures` | `cargo test -p zedflow-ai --test cross-provider-handoff -- --ignored --exact should_have_at_least_2_fixtures_to_test_handoffs`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/cross-provider-handoff.rs::should_handle_cross_provider_handoffs_for_each_target` | live-capability | live provider test skipped; handoff requests require generated live fixtures and completeSimple provider calls | `transform-messages-copilot-openai-to-anthropic + openai-responses-reasoning-replay-e2e active captures` | `cargo test -p zedflow-ai --test cross-provider-handoff -- --ignored --exact should_handle_cross_provider_handoffs_for_each_target`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/empty.rs::provider_empty_content_array_live_parity` | live-capability | live provider parity test; see BLOCKER | `openai-completions-empty-tools + openai-responses-empty-tool-result` | `cargo test -p zedflow-ai --test empty -- --ignored --exact provider_empty_content_array_live_parity`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/empty.rs::provider_empty_string_content_live_parity` | live-capability | live provider parity test; see BLOCKER | `openai-completions-empty-tools + openai-responses-empty-tool-result` | `cargo test -p zedflow-ai --test empty -- --ignored --exact provider_empty_string_content_live_parity`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/empty.rs::provider_whitespace_only_content_live_parity` | live-capability | live provider parity test; see BLOCKER | `openai-completions-empty-tools + openai-responses-empty-tool-result` | `cargo test -p zedflow-ai --test empty -- --ignored --exact provider_whitespace_only_content_live_parity`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/empty.rs::provider_empty_assistant_message_in_conversation_live_parity` | live-capability | live provider parity test; see BLOCKER | `openai-completions-empty-tools + openai-responses-empty-tool-result` | `cargo test -p zedflow-ai --test empty -- --ignored --exact provider_empty_assistant_message_in_conversation_live_parity`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/image-tool-result.rs::image_tool_result_only_image_across_live_providers` | live-capability | live provider call skipped; image tool-result context, typed assistant content, and provider streaming are request-capture blockers | `google-shared-image-tool-result-routing + openai-{completions,responses}-tool-result-images + openrouter-images` | `cargo test -p zedflow-ai --test image-tool-result -- --ignored --exact image_tool_result_only_image_across_live_providers`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/image-tool-result.rs::image_tool_result_text_and_image_across_live_providers` | live-capability | live provider call skipped; text+image tool-result context, typed assistant content, and provider streaming are request-capture blockers | `google-shared-image-tool-result-routing + openai-{completions,responses}-tool-result-images + openrouter-images` | `cargo test -p zedflow-ai --test image-tool-result -- --ignored --exact image_tool_result_text_and_image_across_live_providers`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/image-tool-result.rs::image_tool_result_xiaomi_text_and_image_upstream_skip` | upstream-skipped | matches Pi it.skip: Xiaomi MiMo text+image tool results currently ignore image color due upstream multimodal-fusion quality | `google-shared-image-tool-result-routing + openai-{completions,responses}-tool-result-images + openrouter-images` | n/a; not-run; matches explicit upstream skip |
| `crates/zedflow-ai/tests/interleaved-thinking.rs::bedrock_interleaved_thinking_on_claude_opus_4_5` | live-capability | live Bedrock provider call skipped; requires credentials/network and completeSimple/provider streaming parity not yet ported | `anthropic-sse-parsing + bedrock-thinking-payload` | `cargo test -p zedflow-ai --test interleaved-thinking -- --ignored --exact bedrock_interleaved_thinking_on_claude_opus_4_5`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/interleaved-thinking.rs::bedrock_interleaved_thinking_on_claude_opus_4_6` | live-capability | live Bedrock provider call skipped; requires credentials/network and completeSimple/provider streaming parity not yet ported | `anthropic-sse-parsing + bedrock-thinking-payload` | `cargo test -p zedflow-ai --test interleaved-thinking -- --ignored --exact bedrock_interleaved_thinking_on_claude_opus_4_6`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/interleaved-thinking.rs::anthropic_interleaved_thinking_on_claude_opus_4_5` | live-capability | live Anthropic provider call skipped; requires credentials/network and completeSimple/provider streaming parity not yet ported | `anthropic-sse-parsing + bedrock-thinking-payload` | `cargo test -p zedflow-ai --test interleaved-thinking -- --ignored --exact anthropic_interleaved_thinking_on_claude_opus_4_5`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/interleaved-thinking.rs::anthropic_interleaved_thinking_on_claude_opus_4_6` | live-capability | live Anthropic provider call skipped; requires credentials/network and completeSimple/provider streaming parity not yet ported | `anthropic-sse-parsing + bedrock-thinking-payload` | `cargo test -p zedflow-ai --test interleaved-thinking -- --ignored --exact anthropic_interleaved_thinking_on_claude_opus_4_6`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/lazy-module-load.rs::lazy_provider_module_loading_loads_only_anthropic_sdk_when_streaming_through_lazy_api_wrapper` | JS-only | JS-only: Node registerHooks can observe that exactly @anthropic-ai/sdk is imported; Rust static linking has no runtime SDK specifier list | `active production-path provider/utility suite` | n/a; active analogue passed |
| `crates/zedflow-ai/tests/lazy-module-load.rs::lazy_provider_module_loading_loads_only_anthropic_sdk_when_dispatching_through_stream_simple` | JS-only | JS-only: Node registerHooks can observe that compat.streamSimple imports exactly one SDK; Rust static linking has no equivalent module-load hook | `active production-path provider/utility suite` | n/a; active analogue passed |
| `crates/zedflow-ai/tests/openai-responses-cache-affinity-e2e.rs::handles_direct_openai_responses_requests_with_aligned_cache_affinity_identifiers` | live-capability | live provider call skipped; see BLOCKER | `openai-completions-prompt-cache + openai-responses-cache-affinity-e2e active capture` | `cargo test -p zedflow-ai --test openai-responses-cache-affinity-e2e -- --ignored --exact handles_direct_openai_responses_requests_with_aligned_cache_affinity_identifiers`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/openai-responses-reasoning-replay-e2e.rs::skips_reasoning_only_history_after_an_aborted_turn` | live-capability | live provider call skipped; see BLOCKER | `openai-responses-reasoning-replay-e2e active deterministic captures` | `cargo test -p zedflow-ai --test openai-responses-reasoning-replay-e2e -- --ignored --exact skips_reasoning_only_history_after_an_aborted_turn`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/openai-responses-reasoning-replay-e2e.rs::handles_same_provider_different_model_handoff_with_tool_calls` | live-capability | live provider call skipped; see BLOCKER | `openai-responses-reasoning-replay-e2e active deterministic captures` | `cargo test -p zedflow-ai --test openai-responses-reasoning-replay-e2e -- --ignored --exact handles_same_provider_different_model_handoff_with_tool_calls`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/openai-responses-reasoning-replay-e2e.rs::handles_cross_provider_handoff_from_anthropic_to_openai_codex` | live-capability | live provider call skipped; see BLOCKER | `openai-responses-reasoning-replay-e2e active deterministic captures` | `cargo test -p zedflow-ai --test openai-responses-reasoning-replay-e2e -- --ignored --exact handles_cross_provider_handoff_from_anthropic_to_openai_codex`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/responseid.rs::google_provider_exposes_response_id` | live-capability | live Google Gemini API parity test skipped: requires GEMINI_API_KEY and provider network calls | `responseid active local HTTP/SSE captures (OpenAI, Codex, Vertex)` | `cargo test -p zedflow-ai --test responseid -- --ignored --exact google_provider_exposes_response_id`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/responseid.rs::anthropic_provider_exposes_response_id` | live-capability | live Anthropic API parity test skipped: requires ANTHROPIC_API_KEY and provider network calls | `responseid active local HTTP/SSE captures (OpenAI, Codex, Vertex)` | `cargo test -p zedflow-ai --test responseid -- --ignored --exact anthropic_provider_exposes_response_id`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/responseid.rs::azure_openai_responses_provider_exposes_response_id` | live-capability | live Azure OpenAI Responses parity test skipped: requires Azure OpenAI credentials and provider network calls | `responseid active local HTTP/SSE captures (OpenAI, Codex, Vertex)` | `cargo test -p zedflow-ai --test responseid -- --ignored --exact azure_openai_responses_provider_exposes_response_id`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/responseid.rs::mistral_provider_exposes_response_id` | live-capability | live Mistral API parity test skipped: requires MISTRAL_API_KEY and provider network calls | `responseid active local HTTP/SSE captures (OpenAI, Codex, Vertex)` | `cargo test -p zedflow-ai --test responseid -- --ignored --exact mistral_provider_exposes_response_id`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/responseid.rs::github_copilot_openai_path_exposes_response_id` | live-capability | live GitHub Copilot OpenAI-path parity test skipped: requires resolved github-copilot OAuth token and provider network calls | `responseid active local HTTP/SSE captures (OpenAI, Codex, Vertex)` | `cargo test -p zedflow-ai --test responseid -- --ignored --exact github_copilot_openai_path_exposes_response_id`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/responseid.rs::github_copilot_anthropic_path_exposes_response_id` | live-capability | live GitHub Copilot Anthropic-path parity test skipped: requires resolved github-copilot OAuth token and provider network calls | `responseid active local HTTP/SSE captures (OpenAI, Codex, Vertex)` | `cargo test -p zedflow-ai --test responseid -- --ignored --exact github_copilot_anthropic_path_exposes_response_id`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/scratch.rs::scratch_models_api_anthropic_smoke_is_live_provider_sample` | live-capability | live Anthropic scratch script requires ANTHROPIC_API_KEY; provider/auth/completeSimple/streamSimple remain blocked | `anthropic-sse-parsing + providers registered-dispatch tests` | `cargo test -p zedflow-ai --test scratch -- --ignored --exact scratch_models_api_anthropic_smoke_is_live_provider_sample`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/stream.rs::runs_generate_e2e_stream_provider_matrix` | live-capability | live provider/local Ollama E2E suite skipped; see BLOCKER | `faux-provider + provider-specific local HTTP/SSE transport suites` | `cargo test -p zedflow-ai --test stream -- --ignored --exact runs_generate_e2e_stream_provider_matrix`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/tokens.rs::provider_token_stats_on_abort_live_parity` | live-capability | live provider parity test skipped; see BLOCKER | `tokens active usage vectors + faux-provider pacing/abort vectors` | `cargo test -p zedflow-ai --test tokens -- --ignored --exact provider_token_stats_on_abort_live_parity`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/tokens.rs::xiaomi_token_stats_on_abort_remain_upstream_blocked` | upstream-skipped | source test has explicit it.skip Xiaomi cases; see upstream_skip_reason | `tokens active usage vectors + faux-provider pacing/abort vectors` | n/a; not-run; matches explicit upstream skip |
| `crates/zedflow-ai/tests/tool-call-id-normalization.rs::github_copilot_to_openrouter_should_normalize_pipe_separated_ids` | live-capability | live provider parity test; see BLOCKER | `tool-call-id-normalization (5 active transform vectors)` | `cargo test -p zedflow-ai --test tool-call-id-normalization -- --ignored --exact github_copilot_to_openrouter_should_normalize_pipe_separated_ids`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/tool-call-id-normalization.rs::github_copilot_to_openai_codex_should_normalize_pipe_separated_ids` | live-capability | live provider parity test; see BLOCKER | `tool-call-id-normalization (5 active transform vectors)` | `cargo test -p zedflow-ai --test tool-call-id-normalization -- --ignored --exact github_copilot_to_openai_codex_should_normalize_pipe_separated_ids`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/tool-call-id-normalization.rs::openrouter_should_handle_prefilled_context_with_long_pipe_separated_ids` | live-capability | live provider parity test; see BLOCKER | `tool-call-id-normalization (5 active transform vectors)` | `cargo test -p zedflow-ai --test tool-call-id-normalization -- --ignored --exact openrouter_should_handle_prefilled_context_with_long_pipe_separated_ids`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/tool-call-id-normalization.rs::openai_codex_should_handle_prefilled_context_with_long_pipe_separated_ids` | live-capability | live provider parity test; see BLOCKER | `tool-call-id-normalization (5 active transform vectors)` | `cargo test -p zedflow-ai --test tool-call-id-normalization -- --ignored --exact openai_codex_should_handle_prefilled_context_with_long_pipe_separated_ids`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/tool-call-without-result.rs::live_provider_tool_call_without_result_suite_is_represented` | live-capability | live provider parity suite needs compat getModel/complete and network credentials; see BLOCKER | `tool-call-without-result (3 active transform/manifest vectors)` | `cargo test -p zedflow-ai --test tool-call-without-result -- --ignored --exact live_provider_tool_call_without_result_suite_is_represented`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/total-tokens.rs::total_tokens_live_provider_parity` | live-capability | live provider parity test skipped; see BLOCKER | `faux-provider usage vectors + provider SSE usage suites` | `cargo test -p zedflow-ai --test total-tokens -- --ignored --exact total_tokens_live_provider_parity`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/unicode-surrogate.rs::handles_emoji_in_tool_results` | live-capability | live provider call skipped; see BLOCKER | `sanitize_unicode unit tests + provider request serializer suites` | `cargo test -p zedflow-ai --test unicode-surrogate -- --ignored --exact handles_emoji_in_tool_results`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/unicode-surrogate.rs::handles_real_world_linkedin_comment_data_with_emoji` | live-capability | live provider call skipped; see BLOCKER | `sanitize_unicode unit tests + provider request serializer suites` | `cargo test -p zedflow-ai --test unicode-surrogate -- --ignored --exact handles_real_world_linkedin_comment_data_with_emoji`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/unicode-surrogate.rs::handles_unpaired_high_surrogate_in_tool_results` | JS-only | live provider call skipped; Rust cannot construct JS lone-surrogate strings and compat/tool-result context is incomplete; see BLOCKER | `sanitize_unicode unit tests + provider request serializer suites` | n/a; active analogue passed |
| `crates/zedflow-ai/tests/xhigh.rs::codex_max_supports_xhigh_on_openai_responses` | live-capability | live OpenAI Codex/Responses transport test; requires capability-gated OpenAI Codex credentials | `supports-xhigh + OpenAI Codex/Responses request-capture suites` | `cargo test -p zedflow-ai --test xhigh -- --ignored --exact codex_max_supports_xhigh_on_openai_responses`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/xhigh.rs::gpt_5_mini_errors_with_xhigh_on_openai_responses` | live-capability | live OpenAI Responses xhigh error test; requires OPENAI_API_KEY and network | `supports-xhigh + OpenAI Codex/Responses request-capture suites` | `cargo test -p zedflow-ai --test xhigh -- --ignored --exact gpt_5_mini_errors_with_xhigh_on_openai_responses`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/xhigh.rs::gpt_5_mini_errors_with_xhigh_on_openai_completions` | live-capability | live OpenAI Completions xhigh error test; requires OPENAI_API_KEY and network | `supports-xhigh + OpenAI Codex/Responses request-capture suites` | `cargo test -p zedflow-ai --test xhigh -- --ignored --exact gpt_5_mini_errors_with_xhigh_on_openai_completions`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/xiaomi-token-plan-ams-anthropic-empty-signature-smoke.rs::preserves_empty_thinking_signature_for_replay` | live-capability | parity blocker: live Xiaomi Token Plan Anthropic transport and replay payload capture are not ported | `anthropic-empty-thinking-signature-compat` | `cargo test -p zedflow-ai --test xiaomi-token-plan-ams-anthropic-empty-signature-smoke -- --ignored --exact preserves_empty_thinking_signature_for_replay`; not-run (capability absent); expected pass |
| `crates/zedflow-ai/tests/zen.rs::opencode_models_smoke_suite_requires_live_completion_dispatch` | live-capability | live OpenCode smoke requires provider network credentials; deterministic catalog coverage is local | `models-runtime + providers catalog/dispatch tests` | `cargo test -p zedflow-ai --test zen -- --ignored --exact opencode_models_smoke_suite_requires_live_completion_dispatch`; not-run (capability absent); expected pass |

## Acceptance

- Zero missing AI test targets.
- Zero deterministic implementation-gap ignores.
- Zero unmapped live-path groups.
- Zero capability-present live failures (no live capability present).
- AI-M1 hands a frozen deterministic behavior surface to R-AI.
