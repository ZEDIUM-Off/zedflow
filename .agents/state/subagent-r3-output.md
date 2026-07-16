<!-- migration-document-status: COMPLETED ARTIFACT -->
> [!NOTE]
> **Migration status: COMPLETED ARTIFACT.** Raw historical evidence only; do not use it as current progress. See `.agents/state/zedflow-ai-agent-pi-fidelity-current-status.md`.

Implemented R3: Pi-compatible provider contract metadata and API dispatch.

Changed files (this R3 run):
- crates/zedflow-ai/src/models.rs
- crates/zedflow-ai/src/providers/static_catalog.rs
- crates/zedflow-ai/src/providers/amazon-bedrock.rs
- crates/zedflow-ai/src/providers/ant-ling.rs
- crates/zedflow-ai/src/providers/faux.rs
- crates/zedflow-ai/tests/models-runtime.rs
- crates/zedflow-ai/tests/providers.rs

New provider fields / behavior:
- Provider now carries base_url, headers, auth metadata, model_source, refresh_source, and api dispatch metadata.
- ProviderAuth metadata represents API-key and OAuth placeholders without implementing OAuth refresh.
- ProviderApi represents either Single(ProviderStreams) or ByApi(HashMap<Api, ProviderStreams>).
- stream() and stream_simple() dispatch by model.api for ByApi providers.
- Missing API implementation returns a terminal AssistantMessageEvent::Error stream with StopReason::Error and a Pi-shaped message: Provider <id> has no API implementation for "<api>".
- Models exposes stream_simple()/complete_simple() delegating through providers.

Builtins/static shells:
- static_catalog derives provider base_url from catalog models and uses ByApi when a static provider has multiple model APIs.
- All built-in chat providers remain static transport shells pending provider live work; notably amazon-bedrock (R12), openai/openai-compatible paths (R10), openai-codex (R11), and OpenRouter image transport remains later R9 scope.
- Faux remains deterministic/test-only and was adapted to the new ProviderApi shape.

Tests added/updated:
- Re-enabled/implemented providers::create_provider_dispatches_on_model_api_for_mixed_api_providers.
- Re-enabled/implemented providers::create_provider_produces_a_stream_error_for_a_model_whose_api_has_no_implementation.
- Updated models-runtime provider helpers to the new ProviderApi contract.

Validation commands/results:
- cargo fmt --all --check: passed
- cargo test -p zedflow-ai --test providers --test models-runtime --no-run: passed
- cargo test -p zedflow-ai --test providers --test models-runtime: passed (17 passed, 12 ignored)
- git diff --cached --name-only: no output; no staged files

Residual risks:
- Provider auth is metadata only; Models::get_auth still has hardcoded built-in branches for R4.
- Refresh remains synchronous and non-deduped for R5.
- Live provider transports were intentionally not implemented for R9-R12.
- Working tree contained many pre-existing dirty/untracked files from prior units; changed-file list above is limited to this R3 run.
