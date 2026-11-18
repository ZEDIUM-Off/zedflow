---
description: Read before changing the root workspace, package dependencies, built-in machines or quality rules.
---

# Work on the program

Read [observed.yaml](observed.yaml) for recorded evidence and limits.
Use the [CLI guide](cli-kit.md) to run the program.
Keep the [coordinator experiment](coordinator-prototype.md) as historical lifecycle context.

## Install and verify

Run these commands from the worktree root:

```sh
pnpm bootstrap
pnpm check
```

Use the bootstrap script's `--update-lockfile` argument only when intentionally changing dependencies.
Keep frozen-lockfile checks, dependency release-age checks and disabled installation scripts.
Keep downloaded dependencies and temporary evidence under `.artifacts/`.
Use Git history to retrieve archived experiments in another checkout.

## Keep dependencies directed

Keep shared interfaces and data contracts in `@zf/core`.
Keep storage implementations in `@zf/storage` and run coordination in `@zf/run`.
Keep discovery, module loading, skills and native provider integration in `@zf/capabilities`.
Keep adapted executable tools and Code Mode in `@zf/tools`.
Keep configuration in `@zf/config` and managed installations in `@zf/manager`.
Keep terminal interaction and reusable state views in `@zf/tui`.
Keep engine-specific integration in `@zf/xstate`.
Keep built-in machine definitions in `@zf/machines`.
Assemble their implementations in `@zf/cli`.
Keep the root entrypoint as a facade.
Do not make core contracts import their implementations.

## Use classes for behavior

Give each stateful implementation an explicit constructor and a small public interface.
Inject collaborators through interfaces.
Use composition instead of a shared base-class hierarchy.
Use hard-private fields for internal state and readonly references for stable collaborators.
Use ordinary prototype methods with explicit parameter and return types.
Bind methods when passing callbacks that need their receiver.

Keep records, events and options as interfaces or data types.
Keep stateless transformations as functions.
Keep XState definitions native rather than wrapping them in application classes.
Do not introduce empty classes, marker interfaces or static-only utility classes.

Use the compiler and lint configuration as the executable policy.
Read the [code limits guide](code-limits.md) before you split code.
Run the negative quality tests after changing that policy.
Keep runtime transpilation separate from static validation of dynamically loaded machines.

## Use capabilities deliberately

Resolve project capabilities before global capabilities.
List skill metadata before reading selected skill content.
Treat skill text as data, not executable instructions for the loader.
Expose skill descriptions and paths through machine bindings.
Let the model read relevant bodies through a selected read tool.
Bind executable tools explicitly.
Keep external command discovery separate from execution.

## Launch a built-in machine

Construct `Zedflow` with explicit directories and optional native Pi dependencies.
Use Pi AI `Models`, `Provider` and authentication interfaces directly.
Launch `agent` through the application.
Send `PROMPT` through `runs`.
Use `CANCEL` for active work and `RESET` to clear the interaction.
Continue failed or cancelled turns with another prompt when history must remain available.

Use the same launch path for user machines.
Keep replacement within the supported checkpoint profile.
Abort caller-owned reload preparation before stopping its run.
Keep models, tools and other live services outside persisted machine context.
Use local TypeScript dependencies for cache-free replacement.
Ordinary local JavaScript and JSON imports are refused in this profile.
Installed native dependencies remain supported; in-place dependency changes require a restart.
Trusted modules, runtime filesystem reads and post-preparation imports are not a complete source closure or sandbox.
Command declarations participate in generation identity; executable script bytes remain mutable external effects.
A hung trusted loader requires caller-owned cancellation to release preparation.
Cancellation cannot undo late module side effects.

## Keep development inspection optional

Use [Stately Studio](https://stately.ai/docs/studio) and its editor as external development tools.
Keep the inspector SDK in development dependencies.
Construct `DevelopmentInspector` only when inspection is requested.
Use its engine with an explicitly constructed coordinator.
Dispose the inspector and the caller-owned transport separately.
Qualify visual editing and network transport separately from offline inspection tests.
