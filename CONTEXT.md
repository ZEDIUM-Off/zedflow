# Zedflow

## Development sequence

Zedflow is developed in two non-overlapping stages:

1. **Current stage — faithful Pi port.** Port the frozen Pi TypeScript runtime package by package into matching Rust crates. Pi behavior and deterministic tests are authoritative; Zedflow-specific runtime behavior is out of scope.
2. **Deferred stage — Zedflow product.** After the port is complete and fidelity is validated on one recorded SHA, implement the graph-native product described below with LangGraph.

The language in this document defines the stage-2 product target. It must not be read as authorization to skip or redesign the stage-1 Pi runtime port.

Zedflow is a graph-native coding-agent harness for controlling agentic behavior through specialized flows, while preserving Pi session semantics through LangGraph bindings.

## Language

**Flow**:
A specialized graph-native coding-agent behavior composed to control agent execution; external Flows may be deterministic and contain no model node.
_Avoid_: Monolithic agent loop, prompt-only workflow

**Root Flow**:
The graph-native port of Pi's agentic loop, responsible for routing ordinary agent behavior and plugging external Flows into the run.
_Avoid_: External flow, generic flow, monolithic loop

**Runtime Graph**:
The final graph produced from the native Root Flow plus all active Flow Bundles and Flow Bridges; it is the final validation surface and may compile to one giant LangGraph graph, LangGraph subgraphs, or another native LangGraph composition shape.
_Avoid_: Flow bundle, root composition, plugin pile, mandatory LangGraph subgraphs

**Bundle Validation**:
Isolated validation of a Flow Bundle by composing it with the native Root Flow as a minimal test Runtime Graph.
_Avoid_: Installed-runtime validation, package lint only

**Runtime Graph Validation**:
Validation of the full active Runtime Graph produced from all enabled bundles, bridges, policies, and config, surfacing conflicts and warnings across the complete installed composition.
_Avoid_: Single-bundle validation, isolated package check

**Runtime Event Interface**:
The event surface that aligns LangGraph runtime streams with native Pi event and hook semantics where possible, while introducing Zedflow-specific runtime events for graph execution, traces, checkpoints, interrupts, and UI surfaces that Pi does not natively model.
_Avoid_: Flow bridge, Pi-only event conversion, TUI-only adapter

**LangGraph Sidecar Server**:
A local LangGraph API server managed by Zedflow to host the compiled Runtime Graph while Rust remains the product runtime and execution gateway.
_Avoid_: Ad hoc Python process, primary product runtime

**Runtime Adapter**:
The Rust-side interface to the LangGraph Sidecar Server for compiling the Runtime Graph, starting, resuming, forking, and draining runs, streaming events, restoring checkpoints, and dispatching Rust model and tool execution.
_Avoid_: Generic backend abstraction, direct UI-sidecar coupling

**Graph Drain**:
A cooperative stop of a running Runtime Graph at a LangGraph superstep boundary, preserving a resumable checkpoint.
_Avoid_: Budget interrupt, hard kill, reload

**Zedflow Store**:
A durable cross-run memory or resource store, scoped to user or project, exposed to Flows through Zedflow APIs; SQLite-backed in the MVP and pluggable later.
_Avoid_: Pi session, LangGraph checkpoint, hidden memory

**Store Provider**:
An extension-provided backend or adapter for a Zedflow Store.
_Avoid_: Hardcoded store backend

**Zedflow Extension Surface**:
The APIs, hooks, and resources that let extensions contribute stores, UI, event hooks, Flow Bundles, Flow Bridges, providers, and store providers.
_Avoid_: Flow-only plugin API, closed runtime

**Runtime Graph Scope**:
The set of Zedflow resources resolved for a run or project, including the native Root Flow, enabled Flow Bundles, Flow Bridges, policies, and config, that are compiled into the Runtime Graph.
_Avoid_: Flow workspace, single bundle, implicit resource set

**Runtime Graph Scope Resolution**:
The process where the runtime asks the Zedflow API to resolve declared resources for a given user/project scope into a validated Runtime Graph composition.
_Avoid_: Load-order merge, implicit filesystem scan

**External Flow**:
A specialized Flow plugged into the Root Flow; it may be workflow-like, agent-like, mixed, or deterministic without model nodes. It is a Zedflow composition concept, not necessarily a LangGraph subgraph at runtime.
_Avoid_: Root flow, plugin script, tool macro, LangGraph subgraph primitive

**Standalone Flow**:
A Flow that can run by itself, headless or interactive, with its own input and output contract.
_Avoid_: Embedded-only graph

**Embeddable Flow**:
A Flow that can be invoked from another Flow through a stable input and output contract.
_Avoid_: Root-only flow

**Composable Flow**:
A Flow designed to be connected with other independently defined Flows through explicit bridges.
_Avoid_: Hardwired flow

**Flow Bridge**:
A separately defined user connection between independently defined Flows; any number of bridges may connect the same Flows through different paths or conditions as long as the resulting composition validates. Multiple matching bridges are valid when the composition declares the intended branching or parallelism behavior.
_Avoid_: Flow link, implicit handoff, source-flow modification

**Flow Composition**:
An assembled set of Flows and Flow Bridges; the final validation surface is the total composition of all flows bridged around the Root Flow.
_Avoid_: Single flow, unvalidated plugin pile

**Flow Bundle**:
A saved, shareable, plug-and-play artifact containing either one Flow or a packaged Flow Composition, plus its intended composition point toward the Root Flow.
_Avoid_: Final validation surface, JSON-only definition, diagram-only export

**Flow Diagram**:
A generated visual representation of a Flow, Flow Bundle, or Runtime Graph for understanding, review, and sharing, with Mermaid and SVG as default outputs.
_Avoid_: Source of truth, editable graph runtime

**Zedflow Validation Tooling**:
Commands and APIs that validate Flow Bundles and Runtime Graphs, emit structured diagnostics, and generate diagrams for human, agent, and future LSP workflows.
_Avoid_: Visual editor, runtime-only validation

**Flow Definition**:
A saved graph definition file or bundle produced through Zedflow's Flow Composition API, defining a Flow or Flow Composition that can be authored by humans, agents, or utilities and compiled/transposed to LangGraph.
_Avoid_: JSON-first DSL, runtime-only code, vague graph file, hand-written config language

**Zedflow API**:
The strongly typed API surface used by flows, bundles, extensions, and the runtime to declare resources, compose flows, resolve scopes, validate Runtime Graphs, generate diagrams, and integrate stores, events, and UI.
_Avoid_: Loose plugin registry, JSON-only config surface

**Flow Composition API**:
The Zedflow API's code-facing composition layer, close to LangGraph but enriched with Zedflow primitives for bridging, state contracts, stores, interrupt shapes, parallelism, Rust model/tool dispatch, session binding, budgets, and diagram generation.
_Avoid_: JSON-only DSL, unrelated orchestration API, recoding the harness per flow

**Graph-Native Agent Loop**:
An agent loop whose orchestration is expressed as explicit graph control flow rather than hidden in a monolithic model-driven loop.
_Avoid_: Monolithic agent loop, prompt-only orchestration

**Human Input Interrupt Boundary**:
A flow boundary where execution is paused specifically waiting for user input, regardless of UI surface; normal user input, human commands, Pi-native commands, and skills enter through this boundary.
_Avoid_: Command boundary, TUI-only input, mid-node command injection

**Interrupt Node**:
A pure input-waiting node that creates a Human Input Interrupt Boundary without performing durable actions, tool execution, model calls, file writes, or spawned runs.
_Avoid_: Action node, approval executor, mixed interrupt/tool node

**Interrupt Shape**:
The input contract exposed by an Interrupt Node, such as free-text prompt, confirmation, single-choice, multi-choice, review/edit, selection, or structured form.
_Avoid_: TUI widget, action semantics

**Breakpoint**:
A declared non-HITL pause or stop point in a Flow, node, edge, or branch, used for debug, runtime control, or collecting a completed parallel branch result without asking for human input.
_Avoid_: Human input interrupt, normal end node, hidden tool approval

**Human Return Budget**:
An explicit Flow or Run policy limiting how long or how far execution may proceed before reaching a Human Input Interrupt Boundary, measured by nodes, duration, model calls, tool executions, cost, tokens, or a combination. Flow Diagrams display declared budgets, and runtime reports paths that exceed them.
_Avoid_: Command boundary, headless marker, fixed step limit, hidden runtime cap

**Human Return Distance**:
The graph distance from a node or path to the next Human Input Interrupt Boundary, computed statically when possible and checked at runtime otherwise.
_Avoid_: Command distance, generic graph depth

**Interactive Flow**:
A Flow that has or can reach a Human Input Interrupt Boundary through its composition; interactivity is inferred from the graph, not configured with a flag. The native Root Flow is interactive by definition.
_Avoid_: Interactive flag, TUI-only flow

**Budget Interrupt**:
A forced Human Input Interrupt Boundary caused by Human Return Budget exhaustion; it checkpoints state and pauses for user control rather than treating the budget hit as a technical error.
_Avoid_: Timeout error, silent abort

**Harness Determinism**:
Deterministic control structure around nondeterministic model and tool calls; the Flow decides action order, error paths, model invocation points, routing conditions, and exposed context.
_Avoid_: Deterministic LLM output, seed-only determinism

**Tool Call**:
A model-produced request to execute a tool.
_Avoid_: Tool execution, command step

**Tool Exposure Policy**:
The declaration of which Rust tools are visible or callable at a given model or flow step, including flow-local aliases or runtime-resolved tool names.
_Avoid_: Global execution authorization, all-tools-by-default

**Tool Execution**:
The actual Rust-side execution of a tool or command, whether caused by a Tool Call or by deterministic Flow logic.
_Avoid_: Tool call

**Flow State**:
The LangGraph-aligned state schema associated with a Flow through the Zedflow API, including explicit keys, expected inputs, outputs, reducers, and private/internal channels.
_Avoid_: Separate Zedflow runtime state system, hidden state bag

**State Transition Contract**:
The declared schema of Flow State a node reads and writes when passing information to other nodes, compiled and validated against LangGraph state schemas and reducers; composition APIs may only reference state keys that are available at that composition point.
_Avoid_: Implicit state shape, undocumented node coupling, separate state model

**Logical State Key**:
The Flow State key name declared by a Flow author through the Zedflow API before runtime namespacing, branching, or collision resolution.
_Avoid_: Runtime key, checkpoint key, hidden generated name

**Resolved State Key**:
The actual state key used in the Runtime Graph after Zedflow applies namespacing, branch separation, collision handling, or Root Flow reservations to a Logical State Key.
_Avoid_: Author-facing key, arbitrary alias, untracked rename

**Runtime State Key**:
A Resolved State Key generated or reserved for the Runtime Graph to avoid accidental collisions between independently authored Flows or Bundles while preserving explicit Zedflow API-declared Logical State Keys.
_Avoid_: User-facing flow ID, shared global state name, implicit global key

**State Key Visibility**:
A Flow State key's composition-facing exposure as either public or private; public keys may be composed across Flows where available, while private keys are restricted to the Flow or narrower declared scope that owns them.
_Avoid_: Runtime permission, security boundary, hidden global state, separate shared/scoped category

**State Key Availability**:
The path-sensitive guarantee that a Flow State key exists at a specific graph composition point because upstream nodes on that path have produced it or a default/guard makes it valid; the Composition API exposes only keys resolved as visible and available at the selected composition point.
_Avoid_: Schema membership, optional guess, best-effort lookup

**State Key Scope**:
A composition-facing access boundary declaring where a Flow State key may be read or written, such as across the whole Flow, only selected nodes, only selected edges, or through declared bridge points; scope refines public/private visibility rather than adding a separate visibility class.
_Avoid_: Security sandbox, runtime authorization, accidental convention

**Node State Surface**:
The resolved set of Flow State keys exposed by a specific node, edge, or branch as usable composition values, including directly produced keys plus other public keys visible and available at that point.
_Avoid_: Whole flow state, raw internal state, undeclared node output

**Parallel State Write Conflict**:
A composition case where parallel branches intend to write the same public Flow State key; Zedflow resolves this by requiring an explicit parallel-write declaration, usually by giving each branch a distinct resolved key and requiring an explicit read/merge strategy before the shared value is consumed.
_Avoid_: Sequential overwrite, ordinary state update, hidden race, implicit reducer

**State Reducer**:
A LangGraph-aligned reducer declared for a Resolved State Key, either with the Flow State key or at a custom Flow composition/bridge point, defining how multiple writes to that key are merged, especially during parallel fan-in.
_Avoid_: Branch policy, read selector, implicit last-writer-wins

**Parallel Branch Policy**:
A composition declaration for parallel execution behavior, such as waiting for all branches, taking the first successful branch, cancelling slower branches, or timing out branches.
_Avoid_: State reducer, read strategy, accidental fan-out

**State Read Strategy**:
A composition declaration for how downstream nodes read state when multiple Resolved State Keys correspond to the same Logical State Key, such as reading a specific namespace/key, selecting a winner, or reading a merged value.
_Avoid_: State reducer, branch scheduling, hidden lookup

**Parallel Write Strategy**:
An explicit API declaration that a Flow Bundle or composition intentionally permits parallel writes related to the same Logical State Key, combining a required State Reducer with a Parallel Branch Policy or State Read Strategy as needed; Runtime Graph Validation fails compilation when parallel writes have no reducer.
_Avoid_: Accidental reducer, hidden last-writer-wins, implicit fan-in, warning-only race

**Pi-Compatible Error**:
A model, tool, or provider error represented using existing Pi-style user and session semantics where possible.
_Avoid_: Graph-only error wrapping

**Graph Runtime Error**:
An error from graph execution, checkpoints, interrupts, state contracts, routing, timeout, retry exhaustion, subgraph composition, or runtime adapter behavior.
_Avoid_: Provider error, normal budget interrupt

**Failure Path**:
An explicit route taken when a Pi-Compatible Error or Graph Runtime Error is handled by the Flow after configured retry, timeout, or error handling.
_Avoid_: Silent abort, budget interrupt

**Validation Failure Path**:
An explicit route taken when node output cannot satisfy the State Transition Contract required by downstream nodes.
_Avoid_: Silent coercion, best-effort parsing

**Routable State**:
Typed Flow State intended to be consumed by Router Nodes.
_Avoid_: Raw tool output, arbitrary blob

**Routing Input**:
A Routable State field declared as required by a Router Node condition.
_Avoid_: Implicit router argument, undeclared condition data

**Context Source**:
A named source of context available to a model node, such as the Pi system prompt, AGENTS.md, current user input, session history slice, file, URL, store item, tool result, or custom prompt block.
_Avoid_: Model-decided retrieval, anonymous prompt blob

**Context Assembly Policy**:
A model-node setting that declares exactly which Context Sources are included or excluded, with tags, provenance, and ordering.
_Avoid_: Ad hoc prompt blob, implicit full-context exposure

**Graph Messages State**:
The LangGraph Flow State channel that carries conversation messages for model use and routing, using a message-aware reducer; Pi JSONL may persist these messages for session UX, but message control belongs to the graph state.
_Avoid_: Pi-only transcript, duplicated hidden history, append-only blob

**Structured Response**:
A model-node setting that requests schema-validated output through provider-native structured output or tool-calling strategy.
_Avoid_: Free-text parsing, Zedflow-specific model state

**Normalization Node**:
A node that converts raw node output into typed Routable State, either deterministically, through a model node with Structured Response enabled, or through a specialized tool/command.
_Avoid_: Hidden router logic, implicit parser

**Routing Condition**:
A predicate evaluated by a Router Node over declared Routing Inputs to choose the next path.
_Avoid_: Model path request, hidden command

**Branching Policy**:
A Router Node or Flow Bridge declaration specifying whether matching routes are exclusive, ordered by priority, all taken, parallelized, or launched as spawned async runs. Parallelism is an MVP routing behavior, not a dedicated parallel node.
_Avoid_: Parallel node, implicit multi-match behavior

**Router Decision**:
The selected next node or External Flow after a Router Node evaluates routing conditions, policy, and state.
_Avoid_: Model decision, raw tool call

**Router Node**:
A node that only evaluates existing state and produces a Router Decision; it does not execute tools, commands, semantic scoring, or project inspection itself.
_Avoid_: Hidden tool executor, mixed action-router

**Zedflow Session**:
The session system that pairs Pi Session tree behavior with LangGraph checkpoint and time-travel semantics.
_Avoid_: Product core, replacement Pi session

**Flow Run**:
A concrete execution of a Flow Definition.
_Avoid_: Session, LangGraph thread

**Pi Session**:
The durable record of a Pi run, including the session tree that users can navigate.
_Avoid_: Transcript, chat log

**Execution Binding**:
The complete sidecar persistence association between every Pi Session entry and the LangGraph execution context that produced or consumed it, stored outside the Pi Session entry and outside composable Flow State, keyed by Pi Session entry ID. Zedflow restores flow state by looking up the selected session-tree entry's binding; entries without bindings remain Pi-navigable but cannot restore flow state.
_Avoid_: Inline session field, Flow State key, restore binding, flow reference, provenance note

**Bound Range**:
The contiguous part of a Pi Session tree whose entries have Execution Bindings and can restore Flow Run state. A new Bound Range can start when the user resumes from an older unbound point and sends new input.
_Avoid_: Full session, restore range, retro-binding

**Unbound Session Entry**:
A Pi Session entry without an Execution Binding because it predates Zedflow flow execution or belongs to a session without Zedflow sidecar state; it remains normal Pi history and is not backfilled with synthetic flow state.
_Avoid_: Broken binding, invalid entry, incomplete write

**Incomplete Bound Entry**:
A Zedflow-created Pi Session entry that should have an Execution Binding but does not because a durability unit was interrupted; recovery may repair it, otherwise it remains Pi-readable but graph-unrestorable with a warning.
_Avoid_: Old session entry, normal unbound entry, silent corruption

**Run Reference**:
A cross-run link from one Pi Session or Flow Run to another Flow Run, without implying ownership of the referenced run's state.
_Avoid_: Child checkpoint ownership, inline child session

**Session Branch**:
A branch in the Pi Session tree created by Pi's existing history navigation behavior when the user continues from a non-leaf entry.
_Avoid_: Graph branch

**Resume**:
Continuing execution from the current bound leaf of a Pi Session.
_Avoid_: Fork, rollback

**Fork**:
Continuing from an earlier bound Pi Session entry by pairing Pi's existing session-branch behavior with LangGraph's native time-travel fork semantics. Forks preserve later Pi entries and LangGraph checkpoints while re-executing graph work after the selected checkpoint.
_Avoid_: Resume, destructive rewind, rollback, custom fork runtime

**Attached Path**:
The default branching path behavior where execution remains attached to the current Flow Run, sharing its state, session bindings, and checkpoints.
_Avoid_: Session branch, detached run

**Detached Run**:
An optional branching behavior that launches a separate Flow Run with its own Pi Session, checkpoints, and runtime state, while the parent keeps a Run Reference and may consume a result if declared.
_Avoid_: Headless mode, attached path, graph branch

**Wait Mode**:
The sync or async behavior of a branching path, whether attached to the current run or launched as a Detached Run.
_Avoid_: Run scope, headless mode

**Run Lineage**:
The sidecar ancestry relationship between Flow Runs created by forks or detached runs, linking new and parent flow run IDs, LangGraph thread/checkpoint IDs, source Pi Session entry IDs, and the action that created the relationship.
_Avoid_: Session tree, rollback, inline JSONL branch
