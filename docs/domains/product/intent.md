---
description: Read to understand the product needs and the execution boundary before you choose an implementation.
---

# Product intent

## Purpose

Zedflow is a graph-native harness composition surface.
The operator needs to construct, reuse, inspect, and refine behavior with explicit control over resources and model context.
The requirements below guide product work.

## Composition

The operator needs to express meaningful decisions, transitions, model calls, tool use, and human interaction in a graph.
A flow expresses a specialized behavior through that graph.
A harness combines flows with their resources and policies.
A definition describes reusable behavior; an instance identifies one use of that definition.

| Composition form | Property |
|---|---|
| Independent machines and actors | Each participant keeps its identity and communicates through explicit connections. |
| Structural extension | The operator explicitly extends a definition to produce a derived definition. |

The operator needs both forms, including cycles, conditions, parallel work, and nested behavior.
The operator needs to see the complete composition and its conflicts before execution.
Visual arrangement represents presentation; explicit connections determine behavior.
The operator needs compact views with progressive detail, keyboard access, and labels that complement color.

## Resources and model context

The operator needs to declare resource ownership, access, sharing, and lifetime within explicit scopes.
The operator needs to choose independent instances or shared resources deliberately.

| Concept | Purpose |
|---|---|
| Run state | Controls progress and continuation. |
| Store | Holds work data with an explicit lifetime and access contract. |
| Service | Supplies a capability to a flow. |
| Model context | Contains the selected input for one model call. |

The operator needs to compose reusable context strategies that select, transform, and order available sources.
The operator needs to inspect the exact prepared context and trace its parts to their sources.
The operator needs model context that preserves message roles, tool associations, and compatible media.
Resource access and model exposure require separate choices.
The operator needs to distinguish missing, empty, stale, and failed data.
The agent requests tools within the capabilities that the operator permits.

## Runs and human interaction

A run identifies one execution; an occurrence identifies one visit to a graph element.
The operator needs to connect each occurrence to its definition revision, inputs, outputs, context, and resource provenance.
The operator needs to distinguish current state, historical observations, and current external data.

The operator needs to inspect active work, human waits, control pauses, failures, and completion as distinct situations.
Human responses need a precise target and a declared shape, such as text, confirmation, selection, or structured data.
The operator needs explicit control over work budgets and return to human interaction.

The operator needs continuity independent of an interface or connection.
The operator needs to resume saved work and inspect parent and child runs from another interface.
The operator needs explicit handling of stale responses, revision changes, and uncertain external effects.

## Reuse and evolution

The operator needs reusable, versioned behavior with explicit dependencies and resource requirements.
The operator needs to compare revisions through their runs, outcomes, costs, and failure observations.
The agent can propose improvements through ordinary flows, including improvements to those flows themselves.
The operator needs to distinguish an experiment from active behavior and choose adoption explicitly.

## Execution and reuse boundary

XState owns state-machine execution.
Stately Graph offers structural graph operations; XState supplies execution semantics.
The operator can assess Pi AI and Pi Code Mode for reuse.
Stately Agent remains optional.
The operator must verify technology capabilities against these needs before adoption.
