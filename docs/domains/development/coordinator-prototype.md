---
description: Read before implementing the program entrypoint, coordinator or TypeScript machine reload.
---

# Exercise the coordinator architecture

Use this experiment to assess the program structure before implementing the full kit.
Read `coordinator_prototype` in [observed.yaml](observed.yaml) for the historical experiment.
Use the [program guide](workspace-loop.md) for current operation.
Retrieve the original experiment from its recorded Git revision or the local `.artifacts/experiments/` archive.
Use the [minimal kit analysis](mvp-kit-analysis.md) for the wider responsibility split.

## Keep native definitions

Export ordinary XState machines from TypeScript modules.
Use native machine identity and version fields.
Keep XState-specific lifecycle, inspection and restoration code inside the adapter.
Keep the coordinator independent of XState types and snapshot internals.
Treat its execution interface as an internal integration seam, not another machine language.
Expect machine authors to follow their selected XState version.

## Assemble the program

Use the core entrypoint to assemble the loader, coordinator, adapter and storage.
Resolve project definitions before global definitions.
Provide built-in tools without requiring user configuration.
Discover additional tool declarations under configured `.zedflow/tools/` directories.
Do not execute programs while discovering declarations.

Share the adapter's XState modules with the TypeScript loader.
Reload user code without creating another copy of the engine.
Keep static checking distinct from runtime transpilation.
Validate authored machines before treating them as trusted executable code.

## Replace deliberately

Launch new runs with freshly loaded modules.
Leave existing executions on their original definitions until replacement is requested.
Prepare and validate a replacement before stopping its predecessor.
Record the replacement revision, checkpoint and execution generation.
Refuse concurrent commands during replacement.

Limit replacement to the supported child-free, timer-free checkpoint profile.
Do not treat absence of children as proof that arbitrary user code has no external effects.
Reject incompatible states or changed identity/version instead of silently resetting context.
Keep migration and general crash recovery outside this experiment.

## Run the checks

From the mission worktree, install the pinned dependencies:

```sh
pnpm bootstrap
```

Run formatting verification, linting, strict type checking and offline tests:

```sh
pnpm check
```

Use `pnpm format` at the worktree root to apply formatting.
Inspect test artifacts under `.artifacts/coordinator-prototype/`.

## Respect the scope

Use the FS adapter only with one writer and bounded experimental records.
Do not infer power-loss durability or effect-safe replay from successful appends.
Evaluate a network transport before connecting an editor.
Keep external tool execution, automatic nested-run registration and Stately journal integration as separate experiments.
