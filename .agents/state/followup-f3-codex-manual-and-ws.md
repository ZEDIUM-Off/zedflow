<!-- migration-document-status: SUPERSEDED / HISTORICAL -->
> [!CAUTION]
> **Migration status: SUPERSEDED / HISTORICAL.** Do not use this file as the current execution tracker. See `.agents/state/zedflow-ai-agent-pi-fidelity-current-status.md`.

## Review
- Correct: `stream_live` now resolves Codex credentials from explicit options or Pi auth storage, builds the request, and starts a worker thread for the WebSocket/SSE chain (`crates/zedflow-ai/src/api/openai-codex-responses.rs:1219-1269`). SSE live transport is real: it posts JSON to `/codex/responses`, reads SSE `data:` frames, normalizes events, and feeds the shared Responses stream processor (`crates/zedflow-ai/src/api/openai-codex-responses.rs:1344-1368`, `1419-1438`, `1523-1557`).
- Correct: SSE cache-affinity identifiers are wired for the live path. `prompt_cache_key` is derived from `session_id` and clamped (`crates/zedflow-ai/src/api/openai-codex-responses.rs:641-647`, `684-687`), and SSE headers include both `session-id` and `x-client-request-id` (`crates/zedflow-ai/src/api/openai-codex-responses.rs:965-983`). The live cache-affinity test is capability-gated and forces `Transport::Sse` with a stable session id (`crates/zedflow-ai/tests/openai-codex-cache-affinity-e2e.rs:37-48`, `63-74`).
- Correct: responseId propagation is present for Codex SSE. The shared Responses processor records `ResponseCreated` and terminal response ids (`crates/zedflow-ai/src/api/openai-responses-shared.rs:1254-1258`, `1580-1588`), and Codex canonicalization copies `message.response_id` into the public assistant message (`crates/zedflow-ai/src/api/openai-codex-responses.rs:2038-2054`). The Codex responseId live test is capability-gated and uses SSE (`crates/zedflow-ai/tests/responseid.rs:198-236`, `379-406`).
- Blocker: Direct WebSocket success remains unverified and likely still blocked. R11 recorded backend HTTP 405 before output, then SSE fallback. Current code constructs `OpenAI-Beta: responses_websockets=2026-02-06` (`crates/zedflow-ai/src/api/openai-codex-responses.rs:986-1003`) but then explicitly skips any `openai-beta` header when building the upgrade request (`crates/zedflow-ai/src/api/openai-codex-responses.rs:1655-1665`). The live WebSocket probe only asserts the final message is not an error and closes a no-op cache handle; it does not assert status 101, WebSocket stats, or no fallback (`crates/zedflow-ai/tests/codex-websocket-cached-probe.rs:326-382`).
- Blocker: Cached WebSocket reuse is not implemented in the live code. `close_openai_codex_websocket_sessions` documents that no live sockets are created and is a no-op (`crates/zedflow-ai/src/api/openai-codex-responses.rs:1132-1136`). Each WebSocket attempt creates a new reqwest client, performs one upgrade, sends one request body, reads until terminal, and drops the upgraded stream (`crates/zedflow-ai/src/api/openai-codex-responses.rs:1637-1756`). Real stats only increment `connections_created`; they never increment `connections_reused`, `delta_requests`, or `last_previous_response_id` (`crates/zedflow-ai/src/api/openai-codex-responses.rs:1797-1817`). The reuse/delta tests are deterministic helper simulations, not assertions against the live implementation (`crates/zedflow-ai/tests/openai-codex-stream.rs:269-323`, `623-650`).
- Blocker: WebSocket cached continuation/responseId affinity is only simulated. The implementation’s cached WebSocket body only sets `prompt_cache_key` for Auto/WebSocketCached (`crates/zedflow-ai/src/api/openai-codex-responses.rs:1775-1795`); it does not store a prior response id or build `previous_response_id` continuation requests. The `previous_response_id = "resp_1"` second turn is injected by the test helper (`crates/zedflow-ai/tests/openai-codex-stream.rs:317-323`), not produced by production code.
- Blocker: Codex zstd request compression remains intentionally ignored and failing. There are no zstd/content-encoding references in Codex source or `crates/zedflow-ai/Cargo.toml`; the capture helper hard-codes `body_was_zstd: false` (`crates/zedflow-ai/tests/openai-codex-stream.rs:225-231`), and the ignored test expects `content-encoding: zstd` and `body_was_zstd` true (`crates/zedflow-ai/tests/openai-codex-stream.rs:679-700`). Running that ignored test fails at line 684 with `left: None`, `right: Some("zstd")`.
- Note: Requested `/home/zedium/workspaces/zedflow/plan.md` and `/home/zedium/workspaces/zedflow/progress.md` are absent; review used the supplied R11/R14 state files instead. R11 already reported HTTP 405 WebSocket upgrade plus SSE fallback success, and R14 still lists Codex zstd compression as an unresolved ignored residual (`.agents/state/subagent-r11-output.md`, `.agents/state/zedflow-ai-pi-ai-runtime-drift-final-report.md`).

## Manual/browser validation checklist for Codex login + connection
1. Login only through the normal ChatGPT/Codex browser flow. Do not paste or screenshot OAuth tokens, `Authorization`, cookies, or `~/.pi/agent/auth.json` values.
2. Confirm local credential presence without values: inspect only provider names/types, e.g. a tiny script that prints whether `openai-codex` exists and whether its value is non-empty, never the value itself.
3. In browser DevTools Network, filter for `codex/responses` while starting one Codex request. Record only: endpoint path, status code, transport type, and header names present. Redact all header values.
4. Expected direct WebSocket success signal: a request to the Codex responses endpoint upgrades with HTTP `101` / WebSocket frames and does not fall back to SSE. If you see `405` or a normal `200 text/event-stream`, direct WebSocket is still not proven.
5. Expected SSE fallback success signal: final assistant response completes with no error, and the network entry is `text/event-stream`; this validates connection/login but not WebSocket success.
6. For cache affinity, run the existing live SSE test after login: `CARGO_TARGET_DIR=/tmp/zedflow-target TMPDIR=/tmp/zedflow-tmp cargo test -p zedflow-ai --test openai-codex-cache-affinity-e2e -- --nocapture`. Keep output, but redact any accidental credential values.
7. For responseId, run only the Codex responseId test: `CARGO_TARGET_DIR=/tmp/zedflow-target TMPDIR=/tmp/zedflow-tmp cargo test -p zedflow-ai --test responseid openai_codex_provider_exposes_response_id -- --nocapture`. Passing means the SSE response id path works.
8. For WebSocket cached, run the existing probe, then verify externally via DevTools/logs whether it used `101` WebSocket and reused the same socket. The current test passing alone is insufficient because it does not assert no fallback or reuse.

## Acceptance notes
```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Review-only task completed without source/test edits; report written only to the authoritative artifact output path."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "Findings cite concrete source/test lines and command results for Codex SSE, WebSocket, cache affinity, responseId, and zstd compression."
    }
  ],
  "changedFiles": [
    ".pi-subagents/artifacts/outputs/cba6fbab-94bb-4840-acac-db851d1294c9/.agents/state/followup-f3-codex-manual-and-ws.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "ls -l plan.md progress.md 2>&1 || true; git diff --cached --name-only; git status --short | sed -n '1,80p'",
      "result": "passed",
      "summary": "plan.md/progress.md absent; no staged files; worktree has inherited unstaged changes."
    },
    {
      "command": "grep -R -n \"zstd\\|content-encoding\" crates/zedflow-ai/Cargo.toml crates/zedflow-ai/src crates/zedflow-ai/tests/openai-codex-stream.rs || true",
      "result": "passed",
      "summary": "Only Codex zstd references are in the ignored test/capture fields; none in source or Cargo.toml."
    },
    {
      "command": "CARGO_TARGET_DIR=/tmp/zedflow-target TMPDIR=/tmp/zedflow-tmp cargo test -p zedflow-ai --test openai-codex-stream --test codex-websocket-cached-probe --test openai-codex-cache-affinity-e2e --test responseid",
      "result": "passed",
      "summary": "31 passed, 9 ignored across 4 Codex/responseId suites."
    },
    {
      "command": "CARGO_TARGET_DIR=/tmp/zedflow-target TMPDIR=/tmp/zedflow-tmp cargo test -p zedflow-ai --test openai-codex-stream zstd_compresses_sse_request_bodies -- --ignored",
      "result": "failed as expected",
      "summary": "Ignored zstd test failed: expected content-encoding zstd, got None."
    }
  ],
  "validationOutput": [
    "Codex/responseId targeted tests: cargo test: 31 passed, 9 ignored (4 suites, 5.43s).",
    "Ignored zstd test: FAILED at crates/zedflow-ai/tests/openai-codex-stream.rs:684 with left None, right Some(\"zstd\").",
    "No staged files reported by git diff --cached --name-only."
  ],
  "residualRisks": [
    "Direct Codex WebSocket HTTP 101 success is still unproven; current upgrade omits the constructed OpenAI-Beta websocket header.",
    "Cached WebSocket reuse and previous_response_id delta continuation are not implemented in production code.",
    "Codex zstd request compression remains ignored/failing until a dependency or byte-body seam is approved.",
    "Live provider behavior may vary by account/session; manual browser validation should not expose secrets."
  ],
  "noStagedFiles": true,
  "diffSummary": "No repository source/test diff from this review; only the required review artifact was written.",
  "reviewFindings": [
    "blocker: crates/zedflow-ai/src/api/openai-codex-responses.rs:1655-1665 - WebSocket upgrade skips the OpenAI-Beta header that build_websocket_headers constructs.",
    "blocker: crates/zedflow-ai/src/api/openai-codex-responses.rs:1132-1136 and 1637-1756 - cached WebSocket sessions are not retained or reused.",
    "blocker: crates/zedflow-ai/src/api/openai-codex-responses.rs:1775-1817 - cached mode records only full-context requests and prompt_cache_key; no previous_response_id continuation is produced.",
    "blocker: crates/zedflow-ai/tests/openai-codex-stream.rs:679-700 - zstd compression test remains ignored and fails when run."
  ],
  "manualNotes": "plan.md and progress.md were requested but are absent in the repo root; R11/R14 state files were available and reviewed."
}
```
