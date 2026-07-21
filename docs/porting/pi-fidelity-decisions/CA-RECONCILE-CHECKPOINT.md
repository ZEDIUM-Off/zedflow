# Coding-agent reconciliation checkpoint

## Evidence

At base `73696411517b8a41e79ee54e1c831ec76d65da5a`, the frozen source is `references/pi@2b00dade7cec918aefb025c8b7a4fa304a30acdd`; both the worktree and gitlink are clean.

`references/pi/packages/coding-agent/src` contains 171 files and 56,306 TypeScript/JavaScript/CSS/HTML lines, with 192 files under `test`. The target crate contains only `Cargo.toml` and a six-line `src/lib.rs` that exports the crate name. It implements no frozen coding-agent behavior and has no Rust tests or placeholders.

## Proven resume point

The coding-agent port therefore starts at the package boundary, not from a partially ported module. Begin with deterministic, dependency-light utility behavior, then the core filesystem tools. Do not assign session runtime, extensions, package management, RPC, CLI, or interactive TUI until this foundation passes review.

The DAG units following this checkpoint are the first bounded wave. Their tests are limited to the corresponding frozen deterministic tests; dependency/API gaps must use the repository placeholder policy rather than speculative replacements.
