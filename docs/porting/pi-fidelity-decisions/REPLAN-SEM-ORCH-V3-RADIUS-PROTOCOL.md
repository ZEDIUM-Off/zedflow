# Replan Orchestrator Radius protocol fidelity

`SEM-ORCH-V3-FIDELITY` found that frozen Pi `radius.ts` registers machines and Pis through Radius HTTP, schedules heartbeats with exponential retry/backoff and three-404 recovery, and remotely disconnects both records. Rust `radius.rs` only saved a local machine, returned the input Pi record, and made Pi disconnect a no-op.

The terminal reviewer is removed from the active DAG. `REPLAN-SEM-ORCH-V3-RADIUS-PROTOCOL-REPAIR` attaches directly to its already-satisfied predecessor, `SEM-ORCH-V2-VALIDATE`, and owns only the Radius lifecycle integration and its local tests. It reuses the already locked `reqwest` transport; no new dependency is approved. `REPLAN-SEM-ORCH-V3-FIDELITY-R1` is the fresh equivalent review before the AI and Agent residual units.
