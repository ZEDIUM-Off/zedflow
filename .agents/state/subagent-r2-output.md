<!-- migration-document-status: COMPLETED ARTIFACT -->
> [!NOTE]
> **Migration status: COMPLETED ARTIFACT.** Raw historical evidence only; do not use it as current progress. See `.agents/state/zedflow-ai-agent-pi-fidelity-current-status.md`.

Implemented R2: `models.rs` now re-exports canonical `crate::types` Model/StreamOptions/AssistantMessage/AssistantMessageEventStream/Context and uses the real event stream result path.

Removed duplicate public types and replacements:
- `zedflow_ai::models::Api` local alias -> `zedflow_ai::types::Api`
- `zedflow_ai::models::Model` local minimal struct -> `zedflow_ai::types::Model`
- `zedflow_ai::models::StreamOptions` local minimal struct -> `zedflow_ai::types::StreamOptions`
- `zedflow_ai::models::AssistantMessage` local text-only struct -> `zedflow_ai::types::AssistantMessage`
- `zedflow_ai::models::AssistantMessageEventStream = Vec<_>` -> `zedflow_ai::types::AssistantMessageEventStream`

Changed files:
- crates/zedflow-ai/src/models.rs
- crates/zedflow-ai/src/types.rs
- crates/zedflow-ai/src/providers/static_catalog.rs
- crates/zedflow-ai/src/providers/amazon-bedrock.rs
- crates/zedflow-ai/src/providers/ant-ling.rs
- crates/zedflow-ai/src/providers/faux.rs
- crates/zedflow-ai/src/providers/*.models.rs entries needing `..Model::default()` for canonical fields
- crates/zedflow-ai/tests/models-runtime.rs
- crates/zedflow-ai/tests/providers.rs

Validation:
- `cargo fmt --all --check` passed
- `cargo test -p zedflow-ai --test models-runtime --test providers --no-run` passed
- `cargo test -p zedflow-ai --test models-runtime --test providers` passed: 15 passed, 14 ignored

Remaining blockers assigned to later units:
- R3: full Pi provider contract, per-API dispatch, and provider auth injection/option forwarding.
- R4: runtime auth application/merge through `auth::resolve`.
- R5: async/fallible model source refresh, source failure behavior, and concurrent refresh/OAuth dedupe.

No staged files.
