<!-- migration-document-status: COMPLETED ARTIFACT -->
> [!NOTE]
> **Migration status: COMPLETED ARTIFACT.** Raw historical evidence only; do not use it as current progress. See `.agents/state/zedflow-ai-agent-pi-fidelity-current-status.md`.

Implemented R8 image registry auth/order parity.

Changed files:
- crates/zedflow-ai/src/images-models.rs
- crates/zedflow-ai/src/providers/all.rs
- crates/zedflow-ai/src/providers/openrouter-images.rs
- crates/zedflow-ai/src/api/openrouter-images.rs
- crates/zedflow-ai/tests/images-models.rs

Image registry tests reactivated/added:
- registers_multiple_providers_in_insertion_order
- resolves_auth_through_provider_and_merges_it_into_requests
- merges_provider_resolved_env_into_image_options
- supports_dynamic_providers_via_refresh_with_in_flight_dedupe
- refresh_wraps_single_provider_failures_and_all_provider_refresh_is_best_effort
- builtin_images_models_resolves_openrouter_api_key_from_auth_context

What changed:
- ImagesModels now stores providers in insertion order instead of unordered HashMap iteration.
- Image provider auth now uses the shared auth::resolve provider auth resolver semantics and supports custom auth context/credentials constructors.
- Image generation merges resolved auth into model/options deterministically: resolved baseUrl into request model, explicit apiKey over resolved apiKey, resolved headers/env first and request headers/env per-key last.
- OpenRouter image provider now registers Pi-style OPENROUTER_API_KEY auth, carries catalog base URLs, and routes generation to the deterministic OpenRouter request-preparation API without live transport.
- OpenRouter image request envelope uses deterministic BTreeMap headers and exposes the selected api_key for no-network validation.

Transport-only blockers for R9:
- OpenRouter image generation still returns the prepared-request failure path instead of executing HTTP.
- OpenRouter image success/failure response transport, response hook invocation, request cancellation, retries/timeouts over a real client, and provider error-body passthrough from live HTTP remain R9.
- Live tests in crates/zedflow-ai/tests/images.rs were not run.

Targeted validation commands/results:
- cargo fmt --all --check: passed
- cargo test -p zedflow-ai --test images-models --no-run: passed
- cargo test -p zedflow-ai --test images-models: passed (10 passed)
- cargo test -p zedflow-ai --lib openrouter_images --no-run: passed
- cargo test -p zedflow-ai --lib openrouter_images: passed (9 passed, 1 ignored, 377 filtered out)
- cargo test -p zedflow-ai --test provider-error-body-passthrough --no-run: passed
- cargo test -p zedflow-ai --test provider-error-body-passthrough: passed (1 passed)

Residual risks:
- OpenRouter live image network transport intentionally not implemented for R8.
- Working tree had extensive pre-existing dirty/untracked files from prior units; no files are staged.
