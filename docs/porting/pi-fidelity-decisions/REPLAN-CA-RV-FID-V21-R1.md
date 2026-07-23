# Replan coding-agent runtime dispatch and RPC wire compatibility

The `CA-RV-FID-V21-R1` review found that coding-agent runtime dispatch is missing. The originating reviewer is removed from the active DAG; its already-satisfied validator `CA-V19-R1-CODING-AGENT` remains the repair prerequisite.

`CA-RV-FID-V21-R2-RUNTIME-DISPATCH-WIRE` was blocked because the missing live RPC dispatch is in `crates/zedflow-coding-agent/src/modes/rpc/rpc-mode.rs`, outside its ownership. It is removed from the active DAG and replaced by `CA-RV-FID-V21-R3-RUNTIME-DISPATCH-WIRE`, attached to `CA-V19-R1-CODING-AGENT`, with explicit ownership of that diagnosed file alongside the original runtime files. Fresh validator `CA-V19-R2-CODING-AGENT` preserves the failed review diff gate and coding-agent checks, followed by fresh reviewer `CA-RV-FID-V21-R2` before `CA-NEXT-PORT-DAG-V19`.
