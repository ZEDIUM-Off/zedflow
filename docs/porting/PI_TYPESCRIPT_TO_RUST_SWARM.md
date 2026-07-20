# Pi TypeScript → Rust port controller

The stage-1 port uses `tools/pi-port-swarm/controller.py`. It creates a fresh `pi -p` worker context for one ready DAG unit, verifies the committed result deterministically, then either stops or immediately selects the next ready unit with `--continuous`.

The controller records runtime state outside the repository under `$XDG_STATE_HOME/zedflow-pi-port` and advances only the fixed `refs/heads/automation/pi-port` through null-OID creation/CAS. `main` is never updated behind a checked-out worktree. Interrupted `ACCEPTING` state is reconciled from the ref; failures require explicit `retry --unit`.

Workers receive only an immutable assignment capsule. They cannot edit DAG/state or use persistent intercom coordination. A worker reporting `PLAN_CHANGE` causes one fresh, control-plane-only coordinator to propose a committed DAG/state/docs update; generic failure never silently replans.

There is no scheduled execution. `monitor` emits deterministic, read-only DAG status plus clearly labeled mechanical manifest target-presence counts for an optional external monitoring timer. It cannot launch Pi or mutate Git/state, and its counts are not fidelity completion.

Stage-1 completion remains the exit gate in `BASELINE.md`; closing a DAG wave is not equivalent to completing the Pi port.
