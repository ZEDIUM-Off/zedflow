<!-- migration-document-status: SUPERSEDED / HISTORICAL -->
> [!CAUTION]
> **Migration status: SUPERSEDED / HISTORICAL.** Do not use this file as the current execution tracker. See `.agents/state/zedflow-ai-agent-pi-fidelity-current-status.md`.

# Zedflow-AI vs Pi-AI port audit summary

Generated after read-only subagent audit wave `d91137ba-363c-494b-8309-bfe326b46419`.

## Verdict

The remaining failures are not mostly isolated bugs. They are port drifts where `zedflow-ai` preserved some Pi request/type data but changed or omitted Pi runtime seams: dynamic providers, async/fallible model loading, runtime auth resolution, event streams, live transports, and compat/faux accounting.

## Primary drift clusters

1. Provider/model/auth runtime
   - Pi providers carry auth/baseUrl/headers, model source, refresh, stream, streamSimple, and optional per-API dispatch.
   - Rust `models.rs` has a smaller provider shape, sync refresh, one stream callback, hardcoded auth, and duplicate minimal types.
   - Fix seam: start in `crates/zedflow-ai/src/models.rs`; unify around Pi-compatible provider shape and existing `auth::resolve`.

2. API transports
   - Pi API functions build payload, execute provider network transport, call hooks, and parse streams/responses.
   - Rust often has validated builders/parsers but public functions return prepared requests, empty streams, or implementation-blocker errors.
   - Biggest gaps: OpenRouter images, Bedrock ConverseStream, Codex SSE/WS, OpenAI Responses/Completions live; Anthropic has raw SSE but sync facade is fixture-only.

3. Stream/event contract
   - Pi has one public async event stream with `result()` and terminal done/error semantics.
   - Rust has a real implementation in `utils/event-stream.rs`, but `types.rs` exports a placeholder and `models.rs` returns `Vec<AssistantMessage>` with minimal messages.
   - Fix seam: make the public `AssistantMessageEventStream` the real stream type, then adapt providers/models.

4. Public API/types
   - Pi root exports a curated side-effect-free facade plus package subpaths.
   - Rust exposes broad module topology and has duplicate public model/content/stream types.
   - No public `genai` leak found, but the root surface is not Pi-shaped.

5. Compat/faux/accounting
   - Pi compat forwards options, short-circuits builtins through `Models`, and faux simulates async event streams plus serialized-context usage/cache accounting.
   - Rust compat drops options in builtin wrappers and lacks Pi builtin short-circuit. Rust faux is synchronous, emits opaque events, and simplified cache/usage accounting omits context/common-prefix/cache-write totals.

6. Tests/residuals
   - P1 started with 286 ignored tests; current audit has 98 ignores and 0 `PORT PLACEHOLDER`.
   - 189 formerly ignored tests were reactivated/fixed.
   - Remaining residuals cluster around live transports, provider/model architecture, image auth/order, stream/abort timing, request capture seams, and JS-only/upstream cases.

## Recommended start point

Do not start with individual provider files. Start with `crates/zedflow-ai/src/models.rs` and `crates/zedflow-ai/src/types.rs`:

1. Replace duplicate minimal model/stream types in `models.rs` with `crate::types` equivalents.
2. Make public `types::AssistantMessageEventStream` use/wrap the real `utils::event_stream::AssistantMessageEventStream`.
3. Extend `Provider`/`CreateProviderOptions` with Pi fields: `auth`, `base_url`, `headers`, `stream_simple`, fallible/async model source, refresh, and per-API dispatch.
4. Route `Models::get_auth` and `Models::stream` through existing `auth::resolve::resolve_provider_auth`.

Only after that should live transports be wired provider-by-provider.

## Source artifact reports

- `.agents/state/port-audit-provider-model-auth.md`
- `.agents/state/port-audit-api-transports.md`
- `.agents/state/port-audit-stream-events.md`
- `.agents/state/port-audit-tests-residuals.md`
- `.agents/state/port-audit-public-api-types.md`
- `.agents/state/port-audit-compat-faux-accounting.md`
