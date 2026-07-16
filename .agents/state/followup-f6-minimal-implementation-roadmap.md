<!-- migration-document-status: SUPERSEDED / HISTORICAL -->
> [!CAUTION]
> **Migration status: SUPERSEDED / HISTORICAL.** Do not use this file as the current execution tracker. See `.agents/state/zedflow-ai-agent-pi-fidelity-current-status.md`.

# Implementation Plan

## Goal
Bring `zedflow-ai` runtime-drift acceptance to a reviewed, explicit state with the smallest diffs: prove/remove stale ignores, close the two non-live blockers, and document any remaining product-accepted residuals.

## Tasks
1. **Baseline the ignored-test list before editing**: Re-run the ignore audit and targeted ignored tests so only proved-stale ignores are removed.
   - File: `crates/zedflow-ai/tests/**/*.rs`
   - Changes: No source change in this step; collect current `#[ignore]` lines and run likely-stale ignored tests one at a time.
   - Suggested commands:
     - `grep -R -n '#\[ignore\|ignore =' crates/zedflow-ai/src crates/zedflow-ai/tests`
     - `cargo test -p zedflow-ai --test openai-codex-stream zstd_compresses_sse_request_bodies -- --ignored --exact`
     - `cargo test -p zedflow-ai --test provider-error-body-regression bedrock_body_blind_surfaces_the_gateway_body_instead_of_unknown_unknown_error -- --ignored --exact`
     - Spot-check deterministic-looking ignores such as `crates/zedflow-ai/tests/xhigh.rs` before keeping them labeled live.
   - Acceptance: The executor has a before count, exact stale-ignore candidates, and no bulk ignore deletion.

2. **Implement Codex SSE zstd only if dependency policy allows one small dependency**: This is worth doing if live Codex parity is in scope; do not hand-roll zstd.
   - File: `crates/zedflow-ai/Cargo.toml`
   - Changes: Add `zstd = "0.13"` to `zedflow-ai` dependencies if approved by normal dependency review.
   - File: `crates/zedflow-ai/src/api/openai-codex-responses.rs`
   - Changes:
     - Add `REQUEST_COMPRESSION_ZSTD_LEVEL: i32 = 3` near Pi/Codex constants.
     - Add a tiny helper that serializes request JSON, attempts `zstd` compression at level 3, and falls back to raw JSON bytes only on compression error.
     - Extend `OpenAICodexResponsesRequest` with an observable SSE byte-body seam, e.g. `sse_body: Vec<u8>` plus `sse_body_was_zstd: bool` (`#[serde(skip)]` is fine if serialization noise is a concern).
     - In `build_request`, compute the SSE body once and set `content-encoding: zstd` only when compression succeeded.
     - In `execute_codex_sse_live`, send `request.sse_body.clone()` instead of re-serializing `request.body`.
     - Leave WebSocket frames uncompressed; Pi compresses only the SSE `fetch` body.
   - File: `crates/zedflow-ai/tests/openai-codex-stream.rs`
   - Changes:
     - Update `capture_sse_request` to copy `request.sse_body_was_zstd`.
     - Replace the current placeholder zstd test body setup with an explicit large-context request (`"compress me ".repeat(400)`) and decode `request.sse_body` with `zstd` to assert the JSON round-trips.
     - Keep the small-body assertion if Pi parity requires always-compress-when-available; otherwise document and assert the chosen threshold. Pi currently attempts compression for all SSE bodies.
     - Remove `#[ignore = "Codex request body compression..."]` only after the exact test passes.
   - Acceptance: `cargo test -p zedflow-ai --test openai-codex-stream zstd_compresses_sse_request_bodies -- --exact` passes unignored, and a live Codex SSE smoke still connects when credentials exist.

3. **Close the Bedrock provider error-body regression with a test-first minimal diff**: Current Bedrock/error-body helpers appear to already preserve HTTP body text; prove it and remove the stale ignore.
   - File: `crates/zedflow-ai/tests/provider-error-body-regression.rs`
   - Changes:
     - Replace the empty ignored Bedrock placeholder with a deterministic assertion against `bedrock::format_bedrock_error(...)` using `SdkErrorShape { message: "UnknownError".into(), metadata_http_status_code: Some(403.0), response_status_code: Some(403.0), response_body: Some(json!("{\"message\":\"blocked by gateway WAF\"}")), ..Default::default() }`.
     - Assert the output contains `403` and `blocked by gateway WAF`, and is not the body-blind `UnknownError: UnknownError` shape.
     - Remove `#[ignore = "Bedrock provider error-body parity belongs to P3B"]` after the exact test passes.
   - File: `crates/zedflow-ai/src/api/bedrock-converse-stream.rs`
   - Changes: Prefer no change. If the test fails, adjust only `format_bedrock_error`/`format_bedrock_http_error` to pass the existing `SdkErrorShape.response_body` through `utils::error_body`.
   - Acceptance: `cargo test -p zedflow-ai --test provider-error-body-regression bedrock_body_blind_surfaces_the_gateway_body_instead_of_unknown_unknown_error -- --exact` passes unignored.

4. **Remove only proved-stale ignores**: After Tasks 2-3 and any baseline spot-checks, delete ignore attributes whose tests now pass without provider credentials.
   - File: `crates/zedflow-ai/tests/openai-codex-stream.rs`
   - Changes: Remove the zstd ignore.
   - File: `crates/zedflow-ai/tests/provider-error-body-regression.rs`
   - Changes: Remove the Bedrock error-body ignore.
   - File: `crates/zedflow-ai/tests/xhigh.rs` and any other spot-checked deterministic ignore
   - Changes: Remove ignores only if the exact tests pass locally without live credentials; otherwise rewrite the ignore reason to match the real blocker.
   - Acceptance: Ignore count drops by the number of proved-stale ignores; no live/capability gate is accidentally made default.

5. **Make product residual acceptance explicit**: Full acceptance cannot be claimed from hidden ignores; remaining ignores need either implementation or signed-off residual status.
   - File: `.agents/state/zedflow-ai-pi-ai-runtime-drift-final-report.md`
   - Changes:
     - Refresh validation numbers after Tasks 1-4.
     - Split remaining ignores into: live credential/manual gates, JS-only/nonportable, upstream-skipped, and product-accepted residuals.
     - For product residuals, record the decision, owner, and re-open trigger. Likely residuals include non-current providers/transports (Anthropic/Google/Azure/Mistral/GitHub Copilot/local LLM/Xiaomi) unless product requires full Pi provider parity now.
     - Do not mark global acceptance satisfied unless the product owner accepts those residuals; otherwise leave verdict as not fully satisfied.
   - Acceptance: A reviewer can tell whether every remaining ignore is allowed by policy, not just unimplemented.

6. **Run deterministic validation gates**: Keep the gate boring and narrow first, then full package.
   - File: none
   - Changes: No code changes.
   - Commands:
     - `cargo fmt --all --check`
     - `cargo check -p zedflow-ai --all-targets`
     - `cargo test -p zedflow-ai --test openai-codex-stream --test provider-error-body-regression`
     - `cargo test -p zedflow-ai --all-targets`
     - `grep -R "PORT PLACEHOLDER" crates/zedflow-ai/src crates/zedflow-ai/tests`
     - `grep -R -n '#\[ignore\|ignore =' crates/zedflow-ai/src crates/zedflow-ai/tests`
   - Acceptance: All deterministic gates pass; ignore audit matches the refreshed report.

7. **Run live/manual validation only where credentials exist**: Do not block deterministic acceptance on unavailable providers unless product requires them.
   - File: none
   - Changes: No code changes.
   - Codex manual/browser help:
     - User should complete/refresh OpenAI Codex OAuth in the browser/ChatGPT flow used by Pi auth JSON.
     - Run the existing capability-gated Codex suite without printing tokens: `cargo test -p zedflow-ai --test openai-codex-stream --test responseid --test openai-codex-cache-affinity-e2e --test codex-websocket-cached-probe -- --nocapture`.
     - Add one manual live SSE check after zstd lands: confirm Codex returns a normal response with `content-encoding: zstd` enabled. If the backend rejects zstd, revert to product-accepted residual rather than building fallback complexity beyond the existing raw-body fallback.
   - Bedrock manual help:
     - If the user can provide `AWS_BEARER_TOKEN_BEDROCK`, AWS key pair, or `AWS_PROFILE`, run the Bedrock live suite; otherwise keep Bedrock live tests capability-gated.
   - Acceptance: Live results are recorded as passed/skipped-by-capability with no secrets in logs.

## Suggested Batches
- **Batch A — stale-ignore cleanup**: Task 1 plus any exact-test ignore removals that need no implementation.
- **Batch B — Codex zstd**: Task 2 plus Codex targeted tests and one manual Codex connection check.
- **Batch C — Bedrock error-body**: Task 3 plus the provider-error regression test.
- **Batch D — acceptance/report gate**: Tasks 4-7, including product residual decision and final deterministic gates.

## Files to Modify
- `crates/zedflow-ai/Cargo.toml` - add `zstd` only if Codex zstd is accepted as worth the dependency.
- `crates/zedflow-ai/src/api/openai-codex-responses.rs` - prepare/send zstd SSE request bytes and expose a minimal test seam.
- `crates/zedflow-ai/tests/openai-codex-stream.rs` - unignore and assert Codex zstd request-body behavior.
- `crates/zedflow-ai/tests/provider-error-body-regression.rs` - unignore and assert Bedrock error body preservation.
- `crates/zedflow-ai/src/api/bedrock-converse-stream.rs` - only if the Bedrock regression test proves the current formatter still drops the body.
- `.agents/state/zedflow-ai-pi-ai-runtime-drift-final-report.md` - refresh counts/verdict and document accepted residuals.
- `crates/zedflow-ai/tests/xhigh.rs` or other ignored tests - only if exact baseline tests prove the ignore is stale.

## New Files
- None. Prefer updating the existing final report over creating another acceptance ledger.

## Dependencies
- Task 4 depends on Tasks 1-3.
- Task 5 depends on the refreshed ignore count from Task 4 and a product decision on residual scope.
- Task 6 depends on implementation tasks being complete.
- Task 7 depends on user/provider credentials; Codex OAuth browser refresh is user-assisted.

## Risks
- Lowercase `/home/zedium/workspaces/zedflow/context.md` was absent; planning used `CONTEXT.md` plus the final drift report.
- F1-F5 follow-up outputs were not present/readable at the requested artifact location during planning, so this roadmap relies on the checked-in final report and direct code inspection.
- Adding `zstd` is a dependency-policy decision. If rejected, the smallest path is to leave the zstd test ignored and record it as a product-accepted residual, not to implement a custom compressor.
- Codex zstd may fail only against live backend behavior despite deterministic pass; user/browser OAuth testing is the useful validation point.
- “Full acceptance” is ambiguous unless product agrees that live unavailable providers and JS-only tests can remain accepted residuals.
- The repository had inherited dirty work in prior reports; executor should check `git diff --cached --quiet` before handoff.
