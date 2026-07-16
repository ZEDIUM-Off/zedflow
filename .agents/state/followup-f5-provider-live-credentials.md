<!-- migration-document-status: SUPERSEDED / HISTORICAL -->
> [!CAUTION]
> **Migration status: SUPERSEDED / HISTORICAL.** Do not use this file as the current execution tracker. See `.agents/state/zedflow-ai-agent-pi-fidelity-current-status.md`.

# Code Context

## Files Retrieved
1. `.agents/state/zedflow-ai-pi-ai-runtime-drift-final-report.md` (lines 1-190) - source report for final live run status, ignored residuals, and provider blockers.
2. `crates/zedflow-ai/tests/common/live_credentials.rs` (lines 1-190) - shared live credential detector and Pi auth JSON fallback.
3. `crates/zedflow-ai/tests/images.rs` (lines 1-184) - OpenRouter image live suite and exact credential use.
4. `crates/zedflow-ai/tests/responseid.rs` (lines 250-405) - responseId live provider matrix for OpenAI direct, Codex, Google, Azure, Anthropic, Mistral, GitHub Copilot.
5. `crates/zedflow-ai/tests/bedrock-models.rs` (lines 1-65) - Bedrock live credential gate and extensive-test env gate.
6. `crates/zedflow-ai/tests/bedrock-utils.rs` (lines 1-46) - Bedrock credential detection helper.
7. `crates/zedflow-ai/tests/azure-utils.rs` (lines 1-92) - Azure OpenAI required env and deployment map helper.
8. `crates/zedflow-ai/tests/stream.rs` (lines 1-300, 430-468) - ignored full provider/local Ollama E2E provider matrix and implementation blockers.

## Key Code

Credential sources are centralized in `crates/zedflow-ai/tests/common/live_credentials.rs`:

```rust
pub fn default_pi_auth_path() -> PathBuf {
    ...join(".pi").join("agent").join("auth.json")
}

pub fn api_key_env_vars(provider: &str) -> &'static [&'static str] {
    match provider {
        "github-copilot" => &["COPILOT_GITHUB_TOKEN"],
        "anthropic" => &["ANTHROPIC_OAUTH_TOKEN", "ANTHROPIC_API_KEY"],
        "openai" => &["OPENAI_API_KEY"],
        "azure-openai-responses" => &["AZURE_OPENAI_API_KEY"],
        "google" => &["GEMINI_API_KEY"],
        "google-vertex" => &["GOOGLE_CLOUD_API_KEY"],
        "openrouter" => &["OPENROUTER_API_KEY"],
        "mistral" => &["MISTRAL_API_KEY"],
        "openai-codex" => &[],
        _ => &[],
    }
}
```

Pi auth JSON supports `{type:"api_key", key:...}` and `{type:"oauth", access:...}`. Safe probe result in this workspace: `/home/zedium/.pi/agent/auth.json` exists; `openrouter` present as `api_key`; `openai-codex` present as `oauth`; `openai`, `anthropic`, `github-copilot`, `google`, `google-vertex`, `mistral`, and `azure-openai-responses` absent. No secret values inspected or printed.

## Provider live credential/capability matrix

| Provider/suite | Required env vars / auth files | Exact cargo test command(s) | Manual/browser steps | Blocker status |
|---|---|---|---|---|
| OpenRouter images | `OPENROUTER_API_KEY` or `/home/zedium/.pi/agent/auth.json` entry `openrouter` (`api_key`). | `cargo test -p zedflow-ai --test images -- --nocapture` | None. | Credentials available via Pi auth JSON; implementation works per final report (`3 passed`). |
| OpenRouter cache-write repro / OpenRouter text paths | Same as OpenRouter. Some ignored matrix tests mention env-only `OPENROUTER_API_KEY`, but shared live helper can use Pi auth JSON where wired. | `cargo test -p zedflow-ai --test openrouter-cache-write-repro -- --nocapture`; ignored provider matrix: `cargo test -p zedflow-ai --test google-thinking-disable openrouter -- --ignored --nocapture`; full stream matrix: `cargo test -p zedflow-ai --test stream runs_generate_e2e_stream_provider_matrix -- --ignored --nocapture`. | None. | Mixed: credentials available; cache-write live request is skipped by in-test implementation blocker; full stream matrix blocked by compat/provider dispatch and provider streaming transport. |
| OpenAI direct (Responses/Completions) | `OPENAI_API_KEY`; no `openai` Pi auth entry detected. | Capability-gated already included by `cargo test -p zedflow-ai --test responseid -- --nocapture`; ignored direct suites include `cargo test -p zedflow-ai --test xhigh -- --ignored --nocapture`, `cargo test -p zedflow-ai --test google-thinking-disable -- --ignored --nocapture`, `cargo test -p zedflow-ai --test openai-responses-reasoning-replay-e2e -- --ignored --nocapture`, `cargo test -p zedflow-ai --test openai-responses-cache-affinity-e2e -- --ignored --nocapture`. | None beyond obtaining API key. | Credentials are the first blocker for responseId/xhigh direct live tests; some replay/cache-affinity tests also have explicit `see BLOCKER` implementation notes. |
| Codex / OpenAI Codex | `/home/zedium/.pi/agent/auth.json` entry `openai-codex` (`oauth`). `api_key_env_vars("openai-codex")` is empty by design. | `cargo test -p zedflow-ai --test openai-codex-stream --test responseid --test openai-codex-cache-affinity-e2e --test codex-websocket-cached-probe -- --nocapture`; ignored residual: `cargo test -p zedflow-ai --test openai-codex-stream -- --ignored --nocapture`; `cargo test -p zedflow-ai --test xhigh -- --ignored --nocapture`. | If missing/expired, refresh Pi/OpenAI Codex OAuth in Pi so auth JSON has a valid `openai-codex` OAuth token. Browser likely needed for OAuth refresh. | Credentials available and implemented suites passed (`31 passed, 9 ignored`). Remaining blocker is implementation/approval for zstd request body compression, not credentials. |
| Amazon Bedrock | `AWS_PROFILE` or both `AWS_ACCESS_KEY_ID` + `AWS_SECRET_ACCESS_KEY` or `AWS_BEARER_TOKEN_BEDROCK`; extensive model sweep also requires `BEDROCK_EXTENSIVE_MODEL_TEST`; region may be needed by AWS SDK (`AWS_REGION` appears in endpoint tests). No Bedrock Pi auth entry detected by final report. | `cargo test -p zedflow-ai --test bedrock-models -- --ignored --nocapture`; `cargo test -p zedflow-ai --test bedrock-thinking-payload -- --ignored --nocapture`; `cargo test -p zedflow-ai --test interleaved-thinking -- --ignored --nocapture`; stale regression: `cargo test -p zedflow-ai --test provider-error-body-regression -- --ignored --nocapture`. | AWS credential/profile setup; no browser unless SSO profile requires it. | Credentials/network block the live model sweep, but important implementation blockers remain: completeSimple/provider streaming parity for interleaved thinking and stale Bedrock/genai error-body regression placeholder. |
| Anthropic | `ANTHROPIC_API_KEY` or `ANTHROPIC_OAUTH_TOKEN`; `/home/zedium/.pi/agent/auth.json` entry `anthropic` also supported by shared helper but absent here. | `cargo test -p zedflow-ai --test responseid anthropic -- --ignored --nocapture`; `cargo test -p zedflow-ai --test anthropic-thinking-disable -- --ignored --nocapture`; `cargo test -p zedflow-ai --test google-thinking-disable anthropic -- --ignored --nocapture`; `cargo test -p zedflow-ai --test anthropic-eager-tool-input-e2e -- --ignored --nocapture`; `cargo test -p zedflow-ai --test anthropic-opus-4-8-smoke -- --ignored --nocapture`; `cargo test -p zedflow-ai --test scratch -- --ignored --nocapture`; full stream: `cargo test -p zedflow-ai --test stream runs_generate_e2e_stream_provider_matrix -- --ignored --nocapture`. | API key: none. OAuth token: likely browser/device auth flow if using Pi OAuth. | Both: credentials absent, and several tests cite unported Anthropic SDK/HTTP-SSE, `streamSimple`/`onPayload`, thinking options, and compat catalog/provider dispatch blockers. |
| Google Gemini API | `GEMINI_API_KEY`; `/home/zedium/.pi/agent/auth.json` entry `google` supported by shared helper but absent here. | `cargo test -p zedflow-ai --test responseid google -- --ignored --nocapture`; `cargo test -p zedflow-ai --test google-thinking-disable -- --ignored --nocapture`; `cargo test -p zedflow-ai --test empty -- --ignored --nocapture`; full stream: `cargo test -p zedflow-ai --test stream runs_generate_e2e_stream_provider_matrix -- --ignored --nocapture`. | None beyond API key. | Credentials are absent; full stream matrix additionally blocked by compat/provider dispatch and streaming transports. |
| Google Vertex | API key path: `GOOGLE_CLOUD_API_KEY`. ADC path: `GOOGLE_CLOUD_PROJECT` + `GOOGLE_CLOUD_LOCATION` plus working Google ADC credentials. Shared helper only lists `GOOGLE_CLOUD_API_KEY` for `google-vertex`; responseId ADC test embeds project/location options. | `cargo test -p zedflow-ai --test responseid google_vertex -- --ignored --nocapture`; `cargo test -p zedflow-ai --test google-thinking-disable vertex -- --ignored --nocapture`; full stream: `cargo test -p zedflow-ai --test stream runs_generate_e2e_stream_provider_matrix -- --ignored --nocapture`. | For ADC, run Google auth outside tests (for example `gcloud auth application-default login`) and set project/location. Browser likely for ADC login. | Credentials/auth setup absent; stream matrix also implementation-blocked. |
| Azure OpenAI | `AZURE_OPENAI_API_KEY` plus either `AZURE_OPENAI_BASE_URL` or `AZURE_OPENAI_RESOURCE_NAME`; deployment name via `AZURE_OPENAI_DEPLOYMENT_NAME` or `AZURE_OPENAI_DEPLOYMENT_NAME_MAP` depending suite/model. Shared helper names only `AZURE_OPENAI_API_KEY`, but `azure-utils.rs` requires endpoint/resource too. | `cargo test -p zedflow-ai --test responseid azure -- --ignored --nocapture`; `cargo test -p zedflow-ai --test cross-provider-handoff -- --ignored --nocapture`; full stream: `cargo test -p zedflow-ai --test stream runs_generate_e2e_stream_provider_matrix -- --ignored --nocapture`. | Provision Azure OpenAI deployment and map model IDs to deployment names; no browser except cloud portal provisioning. | Credentials/config absent; cross-provider/stream paths also cite compat catalog/provider dispatch and completeSimple/provider call blockers. |
| Mistral | `MISTRAL_API_KEY`; `/home/zedium/.pi/agent/auth.json` entry `mistral` supported by shared helper but absent here. | `cargo test -p zedflow-ai --test responseid mistral -- --ignored --nocapture`; `cargo test -p zedflow-ai --test empty -- --ignored --nocapture`; full stream: `cargo test -p zedflow-ai --test stream runs_generate_e2e_stream_provider_matrix -- --ignored --nocapture`. | None beyond API key. | Credentials absent; full stream matrix also implementation-blocked. |
| GitHub Copilot | `COPILOT_GITHUB_TOKEN` or `/home/zedium/.pi/agent/auth.json` entry `github-copilot` (OAuth/API key supported by generic parser if present). | `cargo test -p zedflow-ai --test responseid github_copilot -- --ignored --nocapture`; `cargo test -p zedflow-ai --test github-copilot-anthropic -- --ignored --nocapture`; full stream: `cargo test -p zedflow-ai --test stream runs_generate_e2e_stream_provider_matrix -- --ignored --nocapture`. | If token absent/expired, resolve GitHub Copilot OAuth token; likely browser/device code login. | Both: Copilot credentials absent; Anthropic-path tests also blocked because Anthropic Messages stream/client construction is not ported. |
| local / Ollama | Running local Ollama server with requested model `gpt-oss:20b` available; likely `OLLAMA_HOST` only if not default localhost. No shared env credential gate. | `cargo test -p zedflow-ai --test stream runs_generate_e2e_stream_provider_matrix -- --ignored --nocapture`; `cargo test -p zedflow-ai --test context-overflow -- --ignored --nocapture`. | Start Ollama, pull model (`ollama pull gpt-oss:20b`) or adjust local model only if tests are updated. | Both: local server/model absent by default; full stream suite explicitly blocked by compat/provider dispatch and provider streaming transports. |

## Architecture

Live tests follow two patterns:

1. Capability-gated non-ignored tests call `common::live_credentials` and return early with a redacted skip message when credentials are missing. OpenRouter images and Codex suites use this path and can consume Pi auth JSON.
2. Many remaining provider parity tests are still `#[ignore]` with metadata. Running them requires both `--ignored` and the provider credentials/config. For several suites, credentials alone are insufficient because the test body or ignore reason names unported compat/provider dispatch, provider streaming, request capture, or error-body seams.

The final drift report confirms deterministic gates pass and that only OpenRouter plus OpenAI Codex live capabilities were detected and exercised. It also classifies residual ignored tests: most are live/capability gates, while Codex zstd compression and Bedrock/genai provider error-body regression are implementation blockers independent of credentials.

## Start Here

Open `crates/zedflow-ai/tests/common/live_credentials.rs` first. It is the only shared credential/capability map and defines which providers can use env vars versus Pi auth JSON. Then open `.agents/state/zedflow-ai-pi-ai-runtime-drift-final-report.md` for the current pass/fail/live-run state.

## Supervisor coordination

No supervisor decision needed. Scope was read-only scouting plus artifact writing. No repository files were edited.

## Commands run

- `grep`/`read` inspections over `.agents/state/zedflow-ai-pi-ai-runtime-drift-final-report.md` and `crates/zedflow-ai/tests`.
- Safe auth presence probe, no values printed:
  - `/home/zedium/.pi/agent/auth.json` exists.
  - present: `openrouter` (`api_key`), `openai-codex` (`oauth`).
  - absent: `openai`, `anthropic`, `github-copilot`, `google`, `google-vertex`, `mistral`, `azure-openai-responses`.
- `git status --short && git diff --cached --quiet; echo staged:$?` returned many inherited unstaged files and `staged:0`.

## Residual risks / open questions

- Some ignored tests still mention env-only credentials even though the shared helper supports Pi auth JSON; verify the specific test path before assuming auth JSON works.
- Full stream matrix is broad and implementation-blocked; do not treat it as a pure credential task.
- Provider live tests may spend money and can be flaky/provider-dependent.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Built the requested provider live credential/capability matrix from the final drift report and remaining tests; no repo files edited."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "Report includes file/line context, exact cargo commands, credential/auth requirements, manual steps, blockers, commands run, and staged-file check."
    }
  ],
  "changedFiles": [
    "/home/zedium/workspaces/zedflow/.pi-subagents/artifacts/outputs/cba6fbab-94bb-4840-acac-db851d1294c9/.agents/state/followup-f5-provider-live-credentials.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "read .agents/state/zedflow-ai-pi-ai-runtime-drift-final-report.md and selected crates/zedflow-ai/tests files",
      "result": "passed",
      "summary": "Collected credential gates, ignored live suites, and blocker metadata."
    },
    {
      "command": "python3 safe auth presence probe for ~/.pi/agent/auth.json",
      "result": "passed",
      "summary": "Printed provider presence/type only; no secret values."
    },
    {
      "command": "git status --short && git diff --cached --quiet; echo staged:$?",
      "result": "passed",
      "summary": "Inherited unstaged changes exist; no staged files (staged:0)."
    }
  ],
  "validationOutput": [
    "Auth presence: openrouter api_key present; openai-codex oauth present; target providers otherwise absent in Pi auth JSON.",
    "No repository files edited; artifact written to runtime override path.",
    "No staged files detected."
  ],
  "residualRisks": [
    "Live provider tests can spend money and be flaky.",
    "Full stream provider matrix remains implementation-blocked, not just credential-blocked.",
    "Some ignored tests may require env vars directly rather than shared Pi auth JSON helper."
  ],
  "noStagedFiles": true,
  "diffSummary": "Added scouting report artifact only at the required output path; repository worktree left unchanged.",
  "reviewFindings": [
    "no blockers"
  ],
  "manualNotes": "The user-requested in-repo output path conflicts with runtime override; wrote only to the authoritative override path."
}
```
