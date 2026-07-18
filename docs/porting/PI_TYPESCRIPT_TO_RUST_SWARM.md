# Persistent Pi TypeScript → Rust port coordination

The port is coordinated by two long-lived Pi sessions rather than one `pi -p` process per DAG unit. `zedflow-port-coordinator` owns task selection and acceptance; `zedflow-port-worker` is the sole product-code writer. Both may use subagents inside their existing sessions.

The sessions communicate directly through pi-intercom. Assignments and completion reports are fire-and-forget `send` messages; blocking decisions use `ask`/`reply`. The durable truth remains the Git branch, `tools/pi-port-swarm/dag.json`, `.agents/port-swarm/state.json`, and runnable validation commands.

The first unit is `RECONCILE-CHECKPOINT`. It compares the preserved source snapshot with the archived integration and AG-L2 candidate refs, then records the actual proven resume point. Historical candidates are not cherry-picked wholesale.

Paseo is paused until a manual coordinator/worker pilot completes successfully. If scheduling is restored, it must target the existing coordinator agent rather than create a new agent for each tick.
