# Authentication and model-response provenance

**Scope.** Provider/model identity from request setup through assistant error/render/footer state, plus `/login` and `/logout`. Baseline is Pi `2b00dade` and Rust `913488c2`.

## Tests/source evidence

- Pi `packages/coding-agent/src/modes/interactive/interactive-mode.ts` defines `isUnknownModel` as exactly provider/id/api `unknown`; `completeProviderAuthentication` refreshes the registry and, only from that sentinel, verifies a provider default, availability, and `session.setModel`, reporting each failure.
- Rust `crates/zedflow-ai/src/models.rs` makes unknown provider dispatch an `AssistantMessageEvent::Error` containing `Unknown provider: <id>` and preserves the supplied model in `assistant_message`. Its focused tests include `tests/models-runtime.rs` unknown-provider stream coverage.
- Rust `crates/zedflow-coding-agent/src/modes/interactive/interactive-mode.rs` mounts `LiveSelector::Auth` after `/login`, but `SelectorOverlay::select` has an `Auth` arm only when `mode == Logout`; login selection falls through. `SelectorAction` has no login-provider completion path. This is a source-proven functional failure, not merely a styling difference.
- The saved real-PTY lane (`c0fa6b31`) found `/login` frame differences. It did not authenticate or send credentials, so it does not establish secret-storage parity.

## Matching behavior

Both trees have provider auth storage/selector types and separate unknown-provider error handling. Both expose login/logout commands and filter known built-in provider catalogues. Rust logout selection calls `AuthStorage::logout`; Pi logout similarly operates on stored providers.

## Mismatches

- **P0 — login selection is nonfunctional.** Entering a Rust login provider selector has no action that initiates OAuth/API-key entry or completion. Do not treat its rendered list as a safe login implementation.
- **P0 — provenance is not proven end-to-end.** The Rust AI layer emits an unknown-provider error, but there is no actual-CLI fixture proving that provider/model identity survives request failure into transcript, footer, and retry/selection state without being replaced by a misleading identity.
- **P1 — post-auth selection contract absent.** Pi conditionally selects a checked default model only from its unknown sentinel and reports no-default/no-model/set-model failures. No equivalent Rust interactive completion path was found.
- **P2 — logout presentation/state parity is unproven.** Rust reload ordering and provider labels differ in source shape; no differential fixture covers cancel, selected provider, stored API key, or environment-only credential cases.
- **P3 — credential UX parity unknown.** The audit did not run an OAuth browser/device flow or type an API key. Never claim storage, redaction, cancellation, or error parity from the selector code alone.

## Missing fidelity fixtures

A bespoke offline raw-PTY fixture must dispatch both CLIs through `/login`, choose subscription and API-key branches, cancel at each boundary, and assert no credential is printed. A deterministic test-only auth/provider adapter is required before completion cases can assert: unknown sentinel → successful allowed default selection; missing default; unavailable default; selection error; unknown provider request error. It must inspect transcript/footer/model state and credential file permissions/redaction without network access. These are fidelity tests, separate from copied Pi unit tests.

## Fix boundary

Own only `zedflow-ai` model/error provenance, coding-agent interactive auth completion, auth storage integration, and the bespoke PTY fixtures. Do not change provider policy, invent default models, or make a real network login a test prerequisite. The frozen Pi sentinel/default behavior is the contract; unresolved provider-policy differences remain explicit unknowns.
