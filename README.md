# Zedflow

Zedflow is a graph-native coding-agent harness.

It uses explicit Flow and Runtime Graph composition. LangGraph is the reference graph runtime; Rust remains the product runtime and execution gateway.

## Current references

- Product context: [`CONTEXT.md`](CONTEXT.md)
- Migration intent: [`docs/planning/ZEDFLOW_MIGRATION_INTENT.md`](docs/planning/ZEDFLOW_MIGRATION_INTENT.md)
- MVP PRD: [`docs/planning/ZEDFLOW_MVP_PRD.md`](docs/planning/ZEDFLOW_MVP_PRD.md)
- Workspace architecture: [`docs/planning/ZEDFLOW_WORKSPACE_ARCHITECTURE.md`](docs/planning/ZEDFLOW_WORKSPACE_ARCHITECTURE.md)
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
