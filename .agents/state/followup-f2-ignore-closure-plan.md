<!-- migration-document-status: SUPERSEDED / HISTORICAL -->
> [!CAUTION]
> **Migration status: SUPERSEDED / HISTORICAL.** Do not use this file as the current execution tracker. See `.agents/state/zedflow-ai-agent-pi-fidelity-current-status.md`.

# Code Context

## Files Retrieved
1. `.agents/state/zedflow-ai-pi-ai-runtime-drift-final-report.md` (lines 1-180) - baseline drift verdict, validation, and remaining ignore list.
2. `crates/zedflow-ai/tests/openai-codex-stream.rs` (lines 680-699) - failing deterministic zstd compression ignore.
3. `crates/zedflow-ai/tests/provider-error-body-regression.rs` (line 183) - stale ignored Bedrock error-body regression; passes when run with `--ignored`.
4. `crates/zedflow-ai/tests/**/*.rs` ignore attributes - 78 remaining ignored tests under `crates/zedflow-ai/tests`.

## Key Code

Critical non-live residuals:

```rust
// crates/zedflow-ai/tests/openai-codex-stream.rs:680
#[ignore = "Codex request body compression is outside stream/event parity until a zstd dependency or byte-body seam is approved"]
fn zstd_compresses_sse_request_bodies() { ... }
```

```text
cargo test -p zedflow-ai --test openai-codex-stream zstd_compresses_sse_request_bodies -- --ignored --nocapture
FAILED: left None, right Some("zstd") for content-encoding
```

```text
cargo test -p zedflow-ai --test provider-error-body-regression bedrock_body_blind_surfaces_the_gateway_body_instead_of_unknown_unknown_error -- --ignored --nocapture
passed: 1 passed, 3 filtered out
```

## Architecture

The ignore set is mostly parity coverage around provider transports. Deterministic local tests already cover catalogs, payload conversion, faux provider behavior, and error normalization. Remaining ignored tests fall into six buckets:

### 1. Live/capability
Provider/network credential gates; keep ignored unless live acceptance mode supplies credentials and runs `--ignored` selectively.

- `abort.rs`: `provider_abort_mid_stream_live_parity`, `provider_immediate_abort_live_parity`, `bedrock_abort_then_new_message_live_parity`
- `anthropic-eager-tool-input-e2e.rs`: `generated_compat_settings_accept_configured_tool_streaming`, `forced_eager_input_streaming_probe_accepts_forced_eager_input_streaming`
- `anthropic-long-cache-retention-e2e.rs`: `forced_long_cache_retention_probe_accepts_long_cache_retention`
- `anthropic-opus-4-8-smoke.rs`: `streams_claude_opus_4_8_with_reasoning_enabled`
- `anthropic-thinking-disable.rs`: `disables_thinking_for_claude_reasoning_models`
- `bedrock-models.rs`: `makes_a_simple_request_with_each_bedrock_model_live_parity`
- `bedrock-thinking-payload.rs`: `uses_model_max_tokens_cap_instead_of_bedrock_4096_token_default_for_adaptive_claude_models`
- `context-overflow.rs`: `context_overflow_live_provider_matrix_is_blocked`
- `cross-provider-handoff.rs`: `should_have_at_least_2_fixtures_to_test_handoffs`, `should_handle_cross_provider_handoffs_for_each_target`
- `empty.rs`: `provider_empty_content_array_live_parity`, `provider_empty_string_content_live_parity`, `provider_whitespace_only_content_live_parity`, `provider_empty_assistant_message_in_conversation_live_parity`
- `google-thinking-disable.rs`: all 9 ignored provider/API tests
- `image-tool-result.rs`: `image_tool_result_only_image_across_live_providers`, `image_tool_result_text_and_image_across_live_providers`
- `interleaved-thinking.rs`: all 4 Bedrock/Anthropic live tests
- `openai-responses-cache-affinity-e2e.rs`: `handles_direct_openai_responses_requests_with_aligned_cache_affinity_identifiers`
- `openai-responses-reasoning-replay-e2e.rs`: all 3 live replay/handoff tests
- `responseid.rs`: all 8 ignored provider response-id tests
- `scratch.rs`: `scratch_models_api_anthropic_smoke_is_live_provider_sample`
- `stream.rs`: `runs_generate_e2e_stream_provider_matrix`
- `tokens.rs`: `provider_token_stats_on_abort_live_parity`
- `tool-call-id-normalization.rs`: all 4 live handoff normalization tests
- `tool-call-without-result.rs`: `live_provider_tool_call_without_result_suite_is_represented`
- `total-tokens.rs`: `total_tokens_live_provider_parity`
- `unicode-surrogate.rs`: `handles_emoji_in_tool_results`, `handles_real_world_linkedin_comment_data_with_emoji`
- `xhigh.rs`: all 3 live OpenAI/Codex xhigh tests
- `zen.rs`: `opencode_models_smoke_suite_requires_live_completion_dispatch`

### 2. JS-only/upstream-skipped
No Rust implementation needed for acceptance; document as intentionally nonportable/upstream skipped.

- `lazy-module-load.rs`: both Node `registerHooks` module-load observability tests
- `tokens.rs`: `xiaomi_token_stats_on_abort_remain_upstream_blocked`
- `image-tool-result.rs`: `image_tool_result_xiaomi_text_and_image_upstream_skip`

### 3. Accepted Rust limitation
Rust has no separate observable behavior, or cannot construct the JS edge exactly.

- `faux-provider.rs`: `supports_async_response_factories`
- `faux-provider.rs`: `supports_aborting_mid_text_stream_when_paced`, `supports_aborting_mid_thinking_stream_when_paced`, `supports_aborting_mid_toolcall_stream_when_paced`
- `unicode-surrogate.rs`: `handles_unpaired_high_surrogate_in_tool_results`

### 4. Stale ignore removable now
Smallest immediate cleanup: remove this ignore only. It already passes under `--ignored`.

- `provider-error-body-regression.rs`: `bedrock_body_blind_surfaces_the_gateway_body_instead_of_unknown_unknown_error`

### 5. Small deterministic implementation
No provider credentials needed; add the narrow missing seam/logic, then unignore.

- `openai-codex-stream.rs`: `zstd_compresses_sse_request_bodies` — request capture shows no `content-encoding: zstd`; implement Codex request-body zstd compression or approve a byte-body seam/dependency.
- `models-runtime.rs`: `models_runtime_wraps_credential_store_failures_in_models_error` — add a fallible injected credential-store test hook.
- `models-runtime.rs`: `models_runtime_wraps_api_key_auth_failures_in_models_error` — add provider-auth resolver injection/failing resolver hook.
- `providers.rs`: `create_provider_merges_provider_resolved_env_into_stream_options` — expose minimal ProviderAuth resolver injection to observe request/env merge deterministically.

### 6. Larger provider implementation
These need real provider dispatch/transport/catalog work before deterministic or live acceptance can be meaningful.

- `anthropic-long-cache-retention-e2e.rs`: `covers_every_generated_anthropic_messages_model`
- `cache-retention.rs`: `anthropic_adds_cache_control_to_string_user_messages`
- `github-copilot-anthropic.rs`: `uses_bearer_auth_copilot_headers_and_valid_anthropic_messages_payload`, `omits_interleaved_thinking_beta_for_adaptive_thinking_models`
- `xiaomi-token-plan-ams-anthropic-empty-signature-smoke.rs`: `preserves_empty_thinking_signature_for_replay`

## Smallest set of changes to reach full global acceptance

1. Remove stale ignore in `crates/zedflow-ai/tests/provider-error-body-regression.rs` for `bedrock_body_blind_surfaces_the_gateway_body_instead_of_unknown_unknown_error`.
2. Implement Codex zstd request compression in the OpenAI Codex request path used by `crates/zedflow-ai/tests/openai-codex-stream.rs::zstd_compresses_sse_request_bodies`.
3. Add minimal deterministic auth injection hooks for:
   - `crates/zedflow-ai/tests/models-runtime.rs::{models_runtime_wraps_credential_store_failures_in_models_error, models_runtime_wraps_api_key_auth_failures_in_models_error}`
   - `crates/zedflow-ai/tests/providers.rs::create_provider_merges_provider_resolved_env_into_stream_options`
4. Implement Anthropic Messages request construction/capture and generated compat catalog wiring for:
   - `cache-retention.rs::anthropic_adds_cache_control_to_string_user_messages`
   - `github-copilot-anthropic.rs::{uses_bearer_auth_copilot_headers_and_valid_anthropic_messages_payload, omits_interleaved_thinking_beta_for_adaptive_thinking_models}`
   - `anthropic-long-cache-retention-e2e.rs::covers_every_generated_anthropic_messages_model`
   - `xiaomi-token-plan-ams-anthropic-empty-signature-smoke.rs::preserves_empty_thinking_signature_for_replay`
5. Keep live/capability, JS-only/upstream-skipped, and accepted Rust-limitation tests ignored but record them as accepted exclusions; if full acceptance means literally zero ignored tests, replace those ignores with capability-skipping test bodies instead of `#[ignore]`.

## Start Here

Start at `crates/zedflow-ai/tests/openai-codex-stream.rs::zstd_compresses_sse_request_bodies`: it is deterministic, currently fails, and has the smallest product decision surface after the one-line stale-ignore removal.

## Supervisor coordination

No supervisor decision was needed. No repository files were edited; only this requested report artifact was written.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Inspected final drift report and all 78 remaining #[ignore] attributes under crates/zedflow-ai/tests; classified each by file/test name without editing repository files."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "Report includes file ranges, commands, validation output, residual risks, and exact smallest closure set."
    }
  ],
  "changedFiles": [
    "/home/zedium/workspaces/zedflow/.pi-subagents/artifacts/outputs/cba6fbab-94bb-4840-acac-db851d1294c9/.agents/state/followup-f2-ignore-closure-plan.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "grep/read/find inspection of .agents/state/zedflow-ai-pi-ai-runtime-drift-final-report.md and crates/zedflow-ai/tests",
      "result": "passed",
      "summary": "Found 78 remaining ignore attributes under crates/zedflow-ai/tests."
    },
    {
      "command": "cargo test -p zedflow-ai --test provider-error-body-regression bedrock_body_blind_surfaces_the_gateway_body_instead_of_unknown_unknown_error -- --ignored --nocapture",
      "result": "passed",
      "summary": "1 passed, 3 filtered out; ignore is stale/removable."
    },
    {
      "command": "cargo test -p zedflow-ai --test openai-codex-stream zstd_compresses_sse_request_bodies -- --ignored --nocapture",
      "result": "failed",
      "summary": "Fails because content-encoding header is None instead of Some(\"zstd\")."
    },
    {
      "command": "git diff --cached --quiet",
      "result": "passed",
      "summary": "No staged files."
    }
  ],
  "validationOutput": [
    "Ignore count script reported 78 #[ignore] attributes in crates/zedflow-ai/tests.",
    "provider-error-body-regression ignored test passes under --ignored.",
    "openai-codex-stream zstd ignored test fails with left None, right Some(\"zstd\")."
  ],
  "residualRisks": [
    "Worktree already has many inherited unstaged changes outside this report artifact.",
    "Live provider tests may remain flaky or unavailable without credentials/network even after deterministic blockers close.",
    "Full acceptance definition may need a decision: accepted ignored exclusions vs literally zero #[ignore] attributes."
  ],
  "noStagedFiles": true,
  "diffSummary": "Added requested scout report artifact only; no source/test edits.",
  "reviewFindings": [
    "no blockers in this review artifact"
  ],
  "manualNotes": "Runtime output path override was honored instead of writing to the repository .agents/state path."
}
```
