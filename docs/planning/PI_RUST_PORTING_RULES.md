# Pi Rust porting rules

Use these rules for Stage 1: the identity port from frozen `references/pi/packages/`.

## One-to-one scope

Stage 1 contains exactly five package/crate pairs:

| Frozen Pi package | Rust crate |
|---|---|
| `packages/ai` | `zedflow-ai` |
| `packages/agent` | `zedflow-agent` |
| `packages/tui` | `zedflow-tui` |
| `packages/coding-agent` | `zedflow-coding-agent` |
| `packages/orchestrator` | `zedflow-orchestrator` |

Default mapping is one TypeScript source/test to one Rust source/test. Preserve package dependency direction: Coding-agent depends on AI, Agent, and TUI; Orchestrator depends on Coding-agent.

Stage 1 has no Flow, LangGraph, shared-core, tools, or session crate. Do not add Zedflow-specific runtime behavior, monolith compatibility shims, temporary aliases, or type weakening.

## Non-exact mappings

Every frozen `.ts`, `.tsx`, and `.d.ts` file must appear in its package manifest or the exception ledger. A non-exact mapping must use one explicit disposition:

- `consolidated` — several frozen files intentionally share one existing Rust target;
- `type-only` — an ambient declaration has no runtime implementation;
- `platform-specific` — the existing Rust target differs only because of the platform;
- `live-capability` — an existing Rust test is ignored only for an external capability;
- `dependency-arbitration` — no Rust dependency replacement has been approved.

`dependency-arbitration` blocks the controller. Its evidence must identify the npm/API behavior, candidate Rust crates or standard-library option, licenses, MSRV/platform/async differences, observable semantic gaps, and a recommendation. No worker may choose a speculative replacement.

Run the deterministic closure gate with:

```bash
python3 tools/pi-port-swarm/manifest.py check --package zedflow-ai
python3 tools/pi-port-swarm/manifest.py check
```

Target presence is necessary but does not prove semantic fidelity; independent reviews remain required.

## Public Rust API rules

- Document every public item with rustdoc.
- Add `# Errors` to every fallible public function.
- Add `# Panics` only for intentional panic invariants.
- Keep package-local typed errors in the package crate that owns the behavior.
- Keep async APIs cancel-safe: do not hold locks or borrowed temporaries across `.await`.

## Placeholder policy

A placeholder is allowed only when a frozen dependency/API has no approved Rust replacement. It must remain package-local, compile, preserve the intended public shape, and include:

```rust
/// PORT PLACEHOLDER:
/// Original dependency: `<npm package / API>`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `<exact Pi behavior to preserve>`.
/// Replacement decision needed before production use.
```

The corresponding manifest row must be `dependency-arbitration`; therefore the package cannot close. Placeholders are forbidden for convenience, incomplete local code, or compatibility.

## Testing

- Port deterministic Pi tests to the matching package crate.
- Mark live-network/capability tests ignored only with an explicit current disposition.
- Do not run live provider/network tests unless a task explicitly allows it.
- Package closure runs executable tests, not only `--no-run`.
