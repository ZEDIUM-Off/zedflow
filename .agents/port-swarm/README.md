# Persistent Pi port coordination

The port uses two long-lived Pi sessions: `zedflow-port-coordinator` and `zedflow-port-worker`. They communicate through pi-intercom; either session may use subagents internally, but only the worker edits product code and only one writer is active.

`tools/pi-port-swarm/dag.json` defines dependencies, ownership, and validation. `state.json` contains only closed units and the current unit; `HEAD` is the integration head. Git and runnable checks prove completion; intercom is transport, not durable state.

Assignments and completion reports use `send`. `ask`/`reply` is reserved for blocking decisions. There are no acceptance reports, child-run IDs, lifecycle-artifact checks, external state machine, or one-shot Pi process per task.

Recovery refs retained in the repository:

- `refs/archive/pi-port-v1/source-worktree`
- `refs/archive/pi-port-v1/source-bootstrap`
- `refs/archive/pi-port-v1/integration`
- `refs/archive/pi-port-v1/ag-l2-candidate`

## Scheduling

Hourly coordinator ticks are delivered by persistent Paseo relay `dbe0e650` through active schedule `f7c25e49`. Legacy schedule `6f738382` is paused. This relay is transport only; the DAG, Git, and runnable checks remain authoritative.
