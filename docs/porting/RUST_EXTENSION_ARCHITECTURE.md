# Rust extension architecture v1

## Scope

This Stage-1 port exposes Pi's ExtensionAPI as native Rust, in-process contracts in `zedflow-coding-agent`. It supports registration of tools, commands, shortcuts, flags and providers; lifecycle, input, session, compaction, provider and UI events; session actions; errors; stale-context invalidation; and idempotent shutdown.

Extensions are not dynamically loaded or source-installed in this revision. All extension types remain in the coding-agent compilation boundary. Dynamic libraries, ABI design, artifact trust and TypeScript/jiti installation are later work, not behavior implied by this port.

## Contract

`ExtensionRuntime` is the registration surface. Tools and commands have executable native handlers. `ExtensionRunner` owns handlers and dispatches events in registration order. A failing handler reports an `ExtensionError` and does not prevent a later handler from running.

The runner owns an `ExtensionContext`. Reload/session replacement invalidates its generation; shutdown emits `SessionShutdown` once and then marks it stale. Tool and command invocation reject a stale context. Input handlers may replace input and stop propagation with `{ "replacement": "…", "consume": true }`.

UI and component requests are represented as events in this in-process contract. Host modes decide whether UI is available through `ExtensionContext::has_ui`; no terminal implementation is duplicated in the extension layer.

## Deferred

Add a dynamic loading boundary only when a separately compiled extension SDK is required. That work needs an explicit ABI, artifact provenance/trust policy, loader isolation and fixture gates; it must not leak foreign pointers or dynamic-loading concerns into this native contract.
