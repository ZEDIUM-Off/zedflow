# TUI parity oracle scaffold

This directory defines the deterministic JSON protocol used to compare the frozen Pi TUI with Zedflow. It does not contain final parity fixtures; P7.T1 adds those.

## Prerequisites

- Python 3.11 or newer
- Node.js satisfying `references/pi/package.json` (currently `>=22.19.0`)
- npm
- Cargo for the Rust consumer check

No pnpm installation or local `node_modules` is used. `--prepare` copies the frozen `references/pi` tree to a temporary directory and runs `npm ci --ignore-scripts` against its tracked `package-lock.json`; npm may use its normal configured registry/cache. The temporary directory is removed afterwards.

```bash
python3 tools/tui-parity/run.py --self-check
python3 tools/tui-parity/run.py --prepare
node tools/tui-parity/frozen-pi-oracle.mjs --self-check
cargo test -p zedflow-coding-agent --test tui-parity-rust
```

Missing Node.js or npm produces an actionable error instead of falling back to pnpm or an untracked install.

## Protocol

Fixtures conform to [`fixtures/schema.json`](fixtures/schema.json):

- `version` is `1`.
- `dimensions` fixes terminal columns and rows.
- `capabilities` fixes color depth, Unicode, and Kitty keyboard behavior.
- `events` contains terminal writes, user input, resize, and message/tool/session lifecycle events.

The oracle emits JSON containing a normalized frame after every event, captured inputs, and lifecycle records. Frames preserve visible cells, ANSI styles, cell widths, and cursor position/visibility. Terminal control queries are consumed by the virtual terminal; nondeterministic `timestamp`, `cwd`, `path`, and `query` metadata fields are omitted.

Render a fixture with the frozen Pi oracle:

```bash
python3 tools/tui-parity/run.py path/to/fixture.json > pi-output.json
```

A future Rust oracle can be compared in the same process. Its command reads the fixture from stdin and writes protocol JSON to stdout:

```bash
python3 tools/tui-parity/run.py path/to/fixture.json \
  --rust-command cargo run -p zedflow-coding-agent --example tui-parity-oracle
```

The comparison is structural JSON equality, not screenshots. The current Rust integration test is intentionally only a fixture consumer/output skeleton.
