<!-- migration-document-status: COMPLETED ARTIFACT -->
> [!NOTE]
> **Migration status: COMPLETED ARTIFACT.** Raw historical evidence only; do not use it as current progress. See `.agents/state/zedflow-ai-agent-pi-fidelity-current-status.md`.

Implemented R4: Models auth now routes through auth::resolve::resolve_provider_auth, provider auth handlers are attached from provider metadata, and resolved auth is merged into stream/simple stream request model/options before provider dispatch.

Changed files:
- crates/zedflow-ai/src/auth/resolve.rs
- crates/zedflow-ai/src/auth/types.rs
- crates/zedflow-ai/src/models.rs
- crates/zedflow-ai/src/providers/static_catalog.rs
- crates/zedflow-ai/src/providers/cloudflare-auth.rs
- crates/zedflow-ai/tests/models-runtime.rs
- crates/zedflow-ai/tests/providers.rs

Hardcoded auth branches removed/retained:
- Removed from Models::get_auth: stored api-key branch, provider-id OAuth match for anthropic/github-copilot/openai-codex, ambient provider match for anthropic/bedrock/cloudflare/google-vertex.
- Removed from stream paths: direct provider dispatch without auth application; stream and stream_simple now apply resolved auth first.
- Retained: provider-id map in static_catalog to attach provider auth handlers at provider construction; this is metadata wiring, not request-time auth resolution.
- Retained: sync block_on compatibility wrappers around async resolver calls because the current public Models stream/get_auth surface is still sync; R5 should remove/reshape this with async runtime surface.

Auth tests reactivated/updated:
- Reactivated models_runtime_uses_explicit_request_api_key_and_env_during_provider_auth_resolution.
- Reactivated models_runtime_merges_resolved_auth_into_stream_options_with_explicit_fields_winning.
- Updated provider auth tests to use auth::types ModelAuth/AuthContext bridge.

Remaining async refresh blockers for R5:
- Models::get_auth and stream auth application still expose sync wrappers over async auth resolution.
- OAuth refresh still relies on store modify serialization only; no in-flight async refresh dedupe beyond that.
- Deterministic expired-refresh/error tests remain ignored because fake injectable refresh/dedupe is R5 scope.

Validation:
- cargo fmt --all --check: passed.
- cargo test -p zedflow-ai --test oauth-auth --test models-runtime --test providers: passed (27 passed, 10 ignored).
- git diff --cached --name-only: no output; no staged files.

Residual risks:
- Working tree had many pre-existing dirty/untracked files from prior units; changed-file list above is limited to R4-owned edits.
- static_catalog.rs is currently untracked in this worktree but was a dependency from R3; R4 edited it to attach auth handlers.
