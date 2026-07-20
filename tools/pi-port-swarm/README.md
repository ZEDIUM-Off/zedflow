# Pi port controller

`controller.py` executes the stage-1 Pi TypeScript → Rust DAG with one fresh Pi session and worktree per unit. It is event-driven: `--continuous` immediately selects the next unit after acceptance. It never installs or calls cron, Paseo, systemd, or a daemon.

```bash
python3 tools/pi-port-swarm/controller.py validate
python3 tools/pi-port-swarm/controller.py status
python3 tools/pi-port-swarm/controller.py monitor
python3 tools/pi-port-swarm/controller.py run [--unit ID] [--continuous]
```

Runtime state, logs, sessions, and worktrees live under `$XDG_STATE_HOME/zedflow-pi-port`; the repository retains only the DAG and its seed state. The integration head is `refs/heads/automation/pi-port`, never the checked-out `main` ref. `run` accepts one ready unit by default. Failed worktrees and sessions are retained for inspection; retries are manual.

A worker can return `PLAN_CHANGE`, which alone starts a fresh coordinator permitted to modify only DAG/state/docs control files. The controller validates the plan, frozen Pi gitlink, owned diff, declared commands, ancestry, and CAS before accepting any candidate.

A monitoring timer may call `monitor` only. It must be read-only and must not call `run`.
