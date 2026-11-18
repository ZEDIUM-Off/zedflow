---
description: Read when you assess the native loop evidence or extend its behavior.
---

# Native loop qualification

## Conclusion

Use native XState execution as the baseline for the next qualification lots.
Treat this as conditional suitability for the tested in-memory loop, not complete [product](../product/DOMAIN.md) adoption.
Read [observed.yaml](observed.yaml) for measured results, source size and limitations.
Read the [matrix](qualification-matrix.json) for case-level executable evidence.

## Execution boundary

The machine owns request, tool-batch, queue, continuation and completion transitions.
A provider actor consumes one response stream.
A tool-batch actor performs finite preflight and effect execution.
The shared admission function also governs nested capability calls.
Pi AI supplies message utilities and argument validation.
Pi core supplies reference types and test oracles only.

## Comparison method

Feed equivalent deterministic responses and fixture tools to Pi and the native machine.
Compare finalized messages, prepared provider contexts, event order, event payloads and effects.
Normalize timing fields only.
Use explicit barriers to test parallel completion order and awaited observers.
Use the Pi Agent wrapper when queue behavior belongs above its low-level loop.

Exercise native control events independently alongside the differential cases.
Test cancellation during model and tool work, late updates and fresh-run isolation.
Keep source inspection distinct from executed comparisons.

## Host policy

The experiment applies a finite turn budget as an additional host policy.
Budget exhaustion produces an actor error rather than a normal completion event.
The caller must handle that outcome explicitly.
Do not present this policy as Pi behavior.

## Limits

The fixture stream follows the Pi AI terminal-event contract.
A provider that throws instead of returning protocol events requires separate adapter handling.
Provider transport, authentication and billing behavior remain outside these simulated results.

The runtime context contains live callbacks and cancellation resources.
Do not serialize it as a complete durable run.
Qualify resource rebinding and external effect recovery separately.

A batch exposes one machine state while individual effects run inside its bounded actor.
Qualify finer-grained composition and inspection before promising a complete editable tool graph.

## Next use

Run the native package check before changing queue or tool scheduling.
Preserve the reference comparisons when adding context transformation, compaction and lifecycle hooks.
Assess Stately Agent on the same scenarios rather than assuming this baseline establishes its behavior.
