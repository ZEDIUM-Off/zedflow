---
description: Read when you decide how to adopt the qualified packages or reproduce the qualification evidence.
---

# Qualification results

## Read the verdict

Read [observed.yaml](observed.yaml) for package verdicts, measured results, commit history and remaining integration work.
Use the [matrix](qualification-matrix.json) to trace each case to its assertions, sources and evidence.
Keep the [product requirements](../product/DOMAIN.md) separate from demonstrated capabilities.
Treat passing cases as bounded evidence rather than complete product acceptance.

## Read negative qualifications

Inspect `qualification_audit_followup` in the observations before claiming Pi session equivalence.
Treat a passed qualification case as a completed investigation; read its verdict before assuming behavioral parity.
Resolve the recorded summarization retry, post-success compaction and edited-context recovery gaps before relying on those behaviors.
Verify provider authorization against the original role-labelled session entries referenced in the observations.

## Choose the execution foundation

Keep XState authoritative for execution, cancellation and continuation.
Start from the native lane before introducing optional Agent helpers.
Keep incompatible XState generations isolated.
Choose one authoritative history before attaching package-owned memory.
Assess Graph transformations for identity loss before using their results.
Expose Code Mode through validated, authorized capabilities with explicit budgets.

## Reproduce local checks

Bootstrap each lane with its committed lockfile before running its check command.
Run `npm run check --prefix experiments/stately-native` for native experiments and recorded live-result consistency.
Run `npm run check --prefix experiments/stately-agent-stable` for the stable comparison.
Run `npm run check --prefix experiments/stately-agent-current` for the current comparison and its compatibility regression.
Use the [programme](qualification.md) for installation and evidence conventions.

## Reproduce provider trials

Obtain fresh authorization before executing the live runner identified in the observations.
Review the attempt ledger before authorizing another run.
Use only the authorized connection, model, synthetic data and fixture tool.
Keep ordinary checks independent of credentials and network access.
Treat missing final usage after cancellation as unknown, not zero.
Distinguish local stream cancellation from confirmed server-side termination.
Do not interpret catalog cost estimates as subscription billing.

## Interpret the evidence

Use the [loop report](loop-results.md) for differential protocol comparisons.
Use the [context report](context-results.md) for projection, compaction and lifecycle boundaries.
Use the [composition report](composition-results.md) for autonomous actors and structural insertion.
Use the [continuity report](continuity-results.md) for restoration, uncertain effects and recovery.
Use the [package report](package-results.md) for helper APIs, graph losses and VM replay.
Qualify the remaining integration and durability work before adopting these fixtures as product code.

## Delivery

Inspect the mission history for the delivered commits.
Require separate approval for integration, publication or worktree cleanup.

## Gotchas

Import the credential store without importing SDK-wide message type augmentations into the native lane.
Verify emitted provider parameters instead of assuming a client option becomes a server-enforced limit.
