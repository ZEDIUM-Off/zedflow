# Replan coding-agent runtime dispatch and RPC wire compatibility

The `CA-RV-FID-V21` review found two deterministic Pi-fidelity gaps: the coding-agent entry point does not dispatch the selected runtime mode, and RPC command fields are not serialized with Pi's wire-compatible names. The originating reviewer is removed from the active DAG.

`CA-RV-FID-V21-R1-RUNTIME-DISPATCH-WIRE` attaches to the reviewer's already-satisfied dependency `CA-V19-CODING-AGENT` and owns only `crates/zedflow-coding-agent/src/main.rs` and `crates/zedflow-coding-agent/src/modes/rpc/rpc-types.rs`. Fresh package validation preserves the failed diff gate, followed by `CA-RV-FID-V21-R1`, a fresh Pi-fidelity reviewer, before `CA-NEXT-PORT-DAG-V19`.
