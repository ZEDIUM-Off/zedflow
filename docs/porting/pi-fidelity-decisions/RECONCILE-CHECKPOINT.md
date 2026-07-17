# RECONCILE-CHECKPOINT — 2026-07-17

This checkpoint reconciles the clean live snapshot. It changes no Rust source, dependency, frozen reference, DAG node, or external coordinator state.

## Frozen inputs and evidence identity

- Base HEAD: `a4b2e7fb386227341def2769ff0939728bab0135` (`pi-port-swarm snapshot`).
- Frozen source: `references/pi@2b00dade7cec918aefb025c8b7a4fa304a30acdd`; the HEAD gitlink, checked-out submodule HEAD, DAG pin, and external bootstrap pin agree.
- Canonical coordinator plan hash: `ce94f241b6d8083312330f34fd624d314f4873ec6d26fbfdcdb3f6ef161f0244`, computed by `tools.pi-port-swarm.swarm.plan_hash` from the live DAG.
- Live DAG file SHA-256: `6af793bf7e70484dff0c8835161f4760f4c00a95d42937e58b98514b8f11943f`; it contains 20 valid, acyclic units.
- Active plan SHA-256: `91ecee27b9801c06a981e7e1b5f783da5fe83e02490d317d301b645e2dc80e1a` (`.agents/plans/zedflow-ai-agent-pi-fidelity-consolidation.md`).
- Active status SHA-256: `9be538a3d844705b54dee3c9af83ed6bcf43d2542e1bb9b67bd273f41fb1689d` (`.agents/state/zedflow-ai-agent-pi-fidelity-current-status.md`).
- Active tracker SHA-256: `f74f6b127f8bcaf9a26227a72e0c9bad1901abdbe376252c17625bb4b3a9e8e9` (`.agents/state/zedflow-ai-agent-pi-fidelity-consolidation-orchestration.md`).
- Agent source/test manifest SHA-256: `b54cef8b1ae5019d994506b4e71ac08a1863c37a3997a118ea08f1e674a4b162` / `bb68333a1690440bbf2ae197eda2d1278d0adde5913862703979bf3dcfe7a39f`.
- External state SHA-256 at reconciliation: `cb43654454b20a8e3cb2c061f9d94f241b0f9dd6690453805aef47567159ee25`; it records this unit as `CLAIMED` at the required base and plan hash.

The active status and tracker correctly identify AG-C1 as next, but they are historical snapshot evidence rather than the integration ledger: the status still describes HEAD `c293eac3`, a dirty worktree, and an older Agent check failure. Older checkpoint commits also live only on recovery branches based on `14b8693a`; they are not descendants of the current integration ref. Current Git, the external CAS state, and rerunnable checks therefore take precedence. The durable `.agents/port-swarm/tasks.tsv` was expanded from compressed ranges to mirror every live DAG role, model, dependency, ownership, and validation field; in particular, `NEXT-PORT-DAG` is Sol, not Terra.

## Current evidence and boundary

- Both manifests have exact presence and uniqueness: 25/25 Pi Agent source paths and Rust targets, and 20/20 Pi Agent test paths and Rust targets.
- The current deterministic Agent suite passes 115 tests with 6 ignored. Presence and green current tests do not prove Pi behavior.
- Pi Agent `StreamFn` may asynchronously produce its stream, while Rust aliases the immediate `zedflow_ai::StreamFunction`.
- Pi requires asynchronous tool `execute` and represents rejection; Rust makes `execute` optional and its future cannot return `Result`.
- Related fallible hook/event contracts and explicit argument replacement are also absent from current `types.rs`.

Consequently no AG-C1-or-later unit has exact-HEAD implementation/review/validation proof. **AG-C1 is the exact next dependency-safe unit.** AG-C2 through V2 remain transitively blocked; `NEXT-PORT-DAG` remains blocked by V2. Six ignored tests and the later live-stream, lifecycle, persistence, UUID, process-tree, proxy, and timezone gaps remain assigned to their existing DAG units.

## AG-C1 dependency choice

Workspace Rust `1.96.1` is the governing floor. Registry facts below come from the lockfile and cached crate manifests; no dependency is added.

| Option | Semantics | License / MSRV | Async and platform fit | Decision |
| --- | --- | --- | --- | --- |
| Rust standard library `Future`, `Pin`, and `Result` | Directly expresses boxed async/fallible closure outputs and preserves callback lifetimes without a macro; std has no stream trait, so the canonical AI event-stream type remains reused. | Rust toolchain: MIT OR Apache-2.0; workspace Rust `1.96.1` governs. | Runtime-neutral and portable; explicit boxing is appropriate at the existing trait-object callback boundary. | **Selected** for the AG-C1 contract shape. |
| Existing direct `futures` 0.3.32 | `future::BoxFuture` is essentially the existing `Pin<Box<dyn Future + Send>>` convenience alias; its stream utilities already support the canonical event stream but add no missing AG-C1 semantics. | MIT OR Apache-2.0; declared MSRV 1.71. | Executor-neutral and cross-platform; no runtime commitment required. | Retain/reuse where already used; no manifest change. |
| Maintained `async-trait` 0.1.89 alternative | Its proc macro boxes async **trait methods** for dyn compatibility. AG-C1 owns closure aliases/direct callers, so it would not remove the callback boxing or improve semantics. | MIT OR Apache-2.0; declared MSRV 1.56. | Runtime- and platform-neutral, but adds macro expansion and a direct API commitment; currently only transitive in the lockfile. | Not selected and not promoted to a direct dependency. |

The smallest option is std plus existing workspace types. Reconsider a new dependency only if AG-C1's owned signature work demonstrates a semantic requirement these cannot represent.

## Runnable checks

```sh
python3 tools/pi-port-swarm/swarm.py validate-dag
python3 -m unittest tools.pi-port-swarm.test_swarm
# Python audit: tasks.tsv exact field equality with dag.json; manifest presence/uniqueness
cargo fmt --all --check
cargo test -p zedflow-agent --all-targets --no-fail-fast
```

The DAG itself remains unchanged because the live evidence supports all 20 nodes and their current routing.
