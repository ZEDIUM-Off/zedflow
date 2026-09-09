# Zedflow — ADK lab

This branch reboots Zedflow as an ADK-Rust playground. It has no Pi port stage,
Pi fidelity gate, LangGraph runtime, sidecar, or compatibility requirement.

Read `CONTEXT.md` for product intent, `docs/composition.md` for composition concepts,
`docs/resources.md` for state/store scope, and `flows/README.md` before adding experiments.
These documents distinguish future Zedflow concepts from implemented ADK experiments.

- Use ADK-Rust primitives directly. Do not introduce Zedflow contracts, a registry,
  a DSL, a custom executor, or an optimizer unless the task explicitly calls for them.
- Put each workflow in one focused `flows/*.rs` file. Reuse a flow through ADK
  composition when useful; keep its responsibility, inputs, outputs, and effects clear.
- Keep fixture experiments usable without credentials or network calls. Live model
  runs must be explicit. Never commit credentials or runtime databases.
- Keep all ADK library crates in the dependency catalog. See `docs/adk.md` for
  feature groups, the companion CLI, dependency pins, and platform constraints.
- Before Rust changes, load the global `rust-skills` skill and relevant rule files.
  Use Cargo for the Rust workflow. Prefer direct, small implementations.
- Default branch is `main`. Preserve unrelated working changes. Do not run destructive
  Git commands or remove unrelated files without explicit user authorization.

Use an external target directory for builds, for example
`CARGO_TARGET_DIR=/tmp/zedflow-adk-target`. Validate changes with:

```sh
cargo fmt --all --check
cargo check --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
```

When dependencies/features change, also run
`cargo check --locked --workspace --all-targets --all-features`.
The full catalog includes a CPU local-inference build and is more expensive than the
default graph/agent lab. Add tests for actual behavioral boundaries, not for declarations.
