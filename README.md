# Zedflow

Zedflow is a graph-native coding-agent harness developed in two ordered stages:

1. **Current:** port the Pi TypeScript runtime completely and faithfully into matching Rust crates under `crates/`.
2. **After fidelity is proven:** build the Zedflow Flow/Runtime Graph product with LangGraph.

Stage 2 does not begin until the stage-1 port and its deterministic tests are complete. See [`docs/porting/BASELINE.md`](docs/porting/BASELINE.md) for the current baseline.

## Current references

- Product context: [`CONTEXT.md`](CONTEXT.md)
- Migration intent: [`docs/planning/ZEDFLOW_MIGRATION_INTENT.md`](docs/planning/ZEDFLOW_MIGRATION_INTENT.md)
- MVP PRD: [`docs/planning/ZEDFLOW_MVP_PRD.md`](docs/planning/ZEDFLOW_MVP_PRD.md)
- Stage-1 porting rules: [`docs/planning/PI_RUST_PORTING_RULES.md`](docs/planning/PI_RUST_PORTING_RULES.md)
- Pi TypeScript reference: `references/pi`
- LangGraph reference: `references/langgraph`

Initialize submodules after cloning:

```bash
git submodule update --init --recursive
```

## Development

```bash
cargo fmt --all --check
cargo check --workspace --all-targets
cargo test --workspace --all-targets --no-run
```

A local CocoIndex inventory helper lives in `tools/zedflow-index/`:

```bash
cd tools/zedflow-index
uv run cocoindex update main.py --full-reprocess
```

Generated inventory: `tools/zedflow-index/out/inventory.md`.
