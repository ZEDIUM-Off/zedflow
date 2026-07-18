# Pi port task graph

This directory contains data only. `dag.json` is read by the persistent coordinator and worker Pi sessions; it does not launch Pi processes.

Start the sessions manually, name them `zedflow-port-coordinator` and `zedflow-port-worker`, and load `.pi/prompts/pi-port-coordinator.md` and `.pi/prompts/pi-port-worker-session.md`. Coordination uses the pinned pi-intercom package in `.pi/settings.json`.

The old Python swarm, external runtime state, worktree pool, and per-task `pi -p` launcher were retired. Paseo stays paused during the pilot.
