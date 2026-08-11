# Frozen Pi / Rust TUI parity

This directory compares the same deterministic terminal event stream in the frozen Pi terminal oracle and Zedflow's Rust terminal model. Comparison is structural JSON equality: visible cells (including wide-cell continuations), cursor position/visibility, input bytes, and lifecycle payloads. Only nondeterministic `timestamp`, `cwd`, `path`, and `query` fields are removed.

## Toolchain used for the acceptance run

- Node.js `v24.16.0` (the frozen package requires `>=22.19.0`)
- npm `11.13.0`
- Cargo `1.96.1`
- Python 3.11 or newer

The runner copies `references/pi` to a temporary directory and executes `npm ci --offline --ignore-scripts` against its tracked `package-lock.json`. It never uses pnpm, repository `node_modules`, credentials, or the network. Populate npm's cache separately if the frozen packages are not already cached.

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

A single fixture can be compared with:

```bash
python3 tools/tui-parity/run.py tools/tui-parity/fixtures/streaming.json
```

Every successful comparison prints `<fixture>: equal`; `--artifacts` writes that shared normalized frame as JSON. A mismatch prints both complete frames and exits nonzero—there is no snapshot blessing or visible-frame normalization.

## Acceptance fixtures

- `input-editing.json`: key, paste, history, and submitted editor bytes.
- `streaming.json`: message start/update/end; update content remains observable.
- `tools-compaction.json`: partial/final tool output and compaction lifecycle.
- `commands.json`: slash-command bytes and their selector/action lifecycle (not prompt routing).
- `overlays.json`: overlay cursor hiding, selection input, line clearing, and restoration.
- `unicode-resize.json`: CJK, emoji, combining characters, ANSI clear/home, and resize.
- `abort-error.json`: abort/error frames and cursor restoration.

Fixtures conform to [`fixtures/schema.json`](fixtures/schema.json). The frozen side uses the lockfile's `@xterm/headless`, matching Pi's terminal cell semantics. The Rust side is compiled from `tui-parity-rust.rs`; the runner invokes its ignored stdin/stdout oracle entry directly. PTY tests separately prove bracketed-paste bytes and termios restoration on `/exit`, Ctrl-C, and argument-error paths.
