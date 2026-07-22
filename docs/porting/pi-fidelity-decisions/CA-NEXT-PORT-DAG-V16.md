# Coding-agent utility-substrate wave after the bounded TUI pure-primitives wave

## Accepted TUI wave

At base `dd95fc61b39a0b1619977a1f1836f06e5f5f2308`, `TUI-RV-FID-V1-R6` accepted the bounded TUI pure-primitives wave after the OSC11 parsing, channel-scaling, and hex-safety repairs. The worktree and frozen `references/pi` gitlink were clean at checkpoint start.

The global Pi port remains incomplete: coding-agent, TUI, and orchestrator still have unported manifest rows. This checkpoint changes only the port plan and state; it does not alter product Rust.

## Next bounded wave

The next wave returns to `zedflow-coding-agent` and ports only the dependency-light utility substrate: path normalization/resolution, child-process helpers, Git URL parsing, and shell quoting/defaults from frozen Pi. It must not begin CLI startup, package/resource loading, extensions, settings/session runtime, interactive TUI, RPC, or orchestrator work.

A workspace validator and fresh Pi-fidelity reviewer gate the wave. Remaining coding-agent work stays ahead of orchestrator. Every non-final wave requires a successor `NEXT-PORT-DAG` checkpoint; completion still requires every manifest row plus final workspace validation and fidelity/Rust reviews.
