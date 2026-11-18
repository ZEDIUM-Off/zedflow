---
description: Read when you implement or review the confirmed Stately qualification programme.
---

# Stately qualification

## Assignment

Qualify the [product](../product/DOMAIN.md) requirements through executable experiments, not a new runtime.
Use the [matrix](qualification-matrix.json) to select each case and its assertions.
Resolve source keys and package pins through [observed.yaml](observed.yaml).
The matrix describes tests, not a Zedflow definition format.

Follow the [Gitflow protocol](protocols/gitflow/PROTOCOL.md).
The owning session implements the confirmed goal in one isolated worktree.
Return evidence and commits to the operator in the originating conversation.
Keep one writing owner and run experiments sequentially unless the operator authorizes delegation.

The confirmed programme replaces the earlier stop-before-launch boundary.
Reuse the [composition assignment](composition-proof.md#acceptance) as the composition lot's requirements.
Its stop-before-another-experiment boundary does not restrict the confirmed programme.
Stop before integration, publication or cleanup.

## Reference contract

Reproduce observable Pi behavior rather than its storage format or internal classes.
Keep the Pi loop as a test oracle, not the experimental execution engine.
Distinguish turn completion, loop completion and final settlement.
Preserve errors and recovery attempts in inspection even when model context omits them.

Use native XState definitions and explicit actor connections.
Keep model calls and tool effects bounded inside actors.
Keep continuation decisions in visible machine states.
Use small typed derivation functions instead of a general composition language.

## Package lanes

| Lane | Purpose |
|---|---|
| Native | Establish the behavior with XState, Pi AI and optional structural Graph operations. |
| Stable Agent | Compare the stable Agent API against the same simulated fixture. |
| Current Agent | Test the documented Agent generation in its compatible, isolated XState package. |
| Oracle | Execute reference Pi behavior only inside tests with injected models and fixture tools. |

Pin direct dependencies exactly and commit each lockfile.
Read the pinned package's declarations before applying examples from changing online documentation.
Do not merge incompatible major versions into one runtime.
Assess Graph topology separately from executable statechart behavior.
Assess Code Mode capability calls separately from machine persistence.

## Run a lane

Bootstrap dependencies before running a package's checks:

```sh
npm run bootstrap --prefix experiments/stately-native
npm run check --prefix experiments/stately-native
```

Select comparison package paths from the setup observations.
Bootstrap keeps downloaded dependencies and caches in `.artifacts/` and reuses the committed lockfile.
Inspect the recorded typing limitation before choosing the current Agent lane's compiler options.

## Evidence

Mark cases as planned until executable evidence supports another status.
Attach test identifiers and durable observations to each conclusion.
Keep generated logs in the active worktree's `.artifacts/`.
Compare normalized event ordering, messages, effects and termination outcomes against Pi where possible.
Record source inspection separately from executed comparisons.

Test cancellation, malformed input, conflicts, recovery and uncertain effects alongside successful paths.
Preserve tool-call associations and resource provenance across context projection.
Treat snapshots of machine state and journals of external effects as distinct evidence.
Bound graph exploration explicitly.

## Boundaries

Exclude TUI reproduction, plugin-discovery compatibility, product packaging and a bidirectional editor.
Keep lifecycle and policy extension seams in scope.
Ask before excluding another significant reference behavior.
Limit each question to two implementation approaches before requesting a decision.

Use simulated models until the operator approves real-call credentials, provider, sent data and limits.
Confine tools to disposable fixtures.
Do not read personal runtime data or infer authorization from available credentials.
An unexecuted real-call case remains pending.

## Delivery

Run tests and type checks for each implemented lot.
Report adoption verdicts, remaining adapter code and unsupported behavior per package.
Keep experiment success separate from adoption into product code.
