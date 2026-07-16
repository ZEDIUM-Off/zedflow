<!-- migration-document-status: COMPLETED ARTIFACT -->
> [!NOTE]
> **Migration status: COMPLETED ARTIFACT.** Raw historical evidence only; do not use it as current progress. See `.agents/state/zedflow-ai-agent-pi-fidelity-current-status.md`.

Implemented R6 compat builtin dispatch and option forwarding.

Changed files:
- crates/zedflow-ai/src/compat.rs

Compat paths now forwarding options:
- compat::stream for custom/override providers forwards caller StreamOptions after Pi-style env API-key injection.
- compat::complete uses compat::stream, so it forwards the same StreamOptions.
- compat::stream_simple for custom/override providers forwards caller SimpleStreamOptions after Pi-style env API-key injection into options.stream.
- compat::complete_simple uses compat::stream_simple, so it forwards the same SimpleStreamOptions.
- register_faux_provider keeps stream options forwarding to FauxCore and simple options forwarding through options.stream.
- builtin API registry wrappers now preserve option presence when bridging into legacy lazy ProviderStreams.
- builtin short-circuit maps compat StreamOptions/SimpleStreamOptions into Models options for auth/env/header/session/cache/metadata, thinking, and payload/response hooks.

Builtin short-circuit behavior and tests:
- Added should_use_builtin_models equivalent: a model short-circuits only when it exists in builtin Models with the same API and the current registry provider is the original builtin provider instance.
- Default builtin registry now dispatches builtin catalog models through compat_models().stream/stream_simple.
- Public register_api_provider ensures builtins are registered first so later overrides are not mistaken for builtin instances.
- Registering an override for a builtin API disables the short-circuit and forwards options to the override.
- Added unit coverage for builtin short-circuit, override behavior, builtin wrapper option presence, and complete_simple option forwarding.

Validation commands/results:
- cargo fmt --all --check: passed
- cargo test -p zedflow-ai --lib compat --no-run: passed
- cargo test -p zedflow-ai --lib compat: passed (13 passed, 374 filtered)
- cargo test -p zedflow-ai --test providers --test models-runtime --no-run: passed
- cargo test -p zedflow-ai --test providers --test models-runtime: passed (26 passed, 4 ignored)
- cargo test -p zedflow-ai --test faux-provider: passed (11 passed, 5 ignored)
- cargo test -p zedflow-ai compat --no-run: failed because pre-existing crates/zedflow-ai/tests/scratch.rs compile errors are outside the targeted R6 scope.

Residual risks:
- Live transports were not implemented.
- Faux accounting was not implemented; R7 owns that.
- The legacy lazy ProviderStreams option structs are empty, so builtin lazy wrappers preserve option presence only; builtin catalog models use the Models short-circuit for real option forwarding.
- Working tree had extensive pre-existing dirty/untracked files from prior units. This run intentionally edited only compat.rs. No files are staged.
