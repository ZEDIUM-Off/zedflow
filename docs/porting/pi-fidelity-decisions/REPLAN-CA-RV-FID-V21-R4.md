# Replan coding-agent RPC entrypoint dispatch ownership

The `CA-RV-FID-V21-R4-RPC-ENTRY-DISPATCH` worker was blocked because its declared `src/modes/rpc/rpc-entry.rs` path does not exist. The originating unit is removed from the active DAG rather than retried.

`CA-RV-FID-V21-R5-RPC-ENTRY-DISPATCH` attaches to the already-satisfied dependency `CA-V19-R2-CODING-AGENT` and owns the actual entrypoint paths `crates/zedflow-coding-agent/src/main.rs` and `crates/zedflow-coding-agent/src/rpc-entry.rs`. `CA-V19-R3-CODING-AGENT` is reconnected to the replacement and preserves the fresh coding-agent validation before `CA-RV-FID-V21-R3`.
