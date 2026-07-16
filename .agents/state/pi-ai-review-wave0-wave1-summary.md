<!-- migration-document-status: SUPERSEDED / HISTORICAL -->
> [!CAUTION]
> **Migration status: SUPERSEDED / HISTORICAL.** Do not use this file as the current execution tracker. See `.agents/state/zedflow-ai-agent-pi-fidelity-current-status.md`.

# pi-ai / zedflow-ai review — wave 0/1 start

## Started

Read-only subagents launched for:

- shared types/imports/export drift
- manifest/lib.rs coverage
- deterministic utils parity
- event-stream parity

Only the manifest/lib.rs reviewer completed before timeout. The other three were too broad for one pass and should be relaunched in narrower slices.

## Completed finding: manifest coverage

- `.agents/port-manifests/ai-src.tsv` has 148 rows.
- All 148 Pi source files exist.
- All 148 Rust targets exist.
- No extra Rust target files except `crates/zedflow-ai/src/lib.rs`.
- Placeholder marker coverage was OK for direct stub files.

### Fixed immediately

`crates/zedflow-ai/src/lib.rs` was missing public module declarations for 17 manifest targets. Added module declarations for:

- Cerebras
- Cloudflare AI Gateway / Workers AI / auth
- DeepSeek
- Google / Google Vertex
- Groq
- Hugging Face
- `providers/images/register-builtins.rs`

## Catalog generation work

The existing Python generators were not removed, but Makefile now uses Rust binaries:

- `crates/zedflow-ai/src/bin/generate-models.rs`
- `crates/zedflow-ai/src/bin/generate-image-models.rs`

Make targets:

- `make generate-models`
- `make generate-image-models`
- `make generate`
- `make fmt`
- `make check`
- `make test`
- `make doc`
- `make package`

The image generator now emits `f64` literals (`0.0`, `2.0`, etc.) so `cargo check -p zedflow-ai --lib` passes.

## Validation

Passed:

```bash
CARGO_TARGET_DIR=/tmp/zedflow-target TMPDIR=/tmp/zedflow-tmp cargo run -p zedflow-ai --bin generate-models
CARGO_TARGET_DIR=/tmp/zedflow-target TMPDIR=/tmp/zedflow-tmp cargo run -p zedflow-ai --bin generate-image-models
cargo fmt --package zedflow-ai
CARGO_TARGET_DIR=/tmp/zedflow-target TMPDIR=/tmp/zedflow-tmp cargo check -p zedflow-ai --lib
```

Still failing:

```bash
CARGO_TARGET_DIR=/tmp/zedflow-target TMPDIR=/tmp/zedflow-tmp cargo check -p zedflow-ai --all-targets
```

Known failures are test/module issues already observed by the review:

- `Provider` does not implement `Debug`, so `expect_err` fails in provider tests.
- Some tests match `zedflow_core::error::Error` without wildcard `Err(_)` despite non-exhaustive error enum.
- Test dead-code warnings remain.

## Next lazy relaunch

Relaunch narrower read-only reviews instead of broad timeouts:

1. `types.rs` only against `types.ts`.
2. Entrypoints only: `index`, `compat`, `legacy-api-aliases`, `lib.rs`.
3. Utils in 3 chunks:
   - parsing/validation/error-body/json
   - headers/retry/provider-env/proxy/abort
   - estimate/hash/overflow/sanitize/diagnostics
4. Event-stream only plus direct consumers grep.
