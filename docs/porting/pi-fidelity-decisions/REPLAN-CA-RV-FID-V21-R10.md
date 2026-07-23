# Replan remaining coding-agent RPC fidelity

The `CA-RV-FID-V21-R10` review found concrete Pi-fidelity gaps: `rpc-mode.rs:178-209` dispatches commands concurrently instead of preserving Pi's ordered input handling; `:428-440` only stores auto-compaction and retry flags without changing session behavior; `:475-547` omits `contextUsage` and hardcodes `sessionFile`; and `:702-705` omits extension commands and required `sourceInfo`. `rpc-entry.rs:173-225` loads prompt templates in filesystem order and treats invalid YAML as valid plain content, diverging from Pi.

The originating reviewer is removed from the active DAG rather than retried. `CA-RV-FID-V21-R10-REPAIR` attaches to its already-satisfied dependency `CA-V19-R7-CODING-AGENT` and owns only the diagnosed RPC entrypoint files. Fresh validator `CA-V19-R8-CODING-AGENT` preserves the diff gate and coding-agent package checks, followed by fresh reviewer `CA-RV-FID-V21-R11` before `CA-NEXT-PORT-DAG-V19`.
