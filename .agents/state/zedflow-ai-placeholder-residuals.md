<!-- migration-document-status: SUPERSEDED / HISTORICAL -->
> [!CAUTION]
> **Migration status: SUPERSEDED / HISTORICAL.** Do not use this file as the current execution tracker. See `.agents/state/zedflow-ai-agent-pi-fidelity-current-status.md`.

# Zedflow AI placeholder residuals
Generated: 2026-07-08

## Final audit summary
- `grep -R -n "PORT PLACEHOLDER" crates/zedflow-ai/src crates/zedflow-ai/tests`: 117 matches, all in ignored/scaffolded test files; no source placeholders remain.
- `grep -R -n "genai::|pub use genai|pub .*genai" crates/zedflow-ai/src`: genai references are confined to `src/utils/genai-backend.rs` plus the public Bedrock parity-report function name; no public API exports genai types and `utils::genai_backend` is `pub(crate)`.
- Remaining ignored tests: 286 `#[ignore]` attributes; all are accepted live/manual/provider/OAuth/network cases or local parity scaffolds whose implementation seam is still absent.

## Deterministic tests unignored or repaired by U11
- `tests/anthropic-adaptive-thinking-models.rs` adaptive-thinking catalog test
- `tests/together-models.rs` two Together catalog tests
- `tests/xiaomi-models.rs` two Xiaomi catalog tests
- `tests/providers.rs` builtin provider/model catalog smoke
- `tests/zen.rs` updated to live-only ignored residual with deterministic catalog assertion
- `tests/stream.rs` compat catalog smoke repaired
- `tests/tool-call-id-normalization.rs` compat model-registration smoke repaired
- Inline deterministic tests repaired in `bedrock-converse-stream.rs`, `openai-completions.rs`, `image-models.rs`, and `utils/validation.rs`

## Remaining `PORT PLACEHOLDER` matches
| File | Lines | Accepted reason |
|---|---:|---|
| `crates/zedflow-ai/tests/anthropic-eager-tool-input-e2e.rs` | 15, 84, 89, 104, 117 | ignored deterministic scaffold still depends on compat stream_simple/on_payload event capture rather than the U4 raw payload/SSE helpers |
| `crates/zedflow-ai/tests/anthropic-empty-thinking-signature-compat.rs` | 8, 112, 124 | ignored deterministic scaffold still uses local capture_payload placeholder; payload builder exists but compat capture seam is not wired into this parity test |
| `crates/zedflow-ai/tests/anthropic-long-cache-retention-e2e.rs` | 6, 18, 55, 123, 146 | live/provider cache-retention behavior plus missing full cost/sorting parity; no live network in U11 |
| `crates/zedflow-ai/tests/anthropic-opus-4-8-smoke.rs` | 8, 88 | live Anthropic provider smoke requires credentials/network and full stream dispatch |
| `crates/zedflow-ai/tests/anthropic-temperature-compat.rs` | 7, 116, 124, 132, 140, 148, 156 | ignored deterministic scaffold still uses local capture_payload placeholder; compat stream_simple/on_payload capture is not wired |
| `crates/zedflow-ai/tests/anthropic-thinking-disable.rs` | 3, 72, 84, 96, 108, 117, 137, 157 | payload-capture scaffold and live no-thinking E2E remain split; live case needs provider network |
| `crates/zedflow-ai/tests/bedrock-convert-messages.rs` | 3, 10, 37, 62, 83, 101, 115, 136, 151, 163, 184 | fixture scaffolds still document unported exact Bedrock event/payload edges; no live AWS |
| `crates/zedflow-ai/tests/bedrock-models.rs` | 7 | live Bedrock per-model calls require AWS credentials/network |
| `crates/zedflow-ai/tests/bedrock-thinking-payload.rs` | 6, 68, 91, 114, 132, 150, 167, 192, 251, 273, 308 | fixture scaffolds still document exact Bedrock thinking payload parity gaps |
| `crates/zedflow-ai/tests/codex-websocket-cached-probe.rs` | 13 | manual/live OpenAI Codex WebSocket cached probe requires OAuth/network |
| `crates/zedflow-ai/tests/faux-provider.rs` | 4, 17, 171, 177, 184, 190, 196, 202, 208, 214, 220, 226, 232, 238, 244, 250, 256, 262, 268, 274, 280, 286, 292, 298 | advanced faux compat parity (typed events, async factories, cache/abort pacing) remains beyond local core queue coverage |
| `crates/zedflow-ai/tests/github-copilot-anthropic.rs` | 9, 92, 151 | Copilot Anthropic request capture still lacks deterministic stream/client construction seam |
| `crates/zedflow-ai/tests/lazy-module-load.rs` | 9, 60, 69, 78, 87, 96 | Node dynamic import/registerHooks observability is JS-only; Rust static lazy dispatch is covered elsewhere |
| `crates/zedflow-ai/tests/providers.rs` | 86, 110, 118 | provider auth/env/mixed API dynamic parity cases remain unimplemented while builtin catalog basics are now covered |
| `crates/zedflow-ai/tests/scratch.rs` | 5, 20 | manual live Anthropic scratch script requires credentials/network |
| `crates/zedflow-ai/tests/stream.rs` | 7 | live provider/local Ollama E2E matrix requires credentials/network |
| `crates/zedflow-ai/tests/supports-xhigh.rs` | 5, 10, 24, 33, 42, 51, 62, 71, 80, 89, 101, 113, 125, 137, 146, 164, 173, 185, 194 | full lazy::Model reasoning metadata/getSupportedThinkingLevels surface not exposed yet |
| `crates/zedflow-ai/tests/xhigh.rs` | 4, 16, 22, 28 | live OpenAI xhigh provider parity requires network and full transport |
| `crates/zedflow-ai/tests/xiaomi-token-plan-ams-anthropic-empty-signature-smoke.rs` | 11 | live/provider Xiaomi token-plan Anthropic smoke requires credentials/network |

## Remaining ignored test ledger
| File | Lines | Ignore reason |
|---|---:|---|
| `crates/zedflow-ai/tests/abort.rs` | 118, 131, 141 | live provider parity test; see BLOCKER |
| `crates/zedflow-ai/tests/anthropic-eager-tool-input-e2e.rs` | 89 | PORT PLACEHOLDER: compat::get_providers/get_models still return placeholders until the generated provider catalog is wired |
| `crates/zedflow-ai/tests/anthropic-eager-tool-input-e2e.rs` | 117 | live provider call skipped; compat Model eager-tool-input metadata remains a PORT PLACEHOLDER |
| `crates/zedflow-ai/tests/anthropic-eager-tool-input-e2e.rs` | 104 | live provider call skipped; compat catalog, Context tools, and provider streaming remain PORT PLACEHOLDERs |
| `crates/zedflow-ai/tests/anthropic-empty-thinking-signature-compat.rs` | 112, 124 | PORT PLACEHOLDER: anthropic payload construction/on_payload capture is not ported |
| `crates/zedflow-ai/tests/anthropic-long-cache-retention-e2e.rs` | 123 | PORT PLACEHOLDER: compat::get_providers/get_models still return placeholders until the generated provider catalog is wired |
| `crates/zedflow-ai/tests/anthropic-long-cache-retention-e2e.rs` | 146 | live provider call skipped; compat catalog, Anthropic long-cache-retention compat override, thinkingEnabled option, and provider streaming remain PORT PLACEHOLDERs |
| `crates/zedflow-ai/tests/anthropic-oauth.rs` | 38 | ignored: Anthropic OAuth localhost/manual callback parity requires browser/local-server automation; no live provider calls are allowed |
| `crates/zedflow-ai/tests/anthropic-oauth.rs` | 66 | ignored: Anthropic OAuth manual-code prompt abort parity requires interactive callback automation |
| `crates/zedflow-ai/tests/anthropic-oauth.rs` | 52 | ignored: Anthropic OAuth refresh parity requires injectable HTTP; no live provider calls are allowed |
| `crates/zedflow-ai/tests/anthropic-opus-4-8-smoke.rs` | 88 | live provider call skipped; compat catalog/builtin dispatch plus anthropic streamSimple/onPayload/SSE are PORT PLACEHOLDERs |
| `crates/zedflow-ai/tests/anthropic-temperature-compat.rs` | 116, 124, 132, 140, 148, 156 | PORT PLACEHOLDER: anthropic payload construction/on_payload capture is not ported |
| `crates/zedflow-ai/tests/anthropic-thinking-disable.rs` | 72, 84, 96, 108, 117, 137, 157 | PORT PLACEHOLDER: anthropic request-payload construction/on_payload capture is not ported |
| `crates/zedflow-ai/tests/anthropic-thinking-disable.rs` | 177 | live Anthropic API parity test skipped: no live provider calls in P1.T2 |
| `crates/zedflow-ai/tests/bedrock-convert-messages.rs` | 37, 62, 83, 101, 115, 136, 151, 163, 184 | PORT PLACEHOLDER: Bedrock Converse payload construction/on_payload capture is not ported |
| `crates/zedflow-ai/tests/bedrock-models.rs` | 44 | live Bedrock provider parity test skipped; see LIVE_BLOCKER |
| `crates/zedflow-ai/tests/bedrock-thinking-payload.rs` | 68, 91, 114, 132, 150, 167, 192, 251, 273, 308 | PORT PLACEHOLDER: Bedrock request-payload construction/on_payload capture is not ported |
| `crates/zedflow-ai/tests/bedrock-thinking-payload.rs` | 234 | live Bedrock provider parity test requires AWS credentials and network calls |
| `crates/zedflow-ai/tests/cache-retention.rs` | 102 | anthropic_messages::stream does not build request payloads until the Anthropic SDK/HTTP-SSE dependency is selected |
| `crates/zedflow-ai/tests/codex-websocket-cached-probe.rs` | 330 | live provider/AuthStorage/websocket-cached transport parity is blocked; see BLOCKER |
| `crates/zedflow-ai/tests/context-overflow.rs` | 248 | live provider/network/local-LLM parity test skipped; see LIVE_PROVIDER_BLOCKER |
| `crates/zedflow-ai/tests/cross-provider-handoff.rs` | 424 | live provider test skipped; compat catalog/provider dispatch, OAuth resolveApiKey, completeSimple, and real provider streams are request-capture blockers |
| `crates/zedflow-ai/tests/cross-provider-handoff.rs` | 439 | live provider test skipped; handoff requests require generated live fixtures and completeSimple provider calls |
| `crates/zedflow-ai/tests/empty.rs` | 213, 222, 231, 240 | live provider parity test; see BLOCKER |
| `crates/zedflow-ai/tests/faux-provider.rs` | 280 | PORT PLACEHOLDER: AbortSignal equivalent and paced text streaming are not ported |
| `crates/zedflow-ai/tests/faux-provider.rs` | 286 | PORT PLACEHOLDER: AbortSignal equivalent and paced thinking streaming are not ported |
| `crates/zedflow-ai/tests/faux-provider.rs` | 292 | PORT PLACEHOLDER: AbortSignal equivalent and paced tool-call streaming are not ported |
| `crates/zedflow-ai/tests/faux-provider.rs` | 274 | PORT PLACEHOLDER: AbortSignal equivalent and pre-chunk abort handling are not ported |
| `crates/zedflow-ai/tests/faux-provider.rs` | 177 | PORT PLACEHOLDER: AssistantContent is opaque; text/thinking/tool-call block variants are not ported |
| `crates/zedflow-ai/tests/faux-provider.rs` | 214 | PORT PLACEHOLDER: FauxResponseFactory cannot return errors/panics as assistant error events yet |
| `crates/zedflow-ai/tests/faux-provider.rs` | 208 | PORT PLACEHOLDER: FauxResponseFactory is synchronous; async response factories are not ported |
| `crates/zedflow-ai/tests/faux-provider.rs` | 238 | PORT PLACEHOLDER: cacheRetention options and prompt-cache accounting are not ported |
| `crates/zedflow-ai/tests/faux-provider.rs` | 298 | PORT PLACEHOLDER: compat faux registration/unregistration is not ported |
| `crates/zedflow-ai/tests/faux-provider.rs` | 196, 202 | PORT PLACEHOLDER: compat::complete path cannot create a registered faux provider yet |
| `crates/zedflow-ai/tests/faux-provider.rs` | 190 | PORT PLACEHOLDER: compat::register_faux_provider is not wired to providers::faux |
| `crates/zedflow-ai/tests/faux-provider.rs` | 171 | PORT PLACEHOLDER: compat::register_faux_provider is not wired to providers::faux and usage estimates are not implemented |
| `crates/zedflow-ai/tests/faux-provider.rs` | 250 | PORT PLACEHOLDER: fixed-size chunking and exact stream event order are not ported |
| `crates/zedflow-ai/tests/faux-provider.rs` | 184 | PORT PLACEHOLDER: model reasoning metadata is not exposed on lazy::Model |
| `crates/zedflow-ai/tests/faux-provider.rs` | 256 | PORT PLACEHOLDER: multiple typed tool-call stream events are not ported |
| `crates/zedflow-ai/tests/faux-provider.rs` | 226, 232 | PORT PLACEHOLDER: sessionId/cacheRetention options and prompt-cache accounting are not ported |
| `crates/zedflow-ai/tests/faux-provider.rs` | 268 | PORT PLACEHOLDER: terminal aborted event ordering after partial deltas is not ported |
| `crates/zedflow-ai/tests/faux-provider.rs` | 262 | PORT PLACEHOLDER: terminal error event ordering after partial deltas is not ported |
| `crates/zedflow-ai/tests/faux-provider.rs` | 220 | PORT PLACEHOLDER: typed Context/messages/tools and usage estimation are not ported |
| `crates/zedflow-ai/tests/faux-provider.rs` | 244 | PORT PLACEHOLDER: typed stream delta events for thinking/text/tool calls are not ported |
| `crates/zedflow-ai/tests/github-copilot-anthropic.rs` | 92 | PORT PLACEHOLDER: Anthropic Messages stream/client construction is not ported, so Bearer auth and request payload capture cannot run deterministically yet |
| `crates/zedflow-ai/tests/github-copilot-anthropic.rs` | 151 | PORT PLACEHOLDER: Anthropic Messages stream/client construction is not ported, so beta header capture cannot run deterministically yet |
| `crates/zedflow-ai/tests/github-copilot-oauth.rs` | 105 | ignored: loginGitHubCopilot access-token polling HTTP adapter requires injectable HTTP/fake timers; no live provider calls are allowed |
| `crates/zedflow-ai/tests/github-copilot-oauth.rs` | 117 | ignored: loginGitHubCopilot access-token polling timeout path requires injectable HTTP/fake timers; no live provider calls are allowed |
| `crates/zedflow-ai/tests/github-copilot-oauth.rs` | 67 | ignored: loginGitHubCopilot device-code HTTP flow and onDeviceCode callback path require injectable HTTP/fake timers; no live provider calls are allowed |
| `crates/zedflow-ai/tests/github-copilot-oauth.rs` | 83 | ignored: loginGitHubCopilot device-code response deserialization/verification_uri trust boundary requires injectable HTTP/fake timers; no live provider calls are allowed |
| `crates/zedflow-ai/tests/github-copilot-oauth.rs` | 92 | ignored: loginGitHubCopilot verification_uri URL normalization before onDeviceCode requires injectable HTTP/fake timers; no live provider calls are allowed |
| `crates/zedflow-ai/tests/google-thinking-disable.rs` | 72, 88 | live Anthropic API parity test skipped: requires ANTHROPIC_API_KEY and provider network calls |
| `crates/zedflow-ai/tests/google-thinking-disable.rs` | 104, 113, 122 | live Google Gemini API parity test skipped: requires GEMINI_API_KEY and provider network calls |
| `crates/zedflow-ai/tests/google-thinking-disable.rs` | 138, 155 | live Google Vertex parity test skipped: requires GOOGLE_CLOUD_API_KEY or GOOGLE_CLOUD_PROJECT/GOOGLE_CLOUD_LOCATION and provider network calls |
| `crates/zedflow-ai/tests/google-thinking-disable.rs` | 172 | live OpenAI API parity test skipped: requires OPENAI_API_KEY and provider network calls |
| `crates/zedflow-ai/tests/google-thinking-disable.rs` | 187 | live OpenRouter API parity test skipped: requires OPENROUTER_API_KEY and provider network calls |
| `crates/zedflow-ai/tests/image-tool-result.rs` | 541 | live provider call skipped; image tool-result context, typed assistant content, and provider streaming are request-capture blockers |
| `crates/zedflow-ai/tests/image-tool-result.rs` | 550 | live provider call skipped; text+image tool-result context, typed assistant content, and provider streaming are request-capture blockers |
| `crates/zedflow-ai/tests/image-tool-result.rs` | 559 | matches Pi it.skip: Xiaomi MiMo text+image tool results currently ignore image color due upstream multimodal-fusion quality |
| `crates/zedflow-ai/tests/images-models.rs` | 189 | blocked: ImagesModels uses HashMap, not Pi's insertion-ordered Map, so multi-provider order assertions are not yet deterministic |
| `crates/zedflow-ai/tests/images-models.rs` | 197 | blocked: Rust ImagesProvider lacks Pi provider auth resolver and auth context support |
| `crates/zedflow-ai/tests/images-models.rs` | 205 | blocked: Rust ImagesProvider lacks resolved provider env support |
| `crates/zedflow-ai/tests/images-models.rs` | 213 | blocked: Rust create_images_provider does not expose Pi's concurrent in-flight refresh de-duplication semantics |
| `crates/zedflow-ai/tests/images-models.rs` | 221 | blocked: builtin_images_models does not accept Pi auth context or resolve OPENROUTER_API_KEY yet |
| `crates/zedflow-ai/tests/images.rs` | 19, 48, 83 | live OpenRouter image parity test; see BLOCKER |
| `crates/zedflow-ai/tests/interleaved-thinking.rs` | 213, 223 | live Anthropic provider call skipped; requires credentials/network and completeSimple/provider streaming parity not yet ported |
| `crates/zedflow-ai/tests/interleaved-thinking.rs` | 190, 203 | live Bedrock provider call skipped; requires credentials/network and completeSimple/provider streaming parity not yet ported |
| `crates/zedflow-ai/tests/lazy-module-load.rs` | 87 | PORT PLACEHOLDER: Anthropic lazy API wrapper still returns a provider-stream placeholder |
| `crates/zedflow-ai/tests/lazy-module-load.rs` | 69 | PORT PLACEHOLDER: builtin provider catalog/lazy SDK-load observability is not ported yet |
| `crates/zedflow-ai/tests/lazy-module-load.rs` | 78 | PORT PLACEHOLDER: compat entrypoint lazy SDK-load observability is not ported yet |
| `crates/zedflow-ai/tests/lazy-module-load.rs` | 96 | PORT PLACEHOLDER: compat getModel/streamSimple and builtin provider dispatch are not ported yet |
| `crates/zedflow-ai/tests/lazy-module-load.rs` | 60 | PORT PLACEHOLDER: no Rust equivalent for Node registerHooks/dynamic import SDK-load probe yet |
| `crates/zedflow-ai/tests/models-runtime.rs` | 178 | source parity blocker: OAuth refresh errors are not wired through chat Models |
| `crates/zedflow-ai/tests/models-runtime.rs` | 170 | source parity blocker: OAuth refresh/persistence is not wired through chat Models |
| `crates/zedflow-ai/tests/models-runtime.rs` | 206 | source parity blocker: api-key auth failure wrapping is not wired through chat Models |
| `crates/zedflow-ai/tests/models-runtime.rs` | 220 | source parity blocker: auth merging into stream options is not ported for chat Models |
| `crates/zedflow-ai/tests/models-runtime.rs` | 162 | source parity blocker: chat Models auth resolution is placeholder-only |
| `crates/zedflow-ai/tests/models-runtime.rs` | 37 | source parity blocker: chat Models is still a minimal placeholder and does not preserve provider insertion order or provider identity |
| `crates/zedflow-ai/tests/models-runtime.rs` | 64 | source parity blocker: chat Models stores providers in HashMap, so all-provider model listing is not Pi Map insertion ordered |
| `crates/zedflow-ai/tests/models-runtime.rs` | 242 | source parity blocker: chat stream events are Vec<AssistantMessage>, not Pi start/done event streams with result() |
| `crates/zedflow-ai/tests/models-runtime.rs` | 154 | source parity blocker: createModels has no credential store injection and Models::get_auth returns placeholder auth |
| `crates/zedflow-ai/tests/models-runtime.rs` | 200 | source parity blocker: credential store failure wrapping is not wired through chat Models |
| `crates/zedflow-ai/tests/models-runtime.rs` | 186 | source parity blocker: credential-store-backed OAuth refresh serialization is not wired through chat Models |
| `crates/zedflow-ai/tests/models-runtime.rs` | 99 | source parity blocker: current Provider cannot model getModels throwing, so best-effort source failure behavior is unimplemented |
| `crates/zedflow-ai/tests/models-runtime.rs` | 107 | source parity blocker: refresh is sync/minimal and does not preserve Pi async/in-flight refresh semantics |
| `crates/zedflow-ai/tests/models-runtime.rs` | 212 | source parity blocker: request auth resolution, provider env, and completeSimple are not ported for chat Models |
| `crates/zedflow-ai/tests/models-runtime.rs` | 234 | source parity blocker: unknown provider currently returns an empty/default stream, not a Pi error AssistantMessage |
| `crates/zedflow-ai/tests/models-runtime.rs` | 194 | source parity blocker: valid OAuth token fast path is not wired through chat Models |
| `crates/zedflow-ai/tests/oauth-auth.rs` | 86 | ignored: Anthropic refresh requires a live OAuth token endpoint or injectable HTTP fixture |
| `crates/zedflow-ai/tests/oauth-auth.rs` | 95 | ignored: GitHub Copilot refresh requires live provider endpoints or injectable HTTP fixtures |
| `crates/zedflow-ai/tests/oauth-auth.rs` | 110, 119 | ignored: Models::get_auth depends on U9 provider factory wiring, not live OAuth auth |
| `crates/zedflow-ai/tests/openai-codex-cache-affinity-e2e.rs` | 47 | live provider call skipped; see BLOCKER |
| `crates/zedflow-ai/tests/openai-codex-oauth.rs` | 179 | ignored: Rust auth callback surface cannot yet model cancelled selection as Pi's undefined onSelect result |
| `crates/zedflow-ai/tests/openai-codex-oauth.rs` | 205 | ignored: loginOpenAICodexDeviceCode 403/404 pending polling behavior is; no live provider calls are allowed |
| `crates/zedflow-ai/tests/openai-codex-oauth.rs` | 80 | ignored: loginOpenAICodexDeviceCode HTTP polling, token exchange, timers, and callback delivery are; no live provider calls are allowed |
| `crates/zedflow-ai/tests/openai-codex-oauth.rs` | 187 | ignored: loginOpenAICodexDeviceCode cancellation while waiting is; no live provider calls are allowed |
| `crates/zedflow-ai/tests/openai-codex-oauth.rs` | 222 | ignored: loginOpenAICodexDeviceCode device-auth error-body passthrough is; no live provider calls are allowed |
| `crates/zedflow-ai/tests/openai-codex-oauth.rs` | 196 | ignored: loginOpenAICodexDeviceCode timeout path is; no live provider calls are allowed |
| `crates/zedflow-ai/tests/openai-codex-oauth.rs` | 136 | ignored: openaiCodexOAuthProvider.login device-code selection uses live device-code HTTP flow |
| `crates/zedflow-ai/tests/openai-codex-oauth.rs` | 233 | ignored: refreshOpenAICodexToken HTTP refresh failure path is; no live provider calls are allowed |
| `crates/zedflow-ai/tests/openai-codex-stream.rs` | 429 | stream cannot age out fake cached WebSocket sessions yet |
| `crates/zedflow-ai/tests/openai-codex-stream.rs` | 316 | stream cannot capture fake SSE request headers yet |
| `crates/zedflow-ai/tests/openai-codex-stream.rs` | 208 | stream cannot capture fake SSE request headers/body yet |
| `crates/zedflow-ai/tests/openai-codex-stream.rs` | 265 | stream cannot capture fake request reasoning payload yet |
| `crates/zedflow-ai/tests/openai-codex-stream.rs` | 507 | stream cannot capture zstd-compressed SSE request bodies yet |
| `crates/zedflow-ai/tests/openai-codex-stream.rs` | 148 | stream cannot consume fake SSE responses yet |
| `crates/zedflow-ai/tests/openai-codex-stream.rs` | 530 | stream cannot exercise fake exponential backoff SSE retries yet |
| `crates/zedflow-ai/tests/openai-codex-stream.rs` | 490 | stream cannot exercise fake retry-after SSE retries yet |
| `crates/zedflow-ai/tests/openai-codex-stream.rs` | 231 | stream cannot expose on_payload/captured payload yet |
| `crates/zedflow-ai/tests/openai-codex-stream.rs` | 357 | stream cannot fall back from fake WebSocket connect timeout to fake SSE yet |
| `crates/zedflow-ai/tests/openai-codex-stream.rs` | 393 | stream cannot fall back from fake idle WebSocket to fake SSE yet |
| `crates/zedflow-ai/tests/openai-codex-stream.rs` | 166 | stream cannot map fake response.incomplete SSE events yet |
| `crates/zedflow-ai/tests/openai-codex-stream.rs` | 287 | stream cannot process fake service_tier usage costs yet |
| `crates/zedflow-ai/tests/openai-codex-stream.rs` | 380 | stream cannot reconnect fake WebSockets yet |
| `crates/zedflow-ai/tests/openai-codex-stream.rs` | 412 | stream cannot report fake idle WebSocket errors yet |
| `crates/zedflow-ai/tests/openai-codex-stream.rs` | 449 | stream cannot send fake cached WebSocket input deltas yet |
| `crates/zedflow-ai/tests/openai-codex-stream.rs` | 157 | stream cannot terminate from fake response.completed SSE before body close yet |
| `crates/zedflow-ai/tests/openai-codex-stream.rs` | 193 | stream has no abortable fake SSE body-read seam yet |
| `crates/zedflow-ai/tests/openai-codex-stream.rs` | 175 | stream has no abortable fake SSE fetch seam yet |
| `crates/zedflow-ai/tests/openai-codex-stream.rs` | 245 | stream_simple cannot capture fake request reasoning payload yet |
| `crates/zedflow-ai/tests/openai-codex-stream.rs` | 326 | stream_simple cannot use fake WebSocket cached context yet |
| `crates/zedflow-ai/tests/openai-completions-cache-control-format.rs` | 125, 133, 141 | OpenAI Completions payload construction/on_payload capture is not ported |
| `crates/zedflow-ai/tests/openai-completions-empty-tools.rs` | 249, 273, 300 | OpenAI request client options capture is not ported |
| `crates/zedflow-ai/tests/openai-completions-empty-tools.rs` | 93, 111, 126, 146, 166, 186, 331 | OpenAI request params capture is not ported |
| `crates/zedflow-ai/tests/openai-completions-empty-tools.rs` | 207 | OpenAI request params/client options capture is not ported |
| `crates/zedflow-ai/tests/openai-completions-response-model.rs` | 69, 104, 134 | openai_completions::stream cannot consume a fake OpenAI chunk stream yet |
| `crates/zedflow-ai/tests/openai-completions-tool-choice.rs` | 553 | Ant Ling built-in compat metadata/request mapping is not fully ported |
| `crates/zedflow-ai/tests/openai-completions-tool-choice.rs` | 565 | Ant Ling reasoning request mapping is not fully ported |
| `crates/zedflow-ai/tests/openai-completions-tool-choice.rs` | 451, 459 | Moonshot Kimi thinking request payload mapping is not ported |
| `crates/zedflow-ai/tests/openai-completions-tool-choice.rs` | 435, 443 | OpenCode Go thinking request payload mapping is not ported |
| `crates/zedflow-ai/tests/openai-completions-tool-choice.rs` | 476 | OpenCode Grok Build reasoning request mapping is not ported |
| `crates/zedflow-ai/tests/openai-completions-tool-choice.rs` | 333 | OpenRouter Kimi K2.6 built-in compat metadata is not ported |
| `crates/zedflow-ai/tests/openai-completions-tool-choice.rs` | 511 | OpenRouter reasoning object request mapping is not ported |
| `crates/zedflow-ai/tests/openai-completions-tool-choice.rs` | 345 | Xiaomi MiMo built-in compat metadata is not ported |
| `crates/zedflow-ai/tests/openai-completions-tool-choice.rs` | 359 | Xiaomi MiMo request payload mapping is not ported |
| `crates/zedflow-ai/tests/openai-completions-tool-choice.rs` | 542 | chat template effort kwargs request mapping is not ported |
| `crates/zedflow-ai/tests/openai-completions-tool-choice.rs` | 520 | chat template thinking kwargs request mapping is not ported |
| `crates/zedflow-ai/tests/openai-completions-tool-choice.rs` | 467 | max_tokens request payload mapping is not ported |
| `crates/zedflow-ai/tests/openai-completions-tool-choice.rs` | 531 | qwen chat template thinking kwargs request mapping is not ported |
| `crates/zedflow-ai/tests/openai-completions-tool-choice.rs` | 99, 107 | reasoning request payload mapping is not ported |
| `crates/zedflow-ai/tests/openai-completions-tool-choice.rs` | 75, 89 | request payload capture is not ported |
| `crates/zedflow-ai/tests/openai-completions-tool-choice.rs` | 224 | stream finish_reason decoding is not ported |
| `crates/zedflow-ai/tests/openai-completions-tool-choice.rs` | 271 | stream mixed content/reasoning/tool-call decoding is not ported |
| `crates/zedflow-ai/tests/openai-completions-tool-choice.rs` | 244 | stream null finish_reason handling is not ported |
| `crates/zedflow-ai/tests/openai-completions-tool-choice.rs` | 232 | stream null-chunk handling is not ported |
| `crates/zedflow-ai/tests/openai-completions-tool-choice.rs` | 376, 387 | stream reasoning delta decoding is not ported |
| `crates/zedflow-ai/tests/openai-completions-tool-choice.rs` | 257 | stream tool-call delta coalescing is not ported |
| `crates/zedflow-ai/tests/openai-completions-tool-choice.rs` | 484, 493, 502 | stream usage accounting is not ported |
| `crates/zedflow-ai/tests/openai-completions-tool-choice.rs` | 123 | z.ai built-in compat metadata is not ported |
| `crates/zedflow-ai/tests/openai-completions-tool-choice.rs` | 134 | z.ai built-in thinking-level metadata is not ported |
| `crates/zedflow-ai/tests/openai-completions-tool-choice.rs` | 115, 146, 170, 191, 200, 208, 216 | z.ai request payload mapping is not ported |
| `crates/zedflow-ai/tests/openai-responses-cache-affinity-e2e.rs` | 47 | live provider call skipped; see BLOCKER |
| `crates/zedflow-ai/tests/openai-responses-reasoning-replay-e2e.rs` | 114, 128, 148 | live provider call skipped; see BLOCKER |
| `crates/zedflow-ai/tests/openrouter-cache-write-repro.rs` | 111 | live OpenRouter provider call skipped; see BLOCKER |
| `crates/zedflow-ai/tests/provider-error-body-passthrough.rs` | 16 | OpenRouter image transport is not implemented/injectable; see BLOCKER |
| `crates/zedflow-ai/tests/provider-error-body-regression.rs` | 124, 143, 168, 183 | provider streaming/catch paths need fake transport injection |
| `crates/zedflow-ai/tests/providers.rs` | 102 | parity blocker: Cloudflare Workers AI provider documents missing auth/API fields in the current Rust Provider shape |
| `crates/zedflow-ai/tests/providers.rs` | 94 | parity blocker: Models::get_auth currently returns default auth and does not evaluate ambient Bedrock credential env |
| `crates/zedflow-ai/tests/providers.rs` | 180 | parity blocker: Provider auth resolution and request/env merge options are not represented in models.rs StreamOptions yet |
| `crates/zedflow-ai/tests/providers.rs` | 86 | parity blocker: anthropic_provider is a PORT PLACEHOLDER and Models::get_auth has no provider auth contract yet |
| `crates/zedflow-ai/tests/providers.rs` | 110 | parity blocker: cloudflare_ai_gateway_provider is a PORT PLACEHOLDER until auth resolver and mixed API wiring exist |
| `crates/zedflow-ai/tests/providers.rs` | 188 | parity blocker: create_provider has no mixed API map, so missing API implementations cannot synthesize Pi stream errors yet |
| `crates/zedflow-ai/tests/providers.rs` | 174 | parity blocker: create_provider only accepts one stream callback; Pi's per-model API dispatch map is not represented yet |
| `crates/zedflow-ai/tests/providers.rs` | 118 | parity blocker: google_vertex_provider is a PORT PLACEHOLDER until ADC/API-key auth and model wiring exist |
| `crates/zedflow-ai/tests/providers.rs` | 194 | parity blocker: refresh_models is synchronous and does not dedupe concurrent in-flight refreshes like Pi's async provider refresh |
| `crates/zedflow-ai/tests/responseid.rs` | 150 | live Anthropic API parity test skipped: requires ANTHROPIC_API_KEY and provider network calls |
| `crates/zedflow-ai/tests/responseid.rs` | 159 | live Azure OpenAI Responses parity test skipped: requires Azure OpenAI credentials and provider network calls |
| `crates/zedflow-ai/tests/responseid.rs` | 192 | live GitHub Copilot Anthropic-path parity test skipped: requires resolved github-copilot OAuth token and provider network calls |
| `crates/zedflow-ai/tests/responseid.rs` | 180 | live GitHub Copilot OpenAI-path parity test skipped: requires resolved github-copilot OAuth token and provider network calls |
| `crates/zedflow-ai/tests/responseid.rs` | 98 | live Google Gemini API parity test skipped: requires GEMINI_API_KEY and provider network calls |
| `crates/zedflow-ai/tests/responseid.rs` | 107 | live Google Vertex ADC parity test skipped: requires GOOGLE_CLOUD_PROJECT/GOOGLE_CLOUD_LOCATION and provider network calls |
| `crates/zedflow-ai/tests/responseid.rs` | 120 | live Google Vertex API key parity test skipped: requires GOOGLE_CLOUD_API_KEY and provider network calls |
| `crates/zedflow-ai/tests/responseid.rs` | 171 | live Mistral API parity test skipped: requires MISTRAL_API_KEY and provider network calls |
| `crates/zedflow-ai/tests/responseid.rs` | 204 | live OpenAI Codex parity test skipped: requires resolved openai-codex OAuth token and provider network calls |
| `crates/zedflow-ai/tests/responseid.rs` | 132 | live OpenAI Completions parity test skipped: requires OPENAI_API_KEY and provider network calls |
| `crates/zedflow-ai/tests/responseid.rs` | 141 | live OpenAI Responses parity test skipped: requires OPENAI_API_KEY and provider network calls |
| `crates/zedflow-ai/tests/scratch.rs` | 20 | live Anthropic scratch script requires ANTHROPIC_API_KEY; provider/auth/completeSimple/streamSimple remain PORT PLACEHOLDERs |
| `crates/zedflow-ai/tests/stream.rs` | 459 | live provider/local Ollama E2E suite skipped; see BLOCKER |
| `crates/zedflow-ai/tests/supports-xhigh.rs` | 24, 33, 42, 51, 62, 71, 80, 89, 101, 113, 125, 137, 146, 164, 173, 185, 194 | PORT PLACEHOLDER: compat catalog reads and supported thinking level metadata are not ported |
| `crates/zedflow-ai/tests/tokens.rs` | 278 | live provider parity test skipped; see BLOCKER |
| `crates/zedflow-ai/tests/tokens.rs` | 291 | source test has explicit it.skip Xiaomi cases; see upstream_skip_reason |
| `crates/zedflow-ai/tests/tool-call-id-normalization.rs` | 141, 158, 171, 186 | live provider parity test; see BLOCKER |
| `crates/zedflow-ai/tests/tool-call-without-result.rs` | 257 | live provider parity suite needs compat getModel/complete and network credentials; see BLOCKER |
| `crates/zedflow-ai/tests/total-tokens.rs` | 289 | live provider parity test skipped; see BLOCKER |
| `crates/zedflow-ai/tests/unicode-surrogate.rs` | 407 | live provider call skipped; Rust cannot construct JS lone-surrogate strings and compat/tool-result context is incomplete; see BLOCKER |
| `crates/zedflow-ai/tests/unicode-surrogate.rs` | 380, 391 | live provider call skipped; see BLOCKER |
| `crates/zedflow-ai/tests/xhigh.rs` | 28 | PORT PLACEHOLDER: live OpenAI Completions xhigh error parity needs compat catalog and provider transport |
| `crates/zedflow-ai/tests/xhigh.rs` | 22 | PORT PLACEHOLDER: live OpenAI Responses xhigh error parity needs compat catalog and provider transport |
| `crates/zedflow-ai/tests/xhigh.rs` | 16 | PORT PLACEHOLDER: live OpenAI Responses xhigh stream parity needs compat catalog and provider transport |
| `crates/zedflow-ai/tests/xiaomi-token-plan-ams-anthropic-empty-signature-smoke.rs` | 11 | PORT PLACEHOLDER: live Xiaomi Token Plan Anthropic transport and replay payload capture are not ported |
| `crates/zedflow-ai/tests/zen.rs` | 10 | live OpenCode smoke requires provider network credentials; deterministic catalog coverage is local |

## Remaining blockers
- Live provider/network/manual OAuth tests remain intentionally ignored.
- Several parity scaffold tests still contain local `panic!(BLOCKER)` helpers for unported capture seams (not run by deterministic test gate).
- `utils::genai_backend` is currently unused by provider transports and emits dead-code warnings, but remains `pub(crate)` and does not leak genai types.
