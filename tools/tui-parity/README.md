# Frozen Pi / Rust TUI parity

This is a differential **component** oracle. Fixtures contain only semantic lifecycle events, editor input, and resizes; they cannot carry terminal output or expected render strings. The frozen-Pi side feeds those events to frozen `TUI`, `Editor`, `AssistantMessageComponent`, `ToolExecutionComponent`, and selector components. The Rust side feeds the equivalent events to `Tui`, `CustomEditor`, `StreamingAssistantMessage`, `ToolExecutionComponent`, and `SelectList`. Both component trees render to a virtual terminal frame, then the runner compares structural JSON equality (visible cells/styles/cursor, inputs, and lifecycle metadata).

The bounded seam deliberately does not construct a credentialed `InteractiveMode`: its provider/session services are neither deterministic nor part of a terminal fixture. The oracle drives the corresponding editor, transcript, tool, selector, and TUI state machines directly on both sides. The frame bridge is a deterministic clear/home write of lines returned by those real components; no fixture-provided rendering is replayed. `timestamp`, `cwd`, `path`, and terminal `query` metadata are removed, but visible terminal cells are never normalized away.

## Reproduce

```bash
python3 tools/tui-parity/run.py --self-check
python3 tools/tui-parity/run.py --prepare
python3 tools/tui-parity/run.py --all --artifacts /tmp/zedflow-tui-parity-frames
cargo test -p zedflow-tui --all-targets
cargo test -p zedflow-coding-agent --test tui-parity-rust
cargo test -p zedflow-coding-agent --test interactive-pty-parity
cargo test -p zedflow-coding-agent --test interactive-terminal-restoration
```

The runner copies `references/pi` to a temporary directory and runs `npm ci --offline --ignore-scripts` using its tracked `package-lock.json`; it uses the installed tracked `tsx` to execute frozen TypeScript sources. It never uses pnpm, repository `node_modules`, credentials, screenshots, or the network. Missing Node/npm/Cargo fails with an actionable diagnostic.

A successful comparison prints `<fixture>: equal`. A mismatch prints both complete frames and exits nonzero; it is evidence of a visible parity defect, not a snapshot to bless. `--artifacts` writes equal normalized frames only.

## Fixtures

- `input-editing.json`: cursor input, bracketed paste, history navigation.
- `streaming.json`: assistant message start/update/end; update content is visible.
- `tools-compaction.json`: actual tool start/update/end and compaction state.
- `commands.json`: slash inputs are routed to selector/compaction state rather than editor text.
- `overlays.json`: selector input and overlay composition.
- `unicode-resize.json`: CJK, emoji, combining text, and resize.
- `abort-error.json`: assistant abort/error state and cursor-bearing editor restoration.
