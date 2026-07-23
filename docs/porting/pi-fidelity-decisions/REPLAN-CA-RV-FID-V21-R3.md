# Replan configured coding-agent RPC runtime

The `CA-RV-FID-V21-R3` review found that the RPC entrypoint still does not construct the configured Pi runtime and leaves dispatch incomplete. The originating reviewer is removed from the active DAG rather than retried.

`CA-RV-FID-V21-R6-RPC-ENTRY-CONFIGURED-RUNTIME` attaches to the reviewer's already-satisfied dependency `CA-V19-R3-CODING-AGENT` and owns only `crates/zedflow-coding-agent/src/rpc-entry.rs`. Fresh validator `CA-V19-R4-CODING-AGENT` preserves the review diff gate and coding-agent package checks, followed by fresh reviewer `CA-RV-FID-V21-R7` before `CA-NEXT-PORT-DAG-V19`.
