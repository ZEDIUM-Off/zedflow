---
description: Read before defining the minimal Zedflow host or copying a mechanism from Pi.
---

# Define the minimal kit

Treat this analysis as an MVP proposal, not an implemented host.
Read `mvp_kit_analysis` in [observed.yaml](observed.yaml) for operator constraints, pinned sources, feature classifications and probe results.
Keep the [earlier qualification](qualification-results.md) as evidence, not a mandatory MVP backlog.

## Eliminate before implementing

Read each feature's dependency coverage, proposed placement, MVP disposition and sources.
Distinguish inherited mechanisms from application policies.
Do not equate mailboxes with steering queues, guards with sandboxing, or snapshots with effect journals.

| Concern | Recommended owner | Action |
|---|---|---|
| Transitions, hierarchy, parallelism and actor lifecycle | XState | Use native interfaces; add no scheduler. |
| Model protocols and streaming | Pi AI | Use the dependency directly. |
| Machine selection and durable run identity | Host | Associate runs with executable revisions. |
| Observation storage and discovery | Host | Organize inspection and declared domain output. |
| Declared socket | Host transport | Bind events without business commands. |
| Checkpoint storage | Host storage | Support a documented persistence profile. |
| Model, credential and tool helpers | Optional kit | Supply functions and native actor logic. |
| Conversation, context and compaction | Machines | Keep policy replaceable. |
| Approvals and software-factory methods | Machines | Treat them as uses of the kit. |
| Terminal and visual composition | Clients | Keep them outside the host. |
| Package installation and hot reload | Later tooling | Do not require them for launch. |

## Treat sessions as run records

Use one durable run identity, not a compulsory conversation container.
Associate it with the machine revision, input reference and runtime incarnations.
Reuse native machine identity and actor addressing where available.
Do not equate actor `sessionId` with durable run identity.

Let clients derive conversation views from machine-defined events.
Keep model context as a selected projection, not the entire trace.
Distinguish observation archives, native checkpoints and replay journals even when they share storage.
Report serialization failures and unsupported recovery explicitly.

## Preserve native machines

Load ordinary exported XState machines without changing their topology or event vocabulary.
Require no wrapper machine, proprietary flow schema or Pi extension factory.
Offer actors, functions and tools through ordinary modules.
Let authors invoke other machines, provide implementations or replace the root.
Keep the example conversation loop on the same loading path.
Place controlling decisions inside machines or invoked operations, not inspection callbacks.

## Keep sockets neutral

Specify framing, validation, endpoint access, ordering, disconnect behavior and backpressure.
Delegate business event schemas and interactions to machines and clients.
Distinguish observer traffic, domain emissions and replayable inputs.
Do not replay every observed internal event or impose Pi RPC commands.

## Choose persistence deliberately

Select an XState generation before finalizing interfaces.
Reuse native persistence, migration and durable-execution facilities.
Evaluate Stately Agent logs separately from its runtime, and SDK inspection before writing another codec.
Consult the root/child probes before adopting a recorder.
Do not promise universal replay from root-only recording.

Start with compatible checkpoints if sufficient.
Keep live services outside serializable context.
Refuse incompatible revisions and report uncertain effects.
Do not build a workflow platform to solve unqualified crash scenarios.

## Verify

Install the existing isolated experiment dependencies, then run offline:

```sh
node --test experiments/trace-probes/inspection.test.mjs
```

Before accepting the MVP, launch a custom non-conversational machine through the ordinary loading path.
Interact through its socket, inspect its record and reopen a supported checkpoint.
Require no new kernel interaction type or special-case code.
