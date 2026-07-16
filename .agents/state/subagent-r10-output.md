<!-- migration-document-status: COMPLETED ARTIFACT -->
> [!NOTE]
> **Migration status: COMPLETED ARTIFACT.** Raw historical evidence only; do not use it as current progress. See `.agents/state/zedflow-ai-agent-pi-fidelity-current-status.md`.

Implemented R10 OpenAI live transport work.

Changed files:
- `crates/zedflow-ai/src/api/openai-completions.rs`
- `crates/zedflow-ai/src/api/openai-responses.rs`
- `crates/zedflow-ai/tests/openai-completions-response-model.rs`
- `crates/zedflow-ai/tests/responseid.rs`

Transports wired:
- OpenAI-compatible Chat Completions: added `stream_live` HTTP POST to `/chat/completions`, SSE `data:` parsing, `on_payload`/`on_response` hooks, provider error body passthrough, and canonical `AssistantMessageEventStream` emission preserving responseId, usage, response model, reasoning, text, and tool-call deltas through the existing chunk parser.
- OpenAI Responses: added `stream_live` HTTP POST to `/responses`, SSE `data:` parsing with OpenAI dotted event-name normalization, `on_payload`/`on_response` hooks, provider error body passthrough, and canonical `AssistantMessageEventStream` emission preserving responseId, usage, reasoning/text/tool-call deltas through the existing Responses shared parser.
- Existing request builder APIs (`stream`, `build_request`) were preserved for deterministic request parity.

Tests added/updated:
- Added deterministic local HTTP/SSE transport test for Chat Completions hooks, responseId, usage, and canonical text result in `openai-completions-response-model.rs`.
- Added deterministic local HTTP/SSE responseId test for OpenAI Responses in `responseid.rs`.
- Reactivated OpenAI Chat Completions and OpenAI Responses responseId tests to capability-skip instead of permanent ignore.

Live execution report:
- `OPENAI_API_KEY`: absent.
- `OPENROUTER_API_KEY`: absent.
- OpenAI responseId live tests were invoked via the targeted test suite and skipped by capability helper.
- No OpenRouter live test executed because credentials were absent.
- No Codex/Bedrock live tests were run.

Validation commands/results:
- `cargo fmt --all --check` — passed.
- `cargo test -p zedflow-ai --test openai-completions-response-model --test openai-completions-tool-choice --test responseid --no-run` — passed.
- `cargo test -p zedflow-ai --test openai-completions-response-model --test openai-completions-tool-choice --test responseid` — passed: 50 passed, 8 ignored.
- `cargo test -p zedflow-ai --lib openai_completions` — passed: 21 passed, 368 filtered.
- `cargo test -p zedflow-ai --lib openai_responses` — passed: 33 passed, 356 filtered.
- Credential check command confirmed OpenAI/OpenRouter API keys absent without printing secret values.
- `git diff --cached --name-only` — empty; no staged files.

Residual risks:
- The legacy builder-returning `stream` functions remain for request parity; live execution is exposed through `stream_live` in each OpenAI API module.
- The worktree contains many pre-existing modified/untracked files from prior R1-R8 waves; this run's intended edits are limited to the four files above.
- Requested lowercase `context.md`/`plan.md` and `.agents/state/port-audit-api-transports.md` were not present.
