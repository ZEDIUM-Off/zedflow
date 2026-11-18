---
description: Read when you choose Agent helpers, Graph operations or Code Mode for an experimental harness.
---

# Package qualification

## Use the evidence

Read [observed.yaml](observed.yaml) for versions, comparisons, negative findings, measurements and remaining adapter work.
Use the [matrix](qualification-matrix.json) to locate each executable assertion.
Compare semantic workflows separately from the [native loop protocol](loop-results.md).
Treat simulated responses as control-flow evidence, not provider transport or model-quality evidence.

## Agent boundaries

Keep each incompatible Agent generation in its isolated package.
Let machine states own approvals, tools, retry policies and terminal outcomes.
Keep each provider executor single-turn when those steps must remain visible.
Choose one authoritative transcript before adding another package's memory.
Verify cancellation at the provider boundary and at each observable memory boundary.
Restore human waits with fresh capability bindings and an explicit revision policy.
Qualify decision retries separately from transport retries.

## Graph boundaries

Keep authored definitions, structural inspection views, reachable snapshots and observed traces distinct.
Preserve provenance and reject conflicts before combining definitions.
Check entity identities before adopting a graph transformation.
Use patches for changed entity values rather than relying on set difference.
Supply finite event payloads and an explicit expansion limit for behavior exploration.
Report cut boundaries and unexecuted invokes as incomplete evidence.

## Code Mode boundaries

Validate and authorize every injected capability call in the host.
Declare time, memory and call limits before starting a VM.
Distinguish script settlement from worker exit and external effect completion.
Persist the deterministic run envelope and ordered results separately from the machine snapshot.
Reject incompatible fingerprints, changed calls and incomplete journals before replay.
Use an external effect journal to resolve duplicate or uncertain effects.
Qualify partial-run recovery and storage durability before claiming durable program resumption.

## Gotchas

Type an empty stable Agent event registry explicitly when request helpers require a broader event type.
Use enqueued emit effects for XState reducer-style transitions; keep direct side effects in host capabilities.
Check unfinished calls before marking a successful VM run's journal complete.
