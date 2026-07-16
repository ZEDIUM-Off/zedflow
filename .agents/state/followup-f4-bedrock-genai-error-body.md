<!-- migration-document-status: SUPERSEDED / HISTORICAL -->
> [!CAUTION]
> **Migration status: SUPERSEDED / HISTORICAL.** Do not use this file as the current execution tracker. See `.agents/state/zedflow-ai-agent-pi-fidelity-current-status.md`.

# Follow-up F4: Bedrock/genai error-body residual

## Review
- Correct: `plan.md` and `progress.md` were absent at repo root, so I used the requested state reports and source/tests instead.
- Correct: R12 replaced the Bedrock placeholder with a direct Bedrock Runtime fallback, not a `genai` path. Evidence: `crates/zedflow-ai/src/api/bedrock-converse-stream.rs:921-943` starts a worker-backed canonical stream, `:983-1027` sends the HTTP request and reads non-2xx bodies, and `:330-342` documents why test-only genai Bedrock parity still requires the fallback.
- Correct: shared error-body normalization already understands AWS/Bedrock SDK-shaped errors. Evidence: `crates/zedflow-ai/src/utils/error-body.rs:21-39` defines `$metadata`/`$response` fields, `:208-240` extracts status/body, and `:332-352` has a unit test for `UnknownError` + `$response.body`.
- Blocker: the remaining ignored regression is stale and vacuous, not a live/capability test. `crates/zedflow-ai/tests/provider-error-body-regression.rs:182-196` is ignored, builds an unused `_send_error`, and asserts nothing. Removing only `#[ignore]` would pass but would not prove the Pi regression.
- Note: the ignore is now removable if the placeholder is replaced with a real assertion. The direct Rust fallback changed the seam from Pi's mocked AWS SDK `send()` to local error formatting, so the minimal useful test is either:
  - an integration assertion in `provider-error-body-regression.rs` calling public `bedrock::format_bedrock_error(Some("UnknownError"), SdkErrorShape { message: "UnknownError", metadata_http_status_code: Some(403.0), response_status_code: Some(403.0), response_body: Some(json!("{\"message\":\"blocked by gateway WAF\"}")), ..Default::default() })` and checking `stop_reason == error` equivalent, `contains("403")`, `contains("blocked by gateway WAF")`, and not `contains("Unknown: UnknownError")`; or
  - better, a small unit test beside the private direct helper for `format_bedrock_http_error(403, "{\"message\":\"blocked by gateway WAF\"}")`.
- Note: for strict Pi parity (`status + body`), one code line should change in `format_bedrock_http_error` at `crates/zedflow-ai/src/api/bedrock-converse-stream.rs:1039-1048`: do not set `SdkErrorShape.message` to the body itself, because `format_provider_error` then treats the body as already carried and omits the status (`utils/error-body.rs:166-180`). Use an opaque message such as `"{status} status code (no body)"` and keep `response_body`; then the direct Bedrock HTTP path formats as `403: {body}`.

## Bedrock tests checked
- Deterministic Bedrock payload/config tests are local-only by design: `bedrock-convert-messages.rs:1-3`, `bedrock-custom-headers.rs:1-5`, `bedrock-endpoint-resolution.rs:1-7`, and `bedrock-utils.rs:1-4`.
- Live Bedrock coverage remains intentionally ignored/capability-gated: `bedrock-thinking-payload.rs:221-236` panics behind an ignored live test, and `bedrock-models.rs:45-50` requires credentials plus `BEDROCK_EXTENSIVE_MODEL_TEST`.

## Safe manual AWS checks for the user
Run with values redacted/kept local:

1. No-network credential presence:
   `for v in AWS_PROFILE AWS_REGION AWS_DEFAULT_REGION AWS_ACCESS_KEY_ID AWS_SECRET_ACCESS_KEY AWS_BEARER_TOKEN_BEDROCK; do [ -n "${!v}" ] && echo "$v=present" || echo "$v=absent"; done`
2. Identity only, no secrets printed: `aws sts get-caller-identity --no-cli-pager`.
3. Region/model catalog, no inference charge: `aws bedrock list-foundation-models --region "$AWS_REGION" --by-provider Anthropic --query 'modelSummaries[].modelId' --output text --no-cli-pager`.
4. Model metadata, no inference charge: `aws bedrock get-foundation-model --region "$AWS_REGION" --model-identifier "$MODEL_ID" --no-cli-pager`.
5. Tiny billable runtime capability check: `aws bedrock-runtime converse --region "$AWS_REGION" --model-id "$MODEL_ID" --messages '[{"role":"user","content":[{"text":"Reply with OK only."}]}]' --inference-config '{"maxTokens":8,"temperature":0}' --no-cli-pager`.
6. Streaming capability check, also tiny/billable: same payload with `aws bedrock-runtime converse-stream ...`. Avoid repo `BEDROCK_EXTENSIVE_MODEL_TEST=1` unless you intentionally want the broad all-model live suite.

## Commands run
- `git status --short` — inspected only; many pre-existing modified/untracked files are present from the inherited workspace.
- `grep -R -n '#\[ignore.*Bedrock provider error-body parity' crates/zedflow-ai/tests crates/zedflow-ai/src || true` — found the single stale ignore at `crates/zedflow-ai/tests/provider-error-body-regression.rs:183`.
- `cargo test -p zedflow-ai --test provider-error-body-regression` — passed: 3 passed, 1 ignored.
- `cargo test -p zedflow-ai --test provider-error-body-regression bedrock_body_blind_surfaces_the_gateway_body_instead_of_unknown_unknown_error -- --ignored` — passed: 1 passed, 3 filtered, proving the current ignored test is vacuous.
- `cargo test -p zedflow-ai --lib bedrock_converse_stream -- --nocapture` — passed: 8 passed, 383 filtered.
- `cargo test -p zedflow-ai --test bedrock-convert-messages --test bedrock-custom-headers --test bedrock-endpoint-resolution --test bedrock-thinking-payload --test bedrock-utils --test bedrock-models` — passed: 38 passed, 2 ignored.
- Redacted AWS env probe — all checked AWS/Bedrock env vars absent in this session.
- `git diff --cached --name-only` — empty.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Review-only task completed without source/test edits; identified the stale Bedrock error-body ignore and the minimal code/test path to remove it."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "Report cites source/test lines and includes commands/results, residual risk, and manual AWS checks."
    }
  ],
  "changedFiles": [
    "/home/zedium/workspaces/zedflow/.pi-subagents/artifacts/outputs/cba6fbab-94bb-4840-acac-db851d1294c9/.agents/state/followup-f4-bedrock-genai-error-body.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "git status --short",
      "result": "passed",
      "summary": "inspected existing workspace state; many pre-existing modified/untracked files present"
    },
    {
      "command": "grep -R -n '#\\[ignore.*Bedrock provider error-body parity' crates/zedflow-ai/tests crates/zedflow-ai/src || true",
      "result": "passed",
      "summary": "single stale ignore found at provider-error-body-regression.rs:183"
    },
    {
      "command": "cargo test -p zedflow-ai --test provider-error-body-regression",
      "result": "passed",
      "summary": "3 passed, 1 ignored"
    },
    {
      "command": "cargo test -p zedflow-ai --test provider-error-body-regression bedrock_body_blind_surfaces_the_gateway_body_instead_of_unknown_unknown_error -- --ignored",
      "result": "passed",
      "summary": "1 passed, 3 filtered; current ignored test has no assertions"
    },
    {
      "command": "cargo test -p zedflow-ai --lib bedrock_converse_stream -- --nocapture",
      "result": "passed",
      "summary": "8 passed, 383 filtered"
    },
    {
      "command": "cargo test -p zedflow-ai --test bedrock-convert-messages --test bedrock-custom-headers --test bedrock-endpoint-resolution --test bedrock-thinking-payload --test bedrock-utils --test bedrock-models",
      "result": "passed",
      "summary": "38 passed, 2 ignored"
    },
    {
      "command": "redacted AWS env presence probe",
      "result": "passed",
      "summary": "AWS_BEARER_TOKEN_BEDROCK, AWS key pair, AWS_PROFILE, AWS_REGION/AWS_DEFAULT_REGION, and BEDROCK_EXTENSIVE_MODEL_TEST absent"
    },
    {
      "command": "git diff --cached --name-only",
      "result": "passed",
      "summary": "empty; no staged files"
    }
  ],
  "validationOutput": [
    "provider-error-body-regression: 3 passed, 1 ignored",
    "ignored Bedrock placeholder when forced: 1 passed, 3 filtered",
    "bedrock_converse_stream lib tests: 8 passed, 383 filtered",
    "Bedrock integration tests: 38 passed, 2 ignored",
    "AWS env capability probe: all checked vars absent"
  ],
  "residualRisks": [
    "Removing the ignore without replacing the placeholder assertion is a false pass.",
    "Current direct HTTP helper appears to surface the body but omit status unless its opaque message is changed before adding a strict status+body test.",
    "No live AWS Bedrock call was run because credentials/capability were absent."
  ],
  "noStagedFiles": true,
  "diffSummary": "No source or test diff; report artifact written only.",
  "reviewFindings": [
    "blocker: crates/zedflow-ai/tests/provider-error-body-regression.rs:182-196 - stale ignored Bedrock test is a no-assertion placeholder; replace it before removing the ignore",
    "note: crates/zedflow-ai/src/api/bedrock-converse-stream.rs:1039-1048 - direct HTTP error helper should use an opaque message if the regression requires status + body"
  ],
  "manualNotes": "plan.md/progress.md were absent at repo root; existing workspace had many pre-existing unstaged changes, but no files were staged."
}
```
