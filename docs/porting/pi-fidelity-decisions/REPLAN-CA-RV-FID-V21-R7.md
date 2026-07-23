# Replan coding-agent RPC runtime fidelity

The `CA-RV-FID-V21-R7` review found two deterministic Pi-fidelity gaps: `rpc-mode.rs` blocks input until prompt completion, preventing `steer`/`follow_up` during streaming, and `rpc-entry.rs` always uses in-memory storage while ignoring `--no-session`, `--session`, `--resume`, and `--session-dir`. The originating reviewer is removed from the active DAG rather than retried.

`CA-RV-FID-V21-R7-REPAIR` attaches to the reviewer's already-satisfied dependency `CA-V19-R4-CODING-AGENT` and owns only the two diagnosed files. Fresh validator `CA-V19-R5-CODING-AGENT` preserves the failed review diff gate and coding-agent package checks, followed by fresh reviewer `CA-RV-FID-V21-R8` before `CA-NEXT-PORT-DAG-V19`.
