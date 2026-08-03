# Replan Orchestrator Radius empty-success fidelity

`REPLAN-REPLAN-REPLAN-REPLAN-REPLAN-REPLAN-REPLAN-REPLAN-SEM-ORCH-V3-FIDELITY-R8` found that frozen Pi `maybePost` accepts every successful HTTP response without parsing a body (`references/pi/packages/orchestrator/src/radius.ts:63-75`). Rust `maybe_post` delegated to JSON-deserializing `post`, so valid empty 2xx heartbeat and disconnect responses failed.

The terminal reviewer is removed from the active DAG. `REPLAN-REPLAN-REPLAN-REPLAN-REPLAN-REPLAN-REPLAN-REPLAN-SEM-ORCH-V3-RADIUS-EMPTY-SUCCESS-REPAIR-R1` attaches directly to its already-satisfied predecessor, `REPLAN-REPLAN-REPLAN-REPLAN-REPLAN-REPLAN-REPLAN-REPLAN-SEM-ORCH-V3-RADIUS-RECOVERY-REPAIR-R1`, and owns only `radius.rs` plus its local regression test. A fresh equivalent reviewer, `REPLAN-REPLAN-REPLAN-REPLAN-REPLAN-REPLAN-REPLAN-REPLAN-REPLAN-SEM-ORCH-V3-FIDELITY-R9`, follows it before AI and Agent residual work.
