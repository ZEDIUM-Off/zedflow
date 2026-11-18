---
description: Read before choosing Effect modules for local routing, worker sleep and durable event handling.
---

# Evaluate Effect for run supervision

Use Effect as the infrastructure candidate, not as a replacement for native machine definitions.
Read `effect_research` in [observed.yaml](observed.yaml) for versions, measurements, probe outcomes and qualification limits.

## Preserve ownership

| Component | Responsibility |
|---|---|
| XState | Machine transitions, context and native checkpoints |
| Pi | Providers, authentication, model messages and tool contracts |
| Effect | Resource scopes, cancellation, routing and storage integration |
| Zedflow | Run identity, sleep eligibility, checkpoint publication and recovery policy |

Use [resource scopes](https://github.com/Effect-TS/effect/blob/effect%404.1.4/packages/effect/src/Effect.ts) around worker ownership.
Keep UI connection scopes separate from run ownership.
Do not propagate ordinary client cancellation into a durable run command without an explicit policy.

Keep [Workflow](https://github.com/Effect-TS/effect/blob/effect%404.1.4/packages/effect/src/workflow/Workflow.ts) outside the initial design.
Its durable execution model would add another business orchestration layer beside XState.
Do not replace Pi providers with Effect AI modules.

## Qualify the local router

Evaluate [SingleRunner](https://github.com/Effect-TS/effect/blob/effect%404.1.4/packages/effect/src/cluster/SingleRunner.ts) before building another mailbox implementation.
It composes SQL message storage and local entity routing without requiring a distributed deployment.
It does not provide the external CLI/TUI endpoint or establish an operating-system singleton.

Map a run identifier to an [Entity](https://github.com/Effect-TS/effect/blob/effect%404.1.4/packages/effect/src/cluster/Entity.ts).
Use its lifecycle to acquire and release a headless worker.
Restore the worker from its native active checkpoint.
Keep machine transitions inside that worker.

Preserve busy work explicitly.
Check native actor activity after an RPC returns; asynchronous work can remain active.
Use native activity tracking and entity keep-alive policy until checkpoint eligibility returns.
Serve UI observations from stored/read-model services.
A long-running entity RPC can keep the entity resident.

Review module stability annotations before adoption.
Core release support does not make every Cluster, SQL or Workflow API stable.
Pin admitted versions and rerun behavioral probes after upgrades.

## Close the remaining gaps

Qualify external local IPC, singleton ownership, stale-owner fencing and separate TUI processes.
Test checkpoint publication together with mailbox progress and replies.
Preserve uncertain outcomes after a worker crash instead of blindly repeating external effects.

Measure representative model histories and media payloads before choosing production budgets.
Keep polling configuration explicit.
The [reaper](https://github.com/Effect-TS/effect/blob/effect%404.1.4/packages/effect/src/cluster/internal/entityReaper.ts) performs periodic checks rather than exact-deadline eviction.

Account for synchronous SQLite lock waits when selecting the [Node driver](https://github.com/Effect-TS/effect/blob/effect%404.1.4/packages/sql/sqlite-node/src/SqliteClient.ts).
Keep the storage writer narrowly scoped and bound contention.

## Reproduce

Run from the mission workspace:

```sh
bash .artifacts/effect-research/run.sh probe
bash .artifacts/effect-research/run.sh measure
```

Use `bootstrap` only to restore the pinned research dependencies.
The wrapper isolates homes, caches and temporary data and disables installation scripts.
The prototypes remain disposable research code.

## Gotchas

A [SQL rollback](https://github.com/Effect-TS/effect/blob/effect%404.1.4/packages/effect/src/cluster/ClusterSchema.ts) does not rewind an already mutated XState actor.
Reconcile or discard that actor before processing another command.

Persisted request deduplication does not prevent a repeated external effect after an interrupted commit window.

The server-only uninterruptible policy interrupted and restarted the tested request.
The combined client/server policy preserved its bounded execution.
Treat this as version-qualified behavior, not a universal cancellation guarantee.
