# Persistent Pi TypeScript → Rust port coordination

The swarm automates **stage 1**, the faithful port of frozen Pi TypeScript packages into matching Rust crates. It must not implement stage-2 Zedflow/LangGraph behavior.

The port is coordinated by two long-lived Pi sessions rather than one `pi -p` process per DAG unit. `zedflow-port-coordinator` owns task selection and acceptance; `zedflow-port-worker` is the sole product-code writer. Both may use subagents inside their existing sessions.

The sessions communicate through pi-intercom. Assignments and completion reports use `send`; blocking decisions use `ask`/`reply`. Durable truth remains the Git branch, `tools/pi-port-swarm/dag.json`, `.agents/port-swarm/state.json`, and runnable validation commands.

## Current status

Automated Paseo scheduling is paused during main-baseline reconciliation. Resume it only after a manual coordinator/worker pilot succeeds against the current DAG and state. A restored schedule must target the persistent coordinator rather than create a new process per tick.

## Stage-1 completion

The swarm is finished only when the exit gate in `docs/porting/BASELINE.md` passes on one recorded SHA. Closing the current DAG wave is not equivalent to completing the Pi port.
