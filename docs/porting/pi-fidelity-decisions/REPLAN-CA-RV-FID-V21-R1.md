# Replan coding-agent runtime dispatch and RPC wire compatibility

The `CA-RV-FID-V21-R1` review found that coding-agent runtime dispatch is missing. The originating reviewer is removed from the active DAG; its already-satisfied validator `CA-V19-R1-CODING-AGENT` remains the repair prerequisite.

`CA-RV-FID-V21-R2-RUNTIME-DISPATCH-WIRE` owns only `crates/zedflow-coding-agent/src/main.rs` and `crates/zedflow-coding-agent/src/modes/rpc/rpc-types.rs`. Fresh validator `CA-V19-R2-CODING-AGENT` preserves the failed review diff gate and coding-agent checks, followed by fresh reviewer `CA-RV-FID-V21-R2` before `CA-NEXT-PORT-DAG-V19`.
