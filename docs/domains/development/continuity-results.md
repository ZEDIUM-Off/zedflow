---
description: Read when you assess persisted waits, uncertain effects, session recovery or branch continuity.
---

# Continuity qualification

## Use the evidence

Read [observed.yaml](observed.yaml) for measured results, comparison methods and remaining adapter work.
Use the [matrix](qualification-matrix.json) to locate executable assertions.
Keep these bounded fixtures separate from a complete durable [product](../product/DOMAIN.md) implementation.

## Human waits and restoration

Give each response a run, child, request and generation target.
Validate the declared response shape before advancing a wait.
Distinguish human waiting, control pause, failure and terminal completion.
Persist data-only actor snapshots and bind live capabilities when restoring the logic.
Reject incompatible definition revisions before creating actors.
Treat restored state and current external data as separate inspection sources.

## External effects

Record effect intent before dispatch and record the result before claiming durable completion.
Bind an idempotency key to the request it identifies.
Replay a recorded result instead of repeating the effect.
Require explicit reconciliation when an uncertain capability lacks idempotency support.
Do not infer exactly-once execution from actor persistence or a passing replay test.
Qualify storage durability and concurrent writers separately from these actor-stop fixtures.

## Session recovery

Keep failed attempts inspectable while omitting them from the repaired model projection.
Let native states own retry budgets, delays, compaction continuation and final settlement.
Stop internal recovery after its declared budget or failed repair.
Drain queued work before the pre-settlement boundary.
Preserve unstarted queued work when a host budget stops execution.
Await settlement observers before resolving the public run result.

## Branches

Select model context through one parent-linked lineage.
Keep the original branch unchanged when creating a fork.
Attach explicit summary provenance when carrying work from an abandoned branch.
Keep summary wrapper compatibility separate from lineage and content comparisons.

## Gotchas

Supply validated original input when the actor type requires it during snapshot restoration.
Treat checkpoint-envelope validation as distinct from validating arbitrary uploaded actor state.
Recheck cancellation after awaited observers before publishing subsequent recovery notices.
