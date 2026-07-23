# Replan coding-agent RPC entrypoint dispatch

The `CA-RV-FID-V21-R2` review found that the production RPC entrypoint still calls the framing-only `run_rpc_loop`, leaving `AgentSessionRuntime` dispatch unreachable. The originating reviewer is removed from the active DAG.

`CA-RV-FID-V21-R4-RPC-ENTRY-DISPATCH` attaches to the reviewer's already-satisfied dependency `CA-V19-R2-CODING-AGENT` and owns only `crates/zedflow-coding-agent/src/main.rs` and `crates/zedflow-coding-agent/src/modes/rpc/rpc-entry.rs`. Fresh validator `CA-V19-R3-CODING-AGENT` preserves the failed review diff gate and coding-agent checks, followed by fresh reviewer `CA-RV-FID-V21-R3` before `CA-NEXT-PORT-DAG-V19`.
