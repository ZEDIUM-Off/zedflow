<!-- migration-document-status: SUPERSEDED / HISTORICAL -->
> [!CAUTION]
> **Migration status: SUPERSEDED / HISTORICAL.** Do not use this file as the current execution tracker. See `.agents/state/zedflow-ai-agent-pi-fidelity-current-status.md`.

# Zedflow Agent dependency reconnaissance

Purpose: pre-plan dependency replacement for `references/pi/packages/agent` before revising the W3/P2.T1 port plan.

## Package dependencies from `references/pi/packages/agent/package.json`

Runtime dependencies:

| TS dependency | Used by | Responsibility in Pi agent | Rust replacement options | Recommendation |
|---|---|---|---|---|
| `@earendil-works/pi-ai` | `types.ts`, `agent-loop.ts`, `agent.ts`, harness compaction/messages/session/proxy | Canonical model/context/message/tool/stream/event types, provider streaming, validation helpers, compat API, model registry | Use existing `zedflow-ai` crate. Do not duplicate message/model/stream types. If a required API is missing, add the smallest `zedflow-ai` facade/helper first. | **Use `zedflow-ai` as foundation dependency.** Sequence agent `types` after confirming required root exports from R13. |
| `ignore` 7.0.5 | `harness/skills.ts` | Gitignore-style filtering while recursively loading skills. Honors `.gitignore`, `.ignore`, `.fdignore`, hidden/node_modules skip behavior. | Rust `ignore` crate (`ignore::gitignore::GitignoreBuilder` or `WalkBuilder`), or small custom matcher. | **Prefer Rust `ignore` crate** if exact gitignore semantics matter. Avoid custom matcher except for a very small subset; skill traversal likely needs real ignore semantics. |
| `typebox` 1.1.38 | `types.ts` only, via `TSchema`/`Static<T>` for `AgentTool` schemas and typed tool params | Compile-time TS schema typing for tool parameters; runtime validation itself happens through Pi AI compat (`validateToolArguments`) / schema value passed with tool. | Existing `zedflow-ai` uses `serde_json` + `jsonschema`; Rust `schemars` for deriving schema; Rust `typebox` crate exists but may not match Pi. | **Do not add Rust `typebox` by default.** Represent schemas as `serde_json::Value` / `zedflow_ai` tool schema types; use `jsonschema` only where validation is owned by agent. |
| `yaml` 2.9.0 | `harness/prompt-templates.ts`, `harness/skills.ts` | Parse frontmatter in Markdown prompt templates and skill files. Only maps simple metadata fields; diagnostics on parse failure. | `yaml_serde` (maintained Serde YAML fork), `serde_yaml` (common but maintenance concerns), `yaml-rust2`, or minimal frontmatter parser + limited YAML. | **Prefer `yaml_serde` if adding dependency is acceptable.** If dependency budget is strict, parse frontmatter delimiters ourselves and use `serde_json`-like simple `key: value` parser only if tests allow. |

Dev dependencies (`typescript`, `vitest`, `@types/node`, coverage) are build/test tooling only and should not be ported.

## Node builtins used by `harness/env/nodejs.ts`

| Node builtin | Responsibility | Rust replacement options | Recommendation |
|---|---|---|---|
| `node:child_process.spawn` | Execute shell commands with timeout, stdout/stderr streaming/truncation, env/cwd, process-tree kill | `std::process::Command`; `tokio::process` for async/cancellation; `duct` crate; `command-group` for process groups | Start with **std::process + thread/timeout** if sync API is enough; use `tokio::process` only if async trait design requires it. Process-tree kill may need Unix process group support or documented placeholder on Windows. |
| `node:crypto.randomUUID` | Session/temp IDs | `uuid` crate v4, or existing `getrandom` + formatting | Prefer **`uuid` crate** if IDs must be UUID-shaped; otherwise `zedflow-core` helper. |
| `node:fs`, `node:fs/promises` | File info, read/write/append, mkdir, rm, realpath, symlink-aware lstat, streams | `std::fs`; `tokio::fs` if async; `walkdir` for recursive | Prefer **std::fs** behind `ExecutionEnv` trait; keep async facade only if public API requires it. |
| `node:os.tmpdir` | Temp dirs | `std::env::temp_dir`, maybe `tempfile` for tests | `std::env::temp_dir`; add `tempfile` as dev-dep only if tests need isolated dirs. |
| `node:path` | join/resolve/isAbsolute/basename/dirname/relative | `std::path::{Path, PathBuf}` | stdlib. |
| `node:readline` | Read file first lines / stream line processing | `std::io::{BufRead, BufReader}` | stdlib. |

## Internal dependency graph highlights

Foundational files with many dependents:

1. `src/types.ts` — root agent loop/tool/context types; imports `pi-ai` + `typebox`.
2. `src/harness/types.ts` — Result helpers, errors, env/session/skill/template/harness contracts; imported by most harness modules.
3. `src/harness/messages.ts` — AgentMessage <-> LLM message conversion and message constructors; used by session/compaction/agent harness.
4. `src/harness/session/session.ts` + storage/repo modules — session tree and context reconstruction; compaction/harness depend on it.
5. `src/agent-loop.ts` — behavioral core; depends on stable types and `zedflow-ai` compat.
6. `src/harness/agent-harness.ts` — integration layer; should be late.
7. `src/index.ts` / `src/node.ts` — facade; should be after modules exist.

## Proposed dependency decisions to debate

1. Add `ignore = "0.4"` to `zedflow-agent` for skill traversal?
   - Pros: closest `.gitignore` semantics, avoids homegrown edge cases.
   - Cons: dependency add.
   - My recommendation: yes.

2. YAML parser choice for frontmatter?
   - Option A: `yaml_serde` for maintained Serde-compatible YAML.
   - Option B: `serde_yaml` for familiar ecosystem but maintenance concern.
   - Option C: no YAML dependency; hand-parse simple frontmatter.
   - My recommendation: `yaml_serde` unless tests show only simple scalar metadata and dependency budget is strict.

3. Tool schema representation?
   - Option A: reuse `zedflow-ai`/`serde_json::Value` schemas and validation helpers.
   - Option B: add `schemars` for Rust-generated schemas.
   - Option C: add Rust `typebox` crate.
   - My recommendation: A now. Add `schemars` later only if Rust users need deriving schemas. Avoid Rust `typebox` until proven needed.

4. Async/runtime choice for `ExecutionEnv` and session APIs?
   - Option A: keep sync std APIs and return `Result`.
   - Option B: use `tokio` throughout.
   - My recommendation: mirror Pi async at public boundaries only where observable; internally prefer std/small blocking helpers for now. Do not introduce full async runtime unless agent loop stream requires it.

5. Process execution/process-tree kill fidelity?
   - Option A: std::process minimal with timeout kill child only.
   - Option B: add process-group crate for tree kill parity.
   - Option C: placeholder Windows/process-tree edge cases.
   - My recommendation: start minimal with explicit documented limitation; add process-group only if tests require tree kill.

## Planning implication

Do **not** assign one subagent per manifest row in raw order for `zedflow-agent`. First wave should lock dependency choices and foundational shared types/modules, then row subagents can port leaf modules against those contracts.
