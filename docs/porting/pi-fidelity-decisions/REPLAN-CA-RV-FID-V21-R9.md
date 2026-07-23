# Replan remaining coding-agent RPC fidelity

The `CA-RV-FID-V21-R9` review found remaining Pi-fidelity gaps: `rpc-mode.rs:219-242` waits for the full prompt instead of emitting after preflight, `:276` makes `new_session` always cancelled, `:438-460` stubs switch/fork/clone/fork-messages, `:384` and `:272-274` no-op or misroute retry/bash controls, and `:406-427` returns zero session stats. `rpc-entry.rs:114-152` never loads prompt templates or honors related resource flags, while `:189-212` ignores `--fork` and global session selection.

The originating reviewer is removed from the active DAG rather than retried. `CA-RV-FID-V21-R9-REPAIR` attaches to its already-satisfied dependency `CA-V19-R6-CODING-AGENT` and owns only the diagnosed RPC entrypoint files. Fresh validator `CA-V19-R7-CODING-AGENT` preserves the diff gate and coding-agent package checks, followed by fresh reviewer `CA-RV-FID-V21-R10` before `CA-NEXT-PORT-DAG-V19`.
