<!-- migration-document-status: COMPLETED ARTIFACT -->
> [!NOTE]
> **Migration status: COMPLETED ARTIFACT.** Raw historical evidence only; do not use it as current progress. See `.agents/state/zedflow-ai-agent-pi-fidelity-current-status.md`.

# R11 Codex SSE/WS live report

Implemented R11 only.

Changed files:
- `Cargo.lock`
- `crates/zedflow-ai/Cargo.toml`
- `crates/zedflow-ai/src/api/openai-codex-responses.rs`
- `crates/zedflow-ai/tests/openai-codex-cache-affinity-e2e.rs`
- `crates/zedflow-ai/tests/codex-websocket-cached-probe.rs`
- `crates/zedflow-ai/tests/responseid.rs`

Transport report:
- Added `stream_live` for OpenAI Codex Responses.
- SSE path performs a real POST to `/codex/responses`, reads SSE `data:` frames, normalizes Codex/OpenAI dotted event names, maps them through the shared Responses stream processor, and emits canonical `AssistantMessageEventStream` events/results with responseId and usage.
- Codex live auth resolves request `api_key` first and falls back to stored `~/.pi/agent/auth.json` `openai-codex` OAuth/API-key credentials without printing values.
- WebSocket path attempts an HTTP upgrade with Codex WebSocket headers, open timeout, idle timeout, client frame masking, JSON frame parsing, connection-limit retry, debug stats, and before-first-event SSE fallback.
- Live websocket-cached probe completed through the fallback path in this environment; the backend returned HTTP 405 to the manual upgrade before output started, then SSE fallback completed successfully.

Redaction confirmation:
- No credential values were printed. Capability checks and reports only disclosed presence/absence and provider names.

Validation commands/results:
- `cargo fmt --all --check` — passed.
- `cargo test -p zedflow-ai --test openai-codex-stream --test responseid --test openai-codex-cache-affinity-e2e --test codex-websocket-cached-probe -- --nocapture` — passed: 31 passed, 9 ignored. Codex credentials were present, so Codex live tests executed.
- `cargo test -p zedflow-ai --lib openai_codex_responses` — passed: 9 passed, 382 filtered.
- `git diff --cached --name-only` — empty; no staged files.

Remaining Codex blockers/risks:
- Direct WebSocket live success was not observed: backend returned HTTP 405 to the manual upgrade attempt, so the intended Pi fallback path handled the request over SSE.
- WebSocket cached continuation currently records request/debug state but does not retain a reusable upgraded socket across calls.
- Requested lowercase `context.md`/`plan.md` and optional P6/P7/port-audit files were not present.
