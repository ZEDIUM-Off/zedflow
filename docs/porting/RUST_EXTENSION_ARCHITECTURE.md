# Rust extension architecture v1

## Scope

This Stage-1 port exposes Pi's ExtensionAPI through native Rust contracts in `zedflow-coding-agent`. It supports registration of tools, commands, shortcuts, flags and providers; lifecycle, input, session, compaction, provider and UI events; session actions; errors; stale-context invalidation; and idempotent shutdown.

Extensions are separately compiled Rust cdylibs loaded through custom ABI v1 for the process lifetime. Installation accepts only source inputs: `crate:<name>@<exact-version>`, `github:<owner>/<repo>@<resolved-commit>[#package]`, and development `path:` sources. Installation stages sanitized sources, builds locally, rejects prebuilt artifacts, records sha2 provenance/trust receipts, and binds the resolved source/artifact digest before load. TypeScript/jiti compatibility is deferred.

## Trust authority

A receipt and any receipt-adjacent registration are untrusted artifact metadata; equality between them never authorizes loading. Source installation must transactionally create the only load authority in the application-managed `native-extension-trust` root, a sibling of (not a sidecar in) the receipt directory. That authority binds the installed source identity and verified artifact digest, and `ResourceLoader`/default interactive startup must require its matching binding before dynamic loading. A forged matching receipt/registration pair, substituted receipt, or artifact-path substitution is rejected.

The trust root is the exclusive persisted authorization boundary for this local installation model. It must not be reconstructed from artifact-controlled files during load, and failed installation or reload must not publish a partial authorization.

## Contract

`ExtensionRuntime` is the registration surface. Tools and commands have executable native handlers. `ExtensionRunner` owns handlers and dispatches events in registration order. A failing handler reports an `ExtensionError` and does not prevent a later handler from running.

The runner owns an `ExtensionContext`. Reload/session replacement invalidates its generation; shutdown emits `SessionShutdown` once and then marks it stale. Tool and command invocation reject a stale context. Input handlers may replace input and stop propagation with `{ "replacement": "…", "consume": true }`.

UI and component requests are represented as events in this in-process contract. Host modes decide whether UI is available through `ExtensionContext::has_ui`; no terminal implementation is duplicated in the extension layer.

## Deferred

TypeScript/jiti source compatibility, hot unload, and unapproved installation dependencies are deferred. ABI v1 must not leak Rust trait objects, `String`, `Vec`, futures, or foreign pointers across the dynamic boundary; loaders do not unload libraries during process lifetime.
