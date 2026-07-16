<!-- migration-document-status: COMPLETED ARTIFACT -->
> [!NOTE]
> **Migration status: COMPLETED ARTIFACT.** Raw historical evidence only; do not use it as current progress. See `.agents/state/zedflow-ai-agent-pi-fidelity-current-status.md`.

Implemented R1: public `types::AssistantMessageEventStream` now re-exports the real `utils::event_stream::AssistantMessageEventStream`, and provider-free stream contract tests cover Pi event JSON names, event order, done/error `result()`, and aborted partial preservation.

Changed public stream type paths:
- `zedflow_ai::types::AssistantMessageEventStream` -> re-export of `zedflow_ai::utils::event_stream::AssistantMessageEventStream`
- `zedflow_ai::utils::event_stream::AssistantMessageEventStream` remains the concrete real stream implementation

Changed files:
- `crates/zedflow-ai/src/types.rs`
- `crates/zedflow-ai/tests/stream-events.rs`

Reactivated/added stream contract tests:
- `assistant_event_json_names_match_pi`
- `assistant_stream_preserves_iteration_order_and_done_result`
- `assistant_stream_error_result_returns_terminal_error_message`
- `assistant_stream_aborted_error_result_preserves_partial_message`

Validation:
- `cargo fmt --all --check` passed
- `cargo test -p zedflow-ai --test stream-events --no-run` passed
- `cargo test -p zedflow-ai --test stream-events` passed: 4 passed
- `cargo test -p zedflow-ai --test stream` passed: 1 passed, 1 ignored
- `cargo test -p zedflow-ai utils::event_stream` passed: 3 passed, 784 filtered out

Legacy/minimal stream callers remaining for R2/follow-ups:
- `crates/zedflow-ai/src/models.rs` still defines local minimal `Model`, `StreamOptions`, `AssistantMessage`, and `pub type AssistantMessageEventStream = Vec<AssistantMessage>`; R2 must adapt this to canonical `crate::types`.
- `crates/zedflow-ai/src/api/lazy.rs` / `crates/zedflow-ai/src/providers/faux.rs` still use legacy opaque/minimal lazy streams.
- Provider-local stream placeholders remain in `crates/zedflow-ai/src/api/bedrock-converse-stream.rs` and `crates/zedflow-ai/src/api/openai-responses-shared.rs` for later provider/live transport units.

Notes:
- `.agents/state/port-audit-stream-events.md` was not present; `.agents/state/zedflow-ai-vs-pi-ai-port-audit-summary.md` was read.
- Working tree had many unrelated pre-existing modified/untracked files; no files are staged.
