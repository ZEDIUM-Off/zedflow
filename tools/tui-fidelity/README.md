# Real end-user Pi ↔ Zedflow TUI fidelity

This suite is separate from `tools/tui-parity`: it starts the actual frozen Pi CLI and the actual Rust CLI in isolated raw PTYs, sends the same semantic raw-input/resize/checkpoint fixture to both, and decodes both captures with the same `@xterm/headless` decoder. It compares every terminal cell (`text`, width, foreground/background, bold/dim/italic/underline/inverse/strikethrough) and cursor position.

It does **not** call component renderers, build selector trees, invoke prototypes, inject lifecycle events, or accept snapshots as expected output. It copies Pi from the exact tracked object `2b00dade7cec918aefb025c8b7a4fa304a30acdd`, so the known dirty `packages/coding-agent/docs/index.md` is excluded.

Run (requires already cached npm packages; runner never uses network):

```bash
export CARGO_TARGET_DIR=/tmp/zedflow-target CARGO_NET_OFFLINE=true
python3 tools/tui-fidelity/run.py --all --artifacts /tmp/zedflow-tui-fidelity
```

`pi.raw`, `rust.raw`, decoded `pi.json`, `zedflow.json`, and `diff.txt` are written per fixture. A `DIFFER` is a real compatibility finding, not a baseline to bless. Initial scenarios cover full `/settings` and `/login`'s auth method then provider list.

The command exits nonzero on any visible difference and is intended to become the required fidelity gate. The initial `/settings` and `/login` scenarios currently fail, deliberately recording the remaining layout/style/catalog gaps rather than blessing snapshots. Add only deterministic offline flows with no credential or provider network request.
