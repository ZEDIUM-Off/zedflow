# Coding-agent foundation acceptance and second wave

## Accepted foundation

At base `a0e89189a5258ff2c3b4529cd0058fda24e1ebb1`, `CA-RV-FID-R12` accepted the first coding-agent wave after the final grep numeric-context repair. The accepted foundation now covers the dependency-light utilities, shared tool primitives, image reads, and read/write/find/ls/grep/edit filesystem tools. The worktree and frozen `references/pi` gitlink were clean at checkpoint start.

## Next bounded wave

The next wave stays inside `zedflow-coding-agent` and ports only two dependency-light groups needed before session runtime:

1. custom-message conversion plus deterministic compaction serialization, token estimation, and cut-point utilities;
2. the event bus, provider display-name mapping, session-CWD resolution, and timing helpers.

A workspace validator and fresh Pi-fidelity reviewer gate the next checkpoint. Model-driven compaction, session management, extensions, package management, RPC, CLI, and interactive TUI remain unassigned.
