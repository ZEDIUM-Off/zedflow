# Port swarm state

Runtime state is external under `~/.local/state/zedflow-pi-port-swarm`; this directory contains only the durable schema, task index, and recorded crate decisions. States are `CLAIMED`, `IMPLEMENTED`, `REVIEWED`, `VALIDATED`, `INTEGRATED`, `CLOSED`, retryable `FAILED`, and terminal `BLOCKED`. The Pi gitlink is frozen at the value in `tools/pi-port-swarm/dag.json`.
