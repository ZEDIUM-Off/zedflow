# Pi Rust porting rules

Use these rules for the identity port from `references/pi/packages/` into `crates/zedflow-*`.

## Scope

- Preserve Pi behavior and package boundaries as closely as Rust conventions allow.
- Do not add Zedflow-specific runtime behavior while porting Pi packages.
- Do not add old monolith compatibility shims, temporary aliases, or type weakening.
- Do not replace unresolved TypeScript dependencies with speculative Rust designs.

## Public Rust API rules

- Document every public item with rustdoc.
- Add `# Errors` to every fallible public function.
- Add `# Panics` only for intentional panic invariants.
- Prefer explicit, typed errors from `zedflow_core::error` for shared port infrastructure.
- Keep async APIs cancel-safe: do not hold locks or borrowed temporaries across `.await`.

## Placeholder policy

A placeholder is allowed only when a Pi TypeScript dependency or API has no selected Rust replacement yet. The Rust target must still compile, keep the intended public shape documented, and preserve the exact behavior requirement for the future replacement.

Every source-level placeholder must include this exact marker:

```rust
/// PORT PLACEHOLDER:
/// Original dependency: `<npm package / API>`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `<exact Pi behavior to preserve>`.
/// Replacement decision needed before production use.
```

Use `zedflow_core::placeholders::unsupported` only behind that marker. Do not use placeholders for convenience, incomplete local code, or legacy monolith compatibility.

## Testing

- Port deterministic Pi tests to Rust tests in the target package crate.
- Mark live-network or integration-only parity tests as ignored with an explicit parity blocker.
- Do not run live provider or network tests unless a task explicitly allows it.
