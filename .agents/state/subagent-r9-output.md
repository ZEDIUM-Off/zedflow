<!-- migration-document-status: COMPLETED ARTIFACT -->
> [!NOTE]
> **Migration status: COMPLETED ARTIFACT.** Raw historical evidence only; do not use it as current progress. See `.agents/state/zedflow-ai-agent-pi-fidelity-current-status.md`.

Implemented R9 OpenRouter image live transport.

Changed files:
- crates/zedflow-ai/src/api/openrouter-images.rs
- crates/zedflow-ai/src/providers/openrouter-images.rs
- crates/zedflow-ai/tests/images.rs
- crates/zedflow-ai/tests/provider-error-body-passthrough.rs (pre-existing dirty file; read/validated, not newly edited in this pass)

Live report:
- OpenRouter capability: available via ~/.pi/agent/auth.json api_key entry; no secret values printed.
- OpenRouter live image tests: executed and passed once (`cargo test -p zedflow-ai --test images -- --nocapture`: 3 passed, 6.77s).
- Latest rerun of the same test command is blocked before test execution by an unrelated compile error in `crates/zedflow-ai/src/api/openai-completions.rs:712` (`run_openai_completions_live_worker` not found), outside R9 scope.

Redaction confirmation:
- API key is only applied via bearer auth, never formatted into diagnostics.
- `ImagesOptions` debug output redacts `api_key` and secret headers.
- Header validation/client/response errors say headers are redacted and include header names only, not values.
- Added deterministic redaction assertions.

Targeted validation commands/results:
- `cargo fmt --all --check`: passed.
- `cargo test -p zedflow-ai --lib openrouter_images --no-run`: passed before unrelated OpenAI completions compile drift appeared.
- `cargo test -p zedflow-ai --lib openrouter_images`: passed before unrelated drift (12 passed, 377 filtered out).
- `cargo test -p zedflow-ai --test provider-error-body-passthrough`: passed before unrelated drift (1 passed).
- `cargo test -p zedflow-ai --test images -- --nocapture`: passed before unrelated drift (3 passed, OpenRouter credential available).
- Latest rerun of targeted cargo tests: failed due unrelated `openai-completions.rs` missing function, not R9 files.
- `git diff --cached --name-only`: empty; no staged files.

Residual risks:
- Current workspace contains unrelated dirty files and a non-R9 OpenAI completions compile error that now blocks rerunning Rust tests.
- OpenRouter live tests depend on provider/network availability and may be slow/flaky by nature.
