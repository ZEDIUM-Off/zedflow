<!-- migration-document-status: COMPLETED ARTIFACT -->
> [!NOTE]
> **Migration status: COMPLETED ARTIFACT.** Raw historical evidence only; do not use it as current progress. See `.agents/state/zedflow-ai-agent-pi-fidelity-current-status.md`.

Implemented R5 fallible/async model source and refresh dedupe.

Changed files:
- crates/zedflow-ai/src/models.rs
- crates/zedflow-ai/src/auth/resolve.rs
- crates/zedflow-ai/tests/models-runtime.rs
- crates/zedflow-ai/tests/providers.rs

Reactivated/added model source/refresh tests:
- models_runtime_swallows_provider_source_failures_for_listing
- models_runtime_refresh_updates_dynamic_providers_and_rejects_single_failures
- models_runtime_dedupes_concurrent_provider_refreshes
- create_provider_supports_dynamic_providers_empty_until_refreshed_in_flight_refreshes_deduped
- models_runtime_refreshes_expired_oauth_credentials_and_persists_rotation
- models_runtime_rejects_with_oauth_code_and_preserves_stored_credential
- models_runtime_serializes_concurrent_oauth_refreshes_through_store_modify
- models_runtime_valid_oauth_tokens_resolve_without_touching_modify

Remaining sync API compatibility wrappers:
- Models::refresh wraps refresh_async with block_on to preserve existing Rust callers/tests while core refresh is async.
- Models::get_auth wraps get_auth_async with block_on; stream auth paths still use sync wrappers from existing public stream surface.
- Provider::get_models returns an empty Vec on source errors for existing Rust call sites; get_models_result exposes the fallible Result source.

Validation commands/results:
- cargo test -p zedflow-ai --test models-runtime --test providers --no-run: passed
- cargo test -p zedflow-ai --test models-runtime --test providers: passed (26 passed, 4 ignored)
- cargo test -p zedflow-ai --test images-models: passed (4 passed, 5 ignored)
- cargo fmt --all --check: passed

Residual risks:
- Live provider transports intentionally not implemented.
- Full image registry parity remains R8; images-models was only validated because no shared helper change was needed.
- Working tree has many pre-existing dirty/untracked files from prior units; no files are staged.
