# Zedflow MVP PRD

## Product framing

**Zedflow is a graph-native coding-agent harness.**

Zedflow builds a Rust execution gateway around a graph-native Root Flow hosted by official LangGraph. Rust remains the product runtime, execution gateway, and user-facing harness. LangGraph provides graph execution, checkpoints, interrupts, time travel, and runtime state.

The product goal is not merely to bolt LangGraph onto Pi. Zedflow shifts agent behavior from prompt-driven autonomy toward explicit, composable harness control: specialized flows decide action order, model invocation points, exposed context, tool exposure, error paths, interrupts, and parallel routing.

This document is the **stage-2 product brief**. Stage 2 begins only after the complete Pi TypeScript-to-Rust port has passed the stage-1 fidelity gate defined in `docs/porting/BASELINE.md`. Until then, this document is a target architecture, not the current implementation roadmap.

---

## Problem statement

Pi is highly extensible, but its core agent loop remains an implicit monolithic runtime. Power users and extension authors can add tools, skills, prompts, and hooks, but they cannot compose the loop itself as a graph of inspectable, reusable, testable flow units.

Zedflow should let users compose graph-native coding-agent behavior.

---

## Core product promise

Zedflow replaces a prompt-driven monolithic agent loop with specialized graph-native **Flows**, defined through a strongly typed **Zedflow API**, so agent behavior becomes more deterministic, inspectable, composable, and locally runnable.

---

## Canonical architecture

### Rust remains the product runtime

Rust owns:

- provider/model execution
- tool execution
- Pi-compatible sessions
- config/resource loading
- TUI/RPC/user-facing surfaces
- runtime adapter coordination
- extension integration

### LangGraph provides the graph runtime

The MVP uses a local **LangGraph Sidecar Server** managed by Zedflow. It hosts the compiled **Runtime Graph** and provides:

- graph execution
- checkpointing
- interrupts
- time travel / fork / replay capability
- streaming runtime events
- graph persistence semantics

### Runtime Adapter

The Rust-side **Runtime Adapter** talks to the LangGraph Sidecar Server to:

- resolve and compile the Runtime Graph
- start, resume, fork, replay, and drain runs
- stream runtime events
- restore checkpoints
- dispatch model/tool execution back into Rust
- coordinate session bindings

---

## Flow model

### Root Flow

The first runtime feature is the native **Root Flow**: a graph-native port of Pi's current agentic loop.

The Root Flow is internal native code, not a user bundle, but it must be inspectable and diagrammable as a Flow. It is the anchor around which bundles and bridges are composed.

The Root Flow preserves Pi-like behavior:

- user input
- model invocation
- tool-call detection
- tool execution
- tool result propagation
- loop back to model
- assistant output
- session/tree behavior

### External Flows

External Flows are specialized graph-native behaviors plugged into the Root Flow. They may be workflow-like, agent-like, mixed, or fully deterministic with no model node.

A Flow is a Zedflow composition concept, not necessarily a LangGraph subgraph. During compilation, Zedflow may produce one giant LangGraph graph, LangGraph subgraphs, wrapper nodes, conditional edges, `Send`, `Command`, or another native LangGraph shape.

### Flow Bundles

A **Flow Bundle** is a saved, shareable, plug-and-play artifact containing one Flow or a packaged Flow Composition, plus its intended composition point toward the Root Flow.

A bundle should include enough metadata/contracts/diagrams for humans and agents to understand, validate, share, and compose it.

### Flow Bridges

A **Flow Bridge** is a separately defined connection between independently defined Flows. Any number of bridges may connect the same Flows through different paths or conditions as long as the resulting composition validates.

Multiple matching bridges are valid when the composition declares the intended branching or parallelism behavior.

### Runtime Graph

The **Runtime Graph** is the final graph produced from:

- native Root Flow
- active Flow Bundles
- active Flow Bridges
- runtime policies/config

It is the final validation surface and the graph hosted by the LangGraph Sidecar Server.

### Runtime Graph Scope Resolution

The runtime does not merge resources by a dumb load order. It asks the strongly typed Zedflow API to resolve declared resources for the current user/project scope into a validated Runtime Graph composition.

---

## Zedflow API

The **Zedflow API** is the strongly typed API surface used by flows, bundles, extensions, and the runtime to declare resources, compose flows, resolve scopes, validate Runtime Graphs, generate diagrams, and integrate stores, events, and UI.

The **Flow Composition API** is close to LangGraph but enriched with Zedflow primitives for:

- Flow Bundles and Flow Bridges
- state contracts
- stores
- interrupt shapes
- branching policy and parallelism
- Rust model/tool dispatch
- session binding
- human return budgets
- diagram generation

Zedflow must not invent a large JSON DSL or force users to recode the harness for every Flow. JSON may exist as an internal/serialized artifact, but the product direction is API-first, not JSON-first.

---

## State and routing

### State Transition Contracts

Nodes declare the state they read and write through **State Transition Contracts**. Validation checks that downstream nodes receive compatible state.

### Runtime State Keys

Runtime state keys may be generated/resolved to avoid accidental collisions between independently authored Flows or Bundles. Naming collisions are not automatically errors if the composed Runtime Graph can disambiguate them.

### Routers

A **Router Node** evaluates existing state only. It does not execute tools, commands, model calls, semantic scoring, project inspection, or other durable actions.

If routing needs derived data, a previous node must produce it, possibly through a **Normalization Node**.

### Routable State

Routers consume **Routable State**: typed state intended for routing. Raw tool/model output should be normalized before routing unless it is already a typed standard signal such as tool-call presence.

### Branching Policy and parallelism

Parallelism is an MVP routing behavior, not a dedicated `parallel` node.

Router/Bridge composition points declare a **Branching Policy**, such as:

- exclusive route
- ordered priority
- all matching routes
- parallelized routes
- detached run launch

### Attached paths and Detached Runs

Default branching behavior is an **Attached Path**: execution remains attached to the current Flow Run, sharing state, session bindings, and checkpoints.

A **Detached Run** explicitly launches a separate Flow Run with its own Pi Session, checkpoints, and runtime state. The parent keeps a Run Reference and may consume the result if declared.

**Wait Mode** (`sync` / `async`) is orthogonal and applies to either attached paths or detached runs.

---

## Model nodes and context

A model node is optional in External Flows. A Flow is not an LLM-call wrapper.

A model node may configure:

- provider/model selection
- thinking level
- Tool Exposure Policy
- Context Assembly Policy
- Structured Response
- streaming behavior

### Context Assembly Policy

A **Context Assembly Policy** declares exactly which **Context Sources** are included/excluded for a model invocation, with tags, provenance, and ordering.

Context Sources may include:

- Pi system prompt
- AGENTS.md
- current user input
- selected session history
- files
- URLs
- store items
- tool results
- custom prompt blocks

This lets flows expose precise context without depending on the model to discover everything through tool calls.

### Structured Response

A **Structured Response** is a model-node setting requesting schema-validated output through provider-native structured output or tool-calling strategy. `auto` should be the default strategy, with explicit provider/tool strategy available for custom or local models.

If model output is used for semantic routing, it must be structured or normalized before reaching a Router Node.

---

## Tools

Tool execution remains in Rust.

Zedflow distinguishes:

- **Tool Call** — model-produced request to execute a tool
- **Tool Execution** — actual Rust-side execution, whether caused by a Tool Call or deterministic Flow logic
- **Tool Exposure Policy** — declaration of which Rust tools are visible/callable at a given model or flow step

A model may request a tool call, but the Flow/Router decides whether to execute a Rust tool, enter an External Flow, interrupt for approval, normalize, fail, retry, or route elsewhere.

---

## Interrupts and human input

### Human Input Interrupt Boundary

A **Human Input Interrupt Boundary** is a flow boundary where execution pauses specifically waiting for user input, regardless of UI surface.

Normal user input, human commands, Pi-native commands, and skills enter through this boundary.

### Interrupt Nodes

An **Interrupt Node** is a pure input-waiting node. It does not perform durable actions, tool execution, model calls, file writes, or detached run launches.

Actions before/after human input must be separate nodes.

### Interrupt Shapes

An **Interrupt Shape** declares the expected human input contract, such as:

- free-text prompt
- confirmation
- single-choice
- multi-choice
- review/edit
- selection
- structured form

### Human Return Budget

A **Human Return Budget** limits how long or far a Flow may execute before reaching a Human Input Interrupt Boundary. It may be measured by nodes, duration, model calls, tool executions, cost, tokens, or a combination.

A **Human Return Distance** can be computed from the graph when possible. Budget excess creates warnings at validation/runtime and may trigger a **Budget Interrupt** rather than a technical error.

---

## Sessions and persistence

### Pi Sessions remain Pi Sessions

Zedflow preserves Pi session semantics. A Pi Session remains the durable record of a Pi run and its session tree.

Zedflow adds sidecar **Execution Bindings** rather than writing Zedflow-specific fields inline into Pi JSONL session entries.

### Execution Bindings

Every Zedflow-created Pi Session entry inside a **Bound Range** has an Execution Binding to LangGraph execution context.

Bindings are keyed by Pi Session entry ID and include the information needed to restore the corresponding LangGraph state, such as flow/run identity, node, checkpoint, namespace, and step context.

Old/unbound Pi entries remain Pi-navigable without retroactive binding. If the user navigates to one and sends new input, Zedflow starts a new Flow Run and creates a new Bound Range from that point forward.

### Durability unit

For Zedflow-created entries, Pi session persistence, Execution Binding persistence, and LangGraph checkpoint persistence form one logical durability unit. On recovery, incomplete units are repaired or ignored as incomplete writes.

### Fork, resume, replay

- **Resume** = continue from the current bound leaf.
- **Fork** = Pi session branch + LangGraph native time-travel fork from the selected checkpoint.
- **Replay** = LangGraph time-travel capability available to runtime/debug/failure paths and optionally as a `/tree` option for bound entries.

Fork preserves old Pi entries and old LangGraph checkpoints while re-executing graph work after the selected checkpoint.

---

## Runtime events

The **Runtime Event Interface** aligns LangGraph runtime streams with native Pi event/hook semantics where possible, while introducing Zedflow-specific runtime events for graph execution, traces, checkpoints, interrupts, and UI surfaces that Pi does not natively model.

LangGraph stream inputs include:

- messages
- updates
- values
- custom events
- checkpoints
- tasks
- debug/subgraph namespaces
- interrupts

Zedflow TUI/RPC will need Zedflow-specific adaptations; the current Pi TUI should not be assumed to display every graph event natively.

---

## Stores and extensions

### Zedflow Stores

A **Zedflow Store** is durable cross-run memory or resource storage scoped to user or project. SQLite is the MVP backend for simplicity.

Later, extensions may provide custom Store Providers, such as an Obsidian vault provider with semantic querying.

### Zedflow Extension Surface

Extensions may contribute:

- stores and store providers
- UI surfaces
- event hooks
- Flow Bundles
- Flow Bridges
- model/tool providers
- Runtime Event Interface hooks

The extension model should remain interfaceable and strongly typed through the Zedflow API.

---

## Errors and fault tolerance

Zedflow preserves Pi-style model/tool/provider error presentation where possible, while adding typed graph runtime errors.

- **Pi-Compatible Error** — model/tool/provider error represented using existing Pi-style user/session semantics where possible.
- **Graph Runtime Error** — error from graph execution, checkpoints, interrupts, state contracts, routing, timeout, retry exhaustion, composition, or runtime adapter behavior.
- **Failure Path** — explicit route taken when such an error is handled by the Flow after configured retry/timeout/error handling.
- **Validation Failure Path** — explicit route when node output cannot satisfy a State Transition Contract.

Budget Interrupts and Graph Drains are not failures.

### Graph Drain

A **Graph Drain** is a cooperative stop of a running Runtime Graph at a LangGraph superstep boundary, preserving a resumable checkpoint. It is used for shutdown, sidecar restart, controlled stop, and possibly reload coordination.

---

## Validation and diagrams

### Bundle Validation

**Bundle Validation** validates a Flow Bundle in isolation by composing it with the native Root Flow as a minimal test Runtime Graph.

### Runtime Graph Validation

**Runtime Graph Validation** validates the full active Runtime Graph produced from all enabled bundles, bridges, policies, and config, surfacing conflicts and warnings across the complete installed composition.

Validation should detect or warn about:

- missing bridge targets
- state contract incompatibility
- unresolved routing ambiguity where no branching policy is declared
- unsafe cycles or budget excess
- async/detached behavior that is not explicit
- tool exposure ambiguity
- invalid interrupt shapes
- unsupported structured response strategy

### Flow Diagrams

A **Flow Diagram** is generated for a Flow, Flow Bundle, or Runtime Graph. Mermaid and SVG are default outputs.

A visual editor is out of scope, but generated diagrams are expected validation/review artifacts.

### Zedflow Validation Tooling

Zedflow should provide commands/APIs to:

- validate Flow Bundles
- validate the active Runtime Graph
- emit structured diagnostics
- generate diagrams
- support humans, agents, and future LSP-like workflows

---

## MVP user stories

1. As a Pi power user, I want the Root Flow to preserve Pi-like agent behavior while making orchestration explicit.
2. As a flow author, I want to define specialized Flows with a strongly typed Zedflow API close to LangGraph.
3. As a flow author, I want to package Flows as shareable Flow Bundles.
4. As a user, I want to install/enable bundles and have Zedflow compose them into my Runtime Graph.
5. As a flow author, I want Flow Bridges so I can connect independently authored Flows without modifying their source.
6. As a maintainer, I want Bundle Validation and Runtime Graph Validation to catch composition problems early.
7. As a human or agent, I want generated Flow Diagrams so I can understand and repair bundles.
8. As a user, I want Pi session tree navigation to preserve LangGraph runtime state where bindings exist.
9. As a user, I want old Pi sessions to remain usable even when they have no Zedflow bindings.
10. As a user, I want forking from Pi's session tree to use LangGraph-native fork semantics.
11. As a flow author, I want precise model context assembly so I can control what the model sees.
12. As a flow author, I want model nodes to support structured response for routable decisions.
13. As a flow author, I want routers to branch/parallelize through explicit branching policy.
14. As a flow author, I want attached paths and detached runs to be explicit.
15. As a user, I want human input and commands to enter only through explicit interrupt boundaries.
16. As a user, I want budgets/warnings to prevent long uncontrolled execution away from human input.
17. As a maintainer, I want Rust-side providers/tools/sessions/TUI/RPC preserved unless a graph seam requires targeted change.
18. As an extension author, I want stores, events, UI, bundles, bridges, and providers to be interfaceable through Zedflow APIs.

---

## Stage-2 MVP milestones

These milestones are deferred until the stage-1 Pi fidelity port is complete.

### Milestone 1 — Root Flow and Zedflow API foundation

- Define Root Flow as graph-native Pi loop.
- Define Zedflow API / Flow Composition API core.
- Define Flow Bundle, Flow Bridge, Runtime Graph Scope Resolution.
- Add Bundle Validation and Flow Diagram generation stubs.

### Milestone 2 — LangGraph Sidecar Server and Runtime Adapter

- Manage local LangGraph Sidecar Server.
- Compile Runtime Graph from Zedflow API output.
- Add Runtime Adapter for start/resume/fork/replay/drain.
- Stream events through Runtime Event Interface.

### Milestone 3 — Rust model/tool dispatch

- Implement model node dispatch into Rust provider substrate.
- Implement tool execution dispatch into Rust tool substrate.
- Add Tool Exposure Policy.
- Preserve Pi-compatible model/tool error surfaces.

### Milestone 4 — Zedflow Session integration

- Add sidecar Execution Bindings.
- Co-locate Pi sessions, bindings, and LangGraph checkpoint artifacts.
- Make `/tree` navigation flow-aware.
- Implement Resume/Fork semantics using LangGraph time travel.
- Support unbound old Pi entries creating new Bound Ranges forward.

### Milestone 5 — Flow composition MVP

- Enable Flow Bundles and Flow Bridges.
- Implement Runtime Graph Validation across active bundles.
- Add Branching Policy, attached paths, detached runs, wait modes.
- Add Human Return Budget/Distance warnings and Budget Interrupt.

### Milestone 6 — Context, stores, extensions, polish

- Implement Context Assembly Policy and Context Sources.
- Add Structured Response support.
- Add SQLite-backed Zedflow Store.
- Define Store Provider and extension hooks.
- Expand diagrams, diagnostics, tests, and docs.

---

## Out of scope for MVP

- Rewriting LangGraph in Rust.
- Making Python the primary product runtime.
- Replacing Rust provider/auth/model registry substrate.
- Replacing Rust tool implementations.
- Marketplace/distribution platform for bundles.
- Rich visual graph editor.
- Full audit-grade deterministic replay beyond practical checkpoint/fork semantics.
- Strong inter-model portability guarantees.
- Arbitrary JSON DSL as the primary authoring model.
- Dynamic hot-loading flows into a running node outside interrupt/drain boundaries.

---

## Reference baselines

- `langgraph v1.2.6` remains the orchestration reference baseline.

Zedflow adopts specific LangGraph semantics deliberately: `StateGraph`, state schemas, reducers, conditional edges, `Send`, `Command`, interrupts, checkpointers, stores, streaming, time travel, subgraph composition where useful, retry/timeout/error handling, and graceful drain.

Zedflow adopts Pi semantics deliberately: sessions, session tree navigation, provider/tool execution ergonomics, local-first config/resource behavior, TUI/RPC surfaces, and extension philosophy.
