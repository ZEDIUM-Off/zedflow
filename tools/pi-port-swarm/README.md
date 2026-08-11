# Pi port controller

`controller.py` executes the stage-1 Pi TypeScript → Rust DAG with one fresh Pi session and worktree per unit. It is event-driven: `--continuous` immediately selects the next unit after acceptance. It never installs or calls cron, Paseo, systemd, or a daemon.

```bash
python3 tools/pi-port-swarm/controller.py validate
python3 tools/pi-port-swarm/controller.py status
python3 tools/pi-port-swarm/controller.py monitor
python3 tools/pi-port-swarm/controller.py run [--unit ID] [--continuous]
python3 tools/pi-port-swarm/controller.py retry --unit ID      # TRANSIENT only
python3 tools/pi-port-swarm/controller.py repair --unit ID     # bounded REPAIRABLE writer only
python3 tools/pi-port-swarm/controller.py replan --unit ID --reason 'evidence' # PLAN_CHANGE_REQUIRED only
python3 tools/pi-port-swarm/controller.py cleanup [--dry-run] # default, read-only preview
python3 tools/pi-port-swarm/controller.py cleanup --accepted  # remove reviewed accepted attempts only
python3 tools/pi-port-swarm/controller.py upgrade --candidate <sha> --supersede <failed-id> --reason <text> # reviewed control recovery only
```

Runtime state, logs, sessions, and worktrees live under `$XDG_STATE_HOME/zedflow-pi-port`; the repository retains only the DAG and its audit seed state. The seed base is an immutable SHA used only to initialize runtime state. Runtime state version 4 records `controller_sha`, `integration_sha`, `dag_sha`, `plan_sha`, and `pi_gitlink`; it also retains terminal IDs and bounded validation summaries with paths to durable stdout/stderr logs. The integration head is always `refs/heads/automation/pi-port`, never the checked-out `main` ref; it is created by null-OID CAS and never selected by a CLI option. Unit refs use the sibling `automation/pi-port-unit/` namespace. `run` accepts one ready unit by default. A `REPAIRABLE` writer blocker gets at most two attempts in `--continuous` without changing the DAG; structural and arbitration classifications stop safely for a fresh coordinator/human decision.

A worker can return `PLAN_CHANGE_REQUIRED`, which alone starts a fresh coordinator permitted to modify only DAG/state/docs control files. The originating structured result is retained in runtime state and passed intact as coordinator repair context. A blocked coordinator must return its own valid classification so arbitration evidence survives recovery. The coordinator must read the plan-writer skill and reference, remove the terminal source ID, use fresh repair IDs, and leave a ready/reachable frontier. Candidate `validate` is static: it checks that candidate's DAG and Pi gitlink without confusing its product commit SHA with the separately pinned controller SHA. The controller then validates the plan, owned diff, allow-listed commands, durable outcomes, ancestry, and CAS before acceptance.

**U1 runtime migration:** version-3 state is migrated in memory for read-only commands and is written as version 4 only by a mutating controller command. Migration preserves units, history, blockers, worktrees, and current integration/DAG evidence; do not archive/reseed, modify `automation/pi-port`, or discard logs.

A monitoring timer may call `monitor` only. It must be read-only and must not call `run`. Its manifest counts are explicitly mechanical target-presence inventory, never fidelity-completion claims.

A newly accepted unit is cleaned automatically after its state and logs are durable: the controller removes only that unit's clean worktree and exact ref when its commit is reachable from integration. Historical cleanup remains explicit. `cleanup` defaults to a read-only dry-run; `--accepted` removes eligible historical attempts. Failed, blocked, running, dirty, and incomplete attempts are retained. Cleanup uses `git worktree remove` and exact CAS ref deletion; it never broadly removes runtime paths.

`upgrade` is a one-shot reviewed recovery checkpoint, not a normal dispatch path. It requires the clean checked-out candidate HEAD to descend from integration, preserve the Pi gitlink, and contain the approved recovery plan. With `--supersede`, it must name exactly the active failed blockers and leave a reachable DAG frontier. Without `--supersede`, it may preserve an active DAG for a control-only update, or replace a fully completed DAG with fresh, non-reused IDs and a ready/reachable frontier. It never replaces an incomplete active DAG. It persists `ACCEPTING` before CAS so startup reconciliation can finish an interrupted upgrade without losing runtime history.

Monitoring remains read-only. Recovery performs one ledger-bounded action: `repair` + service resume for REPAIRABLE, `retry` + resume for TRANSIENT, and `replan` + resume for PLAN_CHANGE_REQUIRED. ARBITRATION_REQUIRED only notifies and pauses. Every unit still gets a fresh Pi session and worktree.
