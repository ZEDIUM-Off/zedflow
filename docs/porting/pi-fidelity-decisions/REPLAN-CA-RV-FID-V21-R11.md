# Replan remaining coding-agent RPC fidelity

The `CA-RV-FID-V21-R11` review found concrete Pi-fidelity gaps in `rpc-mode.rs`: prompt responses are emitted only after the full turn instead of after Pi preflight (`:387-422`), input-order responses are forced instead of Pi completion-order dispatch (`:237-253`), extension commands and source metadata are omitted or fabricated (`:839-868`), statistics use model context and hardcode `sessionFile` (`:633-687`), and Bash execution is not recorded (`:600-631`). Prompt/skill expansion is also missing (`:387-450`).

The originating reviewer is removed from the active DAG rather than retried. `CA-RV-FID-V21-R11-REPAIR` attaches to its already-satisfied dependency `CA-V19-R8-CODING-AGENT` and owns only the diagnosed RPC mode and entrypoint files. Fresh validator `CA-V19-R9-CODING-AGENT` preserves the failed review diff gate and coding-agent package checks, followed by fresh reviewer `CA-RV-FID-V21-R12` before `CA-NEXT-PORT-DAG-V19`.
