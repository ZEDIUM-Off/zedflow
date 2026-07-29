# Rust extension architecture v1

## Decision

Zedflow's canonical extension model is Rust, compiled separately from the core and loaded in-process. It preserves Pi's `ExtensionAPI` capabilities while replacing Pi's jiti TypeScript execution boundary. TypeScript/jiti compatibility is deferred to a later adapter and is not required for the current Stage-1 closure.

The Stage-1 workspace remains exactly five crates. The author-facing SDK and ABI live in `zedflow-coding-agent` for now.

## Author contract

An extension is a source crate whose library target uses `crate-type = ["cdylib"]`, depends on the public Zedflow extension SDK, implements the Rust `Extension` registration contract, and exports the single ABI-v1 entry point through `export_extension!`.

The Rust `ExtensionApi` must cover Pi-equivalent registration and runtime capabilities: tools, commands, keybindings, flags, lifecycle/input/session events, providers/models, compaction hooks, UI requests/components/widgets, session/model actions, errors, stale-context invalidation and shutdown.

## ABI v1

The dynamic boundary is a small custom C ABI loaded with exact dependency `libloading = "=0.9.0"`. No Rust-owned type crosses it. ABI structures use `#[repr(C)]`, fixed-width scalars, pointer/length byte views, opaque generation-checked handles and `extern "C"` function pointers. Higher-level requests and responses use bounded, versioned JSON envelopes through existing Serde.

Required invariants:

- ABI version and every table's `struct_size` are validated before field access.
- Null/length consistency, alignment, UTF-8/JSON and maximum-message constraints are checked before use. Pointer validity itself is an ABI precondition supplied by the trusted SDK-built extension; no in-process loader can safely prove an arbitrary native pointer is valid.
- Allocation and free operations are paired with the allocating side.
- No panic or unwind may cross the ABI; the SDK export wrapper catches panics and returns a status.
- No host lock is held while invoking extension code; reentrant calls are explicit and tested.
- Handles include type/generation validation, idempotent cancellation/destruction and stale-handle rejection.
- The host retains every loaded `Library` for process lifetime in v1. Reload disables and shuts down old instances but does not unload their libraries.
- Loading occurs only after canonical-path, trust and SHA-256 artifact verification. The absolute content-addressed artifact path is rechecked immediately before load.
- Native code is fully trusted after activation and can bypass ExtensionAPI permissions, violate pointer contracts or terminate/corrupt the process; provenance and explicit trust are security boundaries, not a sandbox. `catch_unwind` only contains conforming SDK callbacks that panic before crossing the C ABI—it cannot contain invalid memory access, abort or foreign unwinding.

The loader's `unsafe` is confined to one documented module. Unsafe remains denied elsewhere.

## Source installation protocol

Accepted source identifiers:

```text
crate:<crate-name>@<exact-version>
github:<owner>/<repository>@<tag-or-commit>[#<cargo-package>]
path:<local-development-path>
```

Tags and branches resolve to immutable commits. Prebuilt release artifacts are never installed. The installer obtains source, resolves dependencies, and builds locally with Cargo in a staging directory using a sanitized environment, isolated target directory, locked resolution and offline build after fetch/vendor. It reuses `sha2 = "0.10"`, already present in the workspace, for source/lock/artifact receipts.

The receipt records canonical source identifier, registry checksum or Git commit, selected Cargo package, source hash, Cargo.lock hash, rustc version, target triple, profile, ABI version and artifact hash. Installed artifacts live in a user-owned content-addressed store. Activation binds explicit project/global trust to the canonical source and artifact digest.

Building locally proves which pinned source produced the installed artifact; it does not prove that source is benign. Build-script/proc-macro isolation is a separate platform hardening requirement. Installation must never claim native runtime sandboxing.

## Lifecycle v1

Extensions are discovered and loaded at startup or explicit reload. Updating an extension requires a local rebuild and Zedflow restart/reload. V1 never unloads a native library during the process lifetime. A failed new load leaves the prior active extension/resource state intact.

## Required gates

- separately compile a real fixture `cdylib` against the public SDK;
- load it and exercise tools, commands, events, providers, UI and shutdown;
- reject missing symbol, ABI/table mismatch, malformed/oversized messages, null/length contract violations, digest mismatch, untrusted source and stale handles; arbitrary non-null invalid pointers are outside the trusted-native ABI contract and must not be claimed as safely rejectable;
- verify panic containment at the SDK boundary, callback ordering, reentrancy, cancellation races and shutdown exactly once;
- test crates.io, GitHub commit/tag, monorepo package and local-path source resolution without accepting prebuilt binaries;
- preserve Pi capability behavior through Rust extension fixtures; source-level TS compatibility is explicitly deferred.
