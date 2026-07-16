<!-- migration-document-status: SUPERSEDED / HISTORICAL -->
> [!CAUTION]
> **Migration status: SUPERSEDED / HISTORICAL.** Do not use this file as the current execution tracker. See `.agents/state/zedflow-ai-agent-pi-fidelity-current-status.md`.

# Zedflow agent port ownership map (A0)

Generated for A0 of `.agents/plans/zedflow-agent-pi-agent-port.md`.

Scope: no Rust source edits; this file is the handoff map for A1-A8 and AT1-AT7. Later workers should not redo dependency discovery or invent alternate owners.

## Approved dependency replacements

| Pi dependency / API | Pi source usage | Rust replacement later units must use | Owner | Blocker / note |
|---|---|---|---|---|
| `@earendil-works/pi-ai`, `@earendil-works/pi-ai/compat` | `src/types.ts`, `src/agent-loop.ts`, `src/agent.ts`, `src/proxy.ts`, `src/harness/types.ts`, `src/harness/messages.ts`, `src/harness/session/session.ts`, `src/harness/compaction/*.ts`, `src/harness/agent-harness.ts` | `zedflow-ai` public Pi-compatible facade; re-export/wrap where needed | A1 for type contracts; A5-A7 for behavior | Blocker if `zedflow-ai` lacks `EventStream`, `streamSimple`/`Models.streamSimple`, `validateToolArguments`, `parseStreamingJson`, or compatible message/event/tool aliases. Do not duplicate these in `zedflow-agent`. |
| `typebox` | `src/types.ts`; test helpers `test/utils/*.ts` | `serde_json::Value` / `ToolSchema`; `jsonschema` only where agent owns runtime validation | A1; AT5/AT6 helpers read the A1 schema shape | No Rust `typebox` dependency. TS `Static<T>` compile-time typing has no direct Rust runtime counterpart. |
| `ignore` | `src/harness/skills.ts` | Rust `ignore` crate | A3; AT2 tests | Must preserve `.gitignore`, `.ignore`, `.fdignore`, hidden entry and `node_modules` skip semantics. |
| `yaml` | `src/harness/prompt-templates.ts`, `src/harness/skills.ts` | `yaml_serde` | A3; AT2 tests | Use for frontmatter only; do not hand-roll unless parent approves a dependency blocker. |
| Node builtins: `child_process`, `crypto`, `fs`, `fs/promises`, `os`, `path`, `readline` | `src/harness/env/nodejs.ts`; tests use Node fs/path fixtures | `std::process` + `wait-timeout`, `uuid`, `std::fs`, `std::env::temp_dir`, `std::path`, `std::io::{BufRead, BufReader}` | A4; AT3 tests | Pi process-tree kill and shell/platform behavior may not be fully covered by stdlib. If exact parity is missing, add a searchable `PORT PLACEHOLDER`/ignored test reason instead of adding a broad process supervisor crate. |
| `crypto.randomUUID` temp IDs | `src/harness/env/nodejs.ts` | `uuid::Uuid::new_v4()` | A4 | Straight v4 replacement is approved for temp path IDs. |
| Custom `uuidv7()` | `src/harness/session/uuid.ts`, session storage/repo files, `test/harness/session-uuid.test.ts` | Approved plan says `uuid` with v4; no new dependency selected | A2 owns implementation; AT1 owns tests | **Dependency replacement blocker:** Pi has UUIDv7 observable behavior with monotonic order and UUID version 7 assertions. A2 must either implement the tiny local UUIDv7 algorithm over approved `uuid`/stdlib randomness or report that the approved v4 direction intentionally breaks `session-uuid` parity. Do not add a new uuid-v7 crate without approval. |

## Source import graph summary

Foundations observed from `references/pi/packages/agent/src` imports:

1. `src/types.ts` and `src/harness/types.ts` are roots for public/private contracts. `harness/types.ts` has TS-only export cycles through `../index.ts`, `./session/session.ts`, and `./agent-harness.ts`; Rust must avoid duplicate type definitions.
2. `src/harness/messages.ts` is shared by session context reconstruction, compaction, branch summarization, and harness.
3. `src/harness/session/*` owns session tree and persistence; `session.ts` imports message constructors.
4. `src/harness/prompt-templates.ts` and `src/harness/skills.ts` both parse YAML frontmatter and use `ExecutionEnv`; `skills.ts` additionally owns ignore semantics.
5. `src/harness/env/nodejs.ts` is the only Node builtin implementation file.
6. `src/harness/compaction/*` depends on messages and sessions.
7. `src/agent-loop.ts` and `src/agent.ts` depend on root types and `zedflow-ai` stream/tool validation contracts.
8. `src/harness/agent-harness.ts` integrates all prior subsystems and must remain late/exclusive.
9. `src/index.ts` and `src/node.ts` are facade/export closure only after implementations exist.

## Ordered source implementation groups

### A1 — Canonical types, errors, and dependency scaffold

Owns foundational contracts before any leaf behavior. Later units may read and extend only within their own files unless blocked.

| Pi source | Rust target / extra file | Ownership |
|---|---|---|
| `src/types.ts` | `crates/zedflow-agent/src/types.rs` | Full A1 ownership for root agent/tool/context/schema/stream/hook contracts. |
| `src/harness/types.ts` | `crates/zedflow-agent/src/harness/types.rs` | Full A1 ownership for `Result`, errors, env/session/skill/template/harness contracts. |
| `src/index.ts` | `crates/zedflow-agent/src/index.rs` | A1 skeleton only; A8 owns final exports. |
| n/a | `crates/zedflow-agent/src/lib.rs` | A1 module scaffold; A8 final closure. |
| n/a | `crates/zedflow-agent/Cargo.toml` | A1 adds approved replacements only: `serde`, `serde_json`, `jsonschema`, `ignore`, `yaml_serde`, `uuid` with `v4`/`serde`, `wait-timeout`. |

Dependency notes: A1 must reuse `zedflow-ai` public types and must not create a second model/message/tool/event universe.

### A2 — Session and storage foundations

May run after A1; write scope is only session files. It reads `harness/messages.rs` contract if A3 has landed, but must not edit message files.

| Pi source | Rust target | Ownership |
|---|---|---|
| `src/harness/session/uuid.ts` | `crates/zedflow-agent/src/harness/session/uuid.rs` | A2 full ownership; see UUIDv7 blocker above. |
| `src/harness/session/session.ts` | `crates/zedflow-agent/src/harness/session/session.rs` | A2 full ownership. |
| `src/harness/session/memory-storage.ts` | `crates/zedflow-agent/src/harness/session/memory-storage.rs` | A2 full ownership. |
| `src/harness/session/memory-repo.ts` | `crates/zedflow-agent/src/harness/session/memory-repo.rs` | A2 full ownership. |
| `src/harness/session/repo-utils.ts` | `crates/zedflow-agent/src/harness/session/repo-utils.rs` | A2 full ownership. |
| `src/harness/session/jsonl-storage.ts` | `crates/zedflow-agent/src/harness/session/jsonl-storage.rs` | A2 full ownership. |
| `src/harness/session/jsonl-repo.ts` | `crates/zedflow-agent/src/harness/session/jsonl-repo.rs` | A2 full ownership. |

Dependency notes: session JSONL shape and context reconstruction are format-sensitive; do not change layout to simplify compaction/harness.

### A3 — Messages, templates, skills, system prompt, and text utilities

May run after A1; write scope is disjoint from A2 except semantic calls from session into messages.

| Pi source | Rust target | Ownership |
|---|---|---|
| `src/harness/messages.ts` | `crates/zedflow-agent/src/harness/messages.rs` | A3 full ownership. |
| `src/harness/prompt-templates.ts` | `crates/zedflow-agent/src/harness/prompt-templates.rs` | A3 full ownership; uses `yaml_serde`. |
| `src/harness/skills.ts` | `crates/zedflow-agent/src/harness/skills.rs` | A3 full ownership; uses `ignore` + `yaml_serde`. |
| `src/harness/system-prompt.ts` | `crates/zedflow-agent/src/harness/system-prompt.rs` | A3 full ownership. |
| `src/harness/utils/truncate.ts` | `crates/zedflow-agent/src/harness/utils/truncate.rs` | A3 full ownership. |
| `src/harness/utils/shell-output.ts` | `crates/zedflow-agent/src/harness/utils/shell-output.rs` | A3 full ownership. |

Dependency notes: `shell-output.rs` depends on `ExecutionEnv` from A1, not A4's concrete env. Do not implement filesystem/process backends here.

### A4 — Node execution environment and proxy seam

Runs after A1. It may run near A2/A3 only if no shared writes are needed. A8 owns final facade exports.

| Pi source | Rust target | Ownership |
|---|---|---|
| `src/harness/env/nodejs.ts` | `crates/zedflow-agent/src/harness/env/nodejs.rs` | A4 full ownership. |
| `src/proxy.ts` | `crates/zedflow-agent/src/proxy.rs` | A4 full ownership. |
| `src/node.ts` | `crates/zedflow-agent/src/node.rs` | A4 behavior/skeleton ownership; A8 owns final export closure. |

Dependency notes: use stdlib + `wait-timeout`; process-tree parity is a known blocker/placeholder candidate.

### A5 — Compaction and branch summarization

Runs after A2 and A3 because it consumes session context and message conversion.

| Pi source | Rust target | Ownership |
|---|---|---|
| `src/harness/compaction/utils.ts` | `crates/zedflow-agent/src/harness/compaction/utils.rs` | A5 full ownership. |
| `src/harness/compaction/compaction.ts` | `crates/zedflow-agent/src/harness/compaction/compaction.rs` | A5 full ownership. |
| `src/harness/compaction/branch-summarization.ts` | `crates/zedflow-agent/src/harness/compaction/branch-summarization.rs` | A5 full ownership. |

Dependency notes: use `zedflow-ai` model APIs; no live provider checks in this unit.

### A6 — Agent loop and agent facade behavior

Runs after A1 and A3. It consumes root types, message conversion, and `zedflow-ai` streams.

| Pi source | Rust target | Ownership |
|---|---|---|
| `src/agent-loop.ts` | `crates/zedflow-agent/src/agent-loop.rs` | A6 full ownership. |
| `src/agent.ts` | `crates/zedflow-agent/src/agent.rs` | A6 full ownership. |

Dependency notes: no alternate stream abstraction; preserve event order, tool execution modes, queue modes, hooks, and continuation semantics.

### A7 — Agent harness integration

Runs after A2-A6. This file is exclusive because it integrates every subsystem.

| Pi source | Rust target | Ownership |
|---|---|---|
| `src/harness/agent-harness.ts` | `crates/zedflow-agent/src/harness/agent-harness.rs` | A7 full ownership. |

Dependency notes: A7 must wire existing session/messages/templates/skills/env/compaction/loop modules; no private duplicate contracts.

### A8 — Root facade and module closure

Runs after A7. It audits the whole source manifest and closes public exports without adding behavior.

| Pi source | Rust target / extra file | Ownership |
|---|---|---|
| `src/index.ts` | `crates/zedflow-agent/src/index.rs` | A8 final export ownership. |
| `src/node.ts` | `crates/zedflow-agent/src/node.rs` | A8 final export ownership after A4. |
| n/a | `crates/zedflow-agent/src/lib.rs` | A8 final module declaration/manifest closure. |
| `.agents/port-manifests/agent-src.tsv` | n/a | A8 verifies every source row is represented or has exact `PORT PLACEHOLDER`. |

## Test groups

Group by subsystem, not raw manifest order. Test units own only these target files.

### AT1 — Session/storage/UUID tests

Depends on A2 and A8. `session-test-utils.rs` is shared test support; other test units should read it but not edit it.

| Pi test/source | Rust target | Ownership |
|---|---|---|
| `test/harness/session-test-utils.ts` | `crates/zedflow-agent/tests/harness/session-test-utils.rs` | AT1 full ownership. |
| `test/harness/session-uuid.test.ts` | `crates/zedflow-agent/tests/harness/session-uuid.rs` | AT1 full ownership; affected by UUIDv7 blocker. |
| `test/harness/session.test.ts` | `crates/zedflow-agent/tests/harness/session.rs` | AT1 full ownership. |
| `test/harness/storage.test.ts` | `crates/zedflow-agent/tests/harness/storage.rs` | AT1 full ownership. |
| `test/harness/repo.test.ts` | `crates/zedflow-agent/tests/harness/repo.rs` | AT1 full ownership. |

### AT2 — Prompt, skills, system prompt, formatting, and truncation tests

Depends on A3 and A8. Some Pi tests instantiate `NodeExecutionEnv` for fixture IO; use existing test helpers/env if available, but do not edit A4 source.

| Pi test | Rust target | Ownership |
|---|---|---|
| `test/harness/prompt-templates.test.ts` | `crates/zedflow-agent/tests/harness/prompt-templates.rs` | AT2 full ownership. |
| `test/harness/skills.test.ts` | `crates/zedflow-agent/tests/harness/skills.rs` | AT2 full ownership. |
| `test/harness/system-prompt.test.ts` | `crates/zedflow-agent/tests/harness/system-prompt.rs` | AT2 full ownership. |
| `test/harness/truncate.test.ts` | `crates/zedflow-agent/tests/harness/truncate.rs` | AT2 full ownership. |
| `test/harness/resource-formatting.test.ts` | `crates/zedflow-agent/tests/harness/resource-formatting.rs` | AT2 full ownership. |

### AT3 — Environment, proxy-adjacent utilities, and reusable tool helpers

Depends on A4 and A8. The two `tests/utils` files are helpers imported by harness/e2e tests; AT6/AT7 should not edit them. If AT6 runs in parallel and needs them, coordinate through parent or inline local helpers in its own test file.

| Pi test/source | Rust target | Ownership |
|---|---|---|
| `test/harness/nodejs-env.test.ts` | `crates/zedflow-agent/tests/harness/nodejs-env.rs` | AT3 full ownership. |
| `test/utils/calculate.ts` | `crates/zedflow-agent/tests/utils/calculate.rs` | AT3 full ownership. |
| `test/utils/get-current-time.ts` | `crates/zedflow-agent/tests/utils/get-current-time.rs` | AT3 full ownership. |

### AT4 — Compaction tests

Depends on A5 and A8.

| Pi test | Rust target | Ownership |
|---|---|---|
| `test/harness/compaction.test.ts` | `crates/zedflow-agent/tests/harness/compaction.rs` | AT4 full ownership. |

### AT5 — Agent loop and agent API tests

Depends on A6 and A8.

| Pi test | Rust target | Ownership |
|---|---|---|
| `test/agent-loop.test.ts` | `crates/zedflow-agent/tests/agent-loop.rs` | AT5 full ownership. |
| `test/agent.test.ts` | `crates/zedflow-agent/tests/agent.rs` | AT5 full ownership. |

### AT6 — Harness integration and stream tests

Depends on A7 and A8. Reads `tests/utils/*` if AT3 has landed; otherwise do not edit those helper targets without parent approval.

| Pi test | Rust target | Ownership |
|---|---|---|
| `test/harness/agent-harness.test.ts` | `crates/zedflow-agent/tests/harness/agent-harness.rs` | AT6 full ownership. |
| `test/harness/agent-harness-stream.test.ts` | `crates/zedflow-agent/tests/harness/agent-harness-stream.rs` | AT6 full ownership. |

### AT7 — E2E and scratch representation

Depends on A8. Do not run live provider/browser behavior.

| Pi test/source | Rust target | Ownership |
|---|---|---|
| `test/e2e.test.ts` | `crates/zedflow-agent/tests/e2e.rs` | AT7 full ownership. |
| `test/scratch/simple.ts` | `crates/zedflow-agent/tests/scratch/simple.rs` | AT7 full ownership. |

## Cross-unit write locks

- `crates/zedflow-agent/src/types.rs`: A1 only.
- `crates/zedflow-agent/src/harness/types.rs`: A1 only; later missing fields require explicit handoff note.
- `crates/zedflow-agent/src/harness/session/*`: A2 only.
- `crates/zedflow-agent/src/harness/messages.rs`, `prompt-templates.rs`, `skills.rs`, `system-prompt.rs`, `utils/*`: A3 only.
- `crates/zedflow-agent/src/harness/env/nodejs.rs`, `src/proxy.rs`: A4 only.
- `crates/zedflow-agent/src/harness/compaction/*`: A5 only.
- `crates/zedflow-agent/src/agent-loop.rs`, `src/agent.rs`: A6 only.
- `crates/zedflow-agent/src/harness/agent-harness.rs`: A7 only.
- `crates/zedflow-agent/src/index.rs`, `src/node.rs`, `src/lib.rs`: sequential A1/A4 skeleton then A8 closure; no leaf task should use these to hide missing implementation.
- `crates/zedflow-agent/tests/harness/session-test-utils.rs` and `crates/zedflow-agent/tests/utils/*.rs` are shared test support; only their owning AT unit edits them.

## Residual blockers to carry forward

1. UUID mismatch: Pi source and tests assert UUIDv7 monotonic ordering, while the approved plan text says UUID v4. This must be settled by A2/AT1 as a documented parity exception or a tiny local UUIDv7 implementation using approved dependencies.
2. `zedflow-ai` facade completeness is prerequisite for A1/A6/A7/A4 proxy work. Missing stream/event/tool validation/parser exports are blockers, not reasons to define parallel types.
3. Process-tree kill parity in `NodeExecutionEnv` is not guaranteed by stdlib + `wait-timeout`; document exact limitations rather than adding a process supervisor crate.
4. AT3 owns test helper rows used by AT6/AT7. Parallel test work must avoid competing edits to `crates/zedflow-agent/tests/utils/*.rs`.
