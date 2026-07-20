# Port coordination decisions

- Use one persistent coordinator Pi session and one persistent worker Pi session.
- A session may use subagents; a DAG task does not create a new top-level Pi process.
- pi-intercom carries assignments, decisions, progress, and completion messages. It is not the source of truth.
- One writer is active. The coordinator may validate directly or through a subagent after the worker becomes idle.
- Git ancestry, owned diffs, and declared commands are the acceptance evidence.
- Independent fidelity and Rust reviews happen at wave boundaries, not after every micro-task.
- The Pi gitlink remains frozen at the value in the DAG.
- `AG-T0` owns mechanical compilation propagation for the three fixtures broken by AG-C1; `AG-T1` retains later semantic parity closure.
- `AG-P1` through `AG-P4` depend independently on `AG-H4`; the single-writer runtime serializes them without inventing false dependency edges.
- Paseo scheduling is paused during main-baseline reconciliation; resume only after a successful manual coordinator/worker pilot against the current DAG and state.
