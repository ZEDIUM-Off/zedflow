# Zedflow Rust workspace architecture

Zedflow is being rebuilt as a clean Rust workspace instead of continuing the inherited monolithic Pi Rust port.

The source-of-truth architecture follows the package separation in `references/pi/packages/` and maps it to small Rust crates:

| Pi reference package | Zedflow crate | Purpose |
| --- | --- | --- |
| `packages/ai` | `crates/zedflow-ai` | Providers, model registry, message streaming, auth-facing model APIs |
| `packages/agent` | `crates/zedflow-agent` | Agent loop primitives and model/tool turn coordination |
| `packages/coding-agent` | `crates/zedflow-coding-agent` | Coding-agent product assembly and CLI-facing harness behavior |
| `packages/orchestrator` | `crates/zedflow-orchestrator` | Flow orchestration, graph composition, Root Flow planning |
| `packages/tui` | `crates/zedflow-tui` | Terminal UI widgets and interaction surface |
| shared substrate | `crates/zedflow-core` | Shared errors, IDs, config primitives, common types |
| shared substrate | `crates/zedflow-tools` | Built-in tool definitions and execution guards |
| shared substrate | `crates/zedflow-session` | Sessions, persistence, checkpoint/session binding |
| LangGraph reference | `crates/zedflow-langgraph` | LangGraph sidecar adapter and runtime event bridge |

## Migration rule

The old root crate remains temporarily as a compiling quarry. New work should go into `crates/` unless it is explicitly preserving or extracting a piece of the old runtime.

Do not add new features to the monolith unless they are needed to keep validation green during extraction.
