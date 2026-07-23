# Replan core session runtime ownership

The `CA-C24-CORE-SESSION-RUNTIME` candidate changed `crates/zedflow-coding-agent/src/lib.rs` while that file was outside the unit's declared ownership. The originating writer is removed from the active DAG.

`CA-C24-R1-CORE-SESSION-RUNTIME-OWNERSHIP` attaches to the originating writer's already-satisfied dependency `CA-C23-REMAINING-UTILITY-ROWS` and re-runs the core session runtime port with `src/lib.rs` explicitly owned alongside the core implementation and focused tests. `CA-C24-V1-CORE-SESSION-RUNTIME` is a fresh validator preserving the formatting gate and adding focused package compilation and tests before `CA-C25-RESOURCE-EXTENSION-RUNTIME`.
