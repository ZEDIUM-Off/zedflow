# Pi port task graph

This directory contains the stage-1 port DAG. `dag.json` is read by the persistent coordinator and worker Pi sessions; it does not launch Pi processes.

Start the sessions manually as `zedflow-port-coordinator` and `zedflow-port-worker`, then load `.pi/prompts/pi-port-coordinator.md` and `.pi/prompts/pi-port-worker-session.md`. Coordination uses the pinned pi-intercom package in `.pi/settings.json`.

The previous per-task Python launcher remains retired. Paseo scheduling is paused until a manual pilot validates the current DAG, state, exact-SHA review, and one-writer integration path.
