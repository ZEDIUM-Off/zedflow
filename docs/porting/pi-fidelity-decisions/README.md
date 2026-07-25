# Pi fidelity decisions

This directory is append-only historical evidence. Each file records the Pi behavior, Rust implementation, validation SHA, alternatives, and blockers observed by one completed or superseded unit.

These files are not an operational queue and their “next wave” wording is historical. Current authority is:

1. `docs/porting/BASELINE.md` for the human Stage-1 status and exit gate;
2. `python3 tools/pi-port-swarm/controller.py status` for runtime/DAG state;
3. `python3 tools/pi-port-swarm/manifest.py status` for deterministic mapping closure.

Nothing here authorizes modifying the frozen `references/pi` gitlink.
