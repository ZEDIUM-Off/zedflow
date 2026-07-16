<!-- migration-document-status: COMPLETED ARTIFACT -->
> [!NOTE]
> **Migration status: COMPLETED ARTIFACT.** Raw historical evidence only; do not use it as current progress. See `.agents/state/zedflow-ai-agent-pi-fidelity-current-status.md`.

# R13 public API/facade parity cleanup

Implemented R13 only.

Public facade changes:
- `crates/zedflow-ai/src/index.rs` is now the Pi-compatible, side-effect-free root facade instead of a marker module.
- `crates/zedflow-ai/src/lib.rs` re-exports `index::*` at the crate root, so core Pi barrel symbols are available from `zedflow_ai::{...}`.
- The facade explicitly exports canonical Rust/Pi types, model/image registry constructors, auth helpers/types, faux provider helpers, session cleanup helpers, diagnostics/event-stream/json/retry/overflow/typebox/validation helpers, OAuth public types, and selected provider option types.
- Legacy duplicate lazy facade types remain under `api::lazy` instead of being flattened over canonical root types.
- `BedrockGenaiParityReport` and `genai_bedrock_parity_report` were made non-public/test-only to avoid exposing dependency-specific genai concepts.
- Updated ignored `tests/scratch.rs` sample to compile against the current canonical Models API so `--all-targets` works.

Tests added/updated:
- Added `crates/zedflow-ai/tests/public-api.rs` covering root facade imports and documenting Rust handling of JS-only dynamic import observability.
- Updated `crates/zedflow-ai/tests/scratch.rs` compile-only ignored live sample for current APIs.

Genai leak audit result:
- Public genai export grep returned no matches.
- `genai::` references are only in `crates/zedflow-ai/src/utils/genai-backend.rs`.
- `genai_backend` remains declared as `pub(crate)` in `lib.rs` and is not re-exported by the facade.

JS-only cases documented:
- `index.rs` documents Node `registerHooks` dynamic import specifier observability as JS-only/non-portable in Rust.
- `tests/public-api.rs` includes the same invariant for the root facade.
- Existing `tests/lazy-module-load.rs` remains the provider-free side-effect alternative.

Validation commands/results:
- `cargo fmt --all --check` — passed after formatting.
- `cargo check -p zedflow-ai --all-targets` — passed; warnings remain from pre-existing dead-code/unused items and function pointer comparison warnings.
- `cargo test -p zedflow-ai --test public-api` — passed: 2 tests.
- Public API/no-genai audit grep — passed: no public genai exports; only private backend `genai::` references.
- `git diff --cached --name-only` — empty; no staged files.

Residual risks:
- Worktree had extensive pre-existing dirty files from R1-R12; R13-owned files are `src/index.rs`, `src/lib.rs`, `src/api/bedrock-converse-stream.rs` (privacy only), `tests/public-api.rs`, and `tests/scratch.rs`.
- `cargo check --all-targets` emits warnings, but no errors.
