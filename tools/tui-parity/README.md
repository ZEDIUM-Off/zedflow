# Frozen Pi / Rust TUI parity

This is a differential **component** oracle. Fixtures contain only semantic lifecycle events, editor input, and resizes; they cannot carry terminal output or expected render strings. The Rust side mounts an actual `InteractiveMode` on a deterministic in-memory `Terminal`: raw input is queued as terminal events and drained through `pump_events`, submissions run through its mounted `CustomEditor` into `queue_user_input` and built-in dispatch, and frames come from `render_current_frame`, which renders the actual TUI root and native overlay stack. The test explicitly asserts that `/model` and `/compact` do not leak into the mounted editor.

The frozen-Pi side cannot construct an `InteractiveMode` from a deterministic public session fixture: its constructor owns a concrete `ProcessTerminal` and requires a fully initialized runtime host/resource/session graph. Its bounded seam is therefore frozen `CustomEditor` submission plus the exact built-in command switch in `InteractiveMode.setupEditorSubmitHandler`, with real frozen selector components mounted on frozen `TUI`; it does not special-case slash strings in the fixture event loop. Lifecycle fixtures continue to use their real frozen transcript/tool components. This is intentionally narrower than Rust's complete root: visible footer/status differences are parity findings, never normalized away. `timestamp`, `cwd`, `path`, and terminal `query` metadata are removed.

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
