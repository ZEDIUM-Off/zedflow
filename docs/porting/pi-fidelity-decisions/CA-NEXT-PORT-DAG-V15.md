# TUI pure-primitives wave after coding-agent entrypoint discovery

## Accepted coding-agent wave

At base `92ad92d9c3d5407bb96f7e93dd92e451531df5d3`, `CA-RV-FID-V17` accepted the bounded coding-agent entrypoint package-directory discovery wave after workspace validation. The port remains incomplete: coding-agent, tui, and orchestrator still have unported manifest rows. The worktree and frozen `references/pi` gitlink were clean at checkpoint start.

## Next bounded wave

The next wave begins `zedflow-tui` with dependency-light pure primitives from frozen Pi: fuzzy matching/filtering, terminal color response parsing, word navigation, kill-ring behavior, and undo-stack behavior. It must not begin terminal I/O, rendering, components, editor state, image handling, or Kitty key parsing.

A workspace validator and fresh Pi-fidelity reviewer gate the wave. Remaining coding-agent work follows this TUI wave; orchestrator remains after coding-agent. Every non-final wave requires a successor NEXT-PORT-DAG checkpoint, and global completion still requires every manifest row plus final workspace validation and fidelity/Rust reviews.
