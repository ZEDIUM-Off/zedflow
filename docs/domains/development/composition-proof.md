---
description: Read when the operator launches the independent-machine and structural-extension experiment.
---

# Composition proof

## Question

Can Zedflow combine independent machines and extend definitions while XState retains execution authority?
Use the [product intent](../product/intent.md#composition) as the requirement.

## Assignment

| Field | Contract |
|---|---|
| Identifier | `composition-proof` |
| Parent | Read the mission parent from the tracked `.gitflow`. |
| Owner | The session that opens this mission owns implementation. |
| Return route | Deliver the source commit and findings to the operator in the launching session. |
| Location | `experiments/composition-proof/` inside the mission worktree. |
| Budget | One fixture, one insertion primitive, at most two implementation approaches. |
| Stop | Deliver the evidence and verdict before integration or another experiment. |

Follow the [Gitflow protocol](protocols/gitflow/PROTOCOL.md).
Record the exact parent commit when opening the mission.
Keep one writing owner.
Arrange read-only review of the delivered revision before integration.
Require operator authorization before delegating work.

## Scope

Build a root machine that invokes two independent instances of one reusable flow.
Give each instance distinct input and observable output.
Build a derived definition that inserts approval before a simulated tool step.
Keep the original definition executable without approval.
Drive approval through explicit events in memory.
Keep control decisions and transitions in native XState machines.

Use one autonomous TypeScript experiment package.
Verify supported APIs against official documentation before implementation.
Pin exact dependency versions and commit the lockfile.
Assess Stately Graph against insertion, conflict detection and provenance.
Use its structural operations only where they remove required code.
Explain any decision to omit it from the executable proof.

Exclude persistence, model providers, Pi integration, UI, general graph rewriting and production packaging.
Do not create a scheduler or a second transition interpreter.
Do not access real models, credentials or personal runtime data.
Preserve `design/`, `LICENSE` and historical refs.

## Acceptance

| Case | Required assertion |
|---|---|
| Autonomous composition | The root observes both child results through explicit connections. |
| Instance isolation | Advancing one child leaves the other child's state and input unchanged. |
| Original behavior | The unextended definition reaches completion without approval. |
| Derived behavior | The simulated tool runs only after approval in the derived definition. |
| Immutability | Derivation leaves the source unchanged and independently executable. |
| Missing target | An unknown extension target fails before actor execution. |
| Incompatible target | A known target with an unsupported shape fails explicitly. |
| Conflict | Two insertions claiming the same point fail in either input order. |
| Provenance | Added states and connections identify their source extension and definition revision. |
| Execution authority | XState executes both definitions without a custom scheduling loop. |

## Delivery

Provide deterministic tests for every acceptance case and one command that runs them.
Provide a separate type-check command.
Store generated logs in the mission's `.artifacts/`.
Record the verdict in a development-domain annex, with measured observations in `observed.yaml` beside `DOMAIN.md`.
Distinguish proven behavior, remaining assumptions and rejected approaches.
Report the composition code required beyond native primitives.
Keep experiment adoption separate from successful tests.

Stop if the second approach fails, required APIs conflict, or execution requires a new interpreter.
Return the failing case and the smallest unresolved design question.
Commit reproducible sources and the verdict before delivery.
Wait for operator authorization before integration, cleanup or publication.
