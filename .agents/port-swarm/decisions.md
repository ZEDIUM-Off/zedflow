# Port coordination decisions

- Use one persistent coordinator Pi session and one persistent worker Pi session.
- A session may use subagents; a DAG task does not create a new top-level Pi process.
- pi-intercom carries assignments, decisions, progress, and completion messages. It is not the source of truth.
- One writer is active. The coordinator may validate directly or through a subagent after the worker becomes idle.
- Git ancestry, owned diffs, and declared commands are the acceptance evidence.
- Independent fidelity and Rust reviews happen at wave boundaries, not after every micro-task.
- The Pi gitlink remains frozen at the value in the DAG.
- Paseo remains paused until the manual two-session pilot succeeds.
