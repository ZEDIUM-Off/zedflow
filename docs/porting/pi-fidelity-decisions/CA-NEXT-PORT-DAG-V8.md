# Coding-agent model-registry acceptance and model-resolution wave

## Accepted model-registry wave

At base `676fb3fe90ac268ecc467d5c92e42d897e1492d7`, `CA-RV-FID-V10-R1` accepted the repaired coding-agent model-registry wave after mechanical and focused validation. The accepted wave covers built-in and custom model registration, provider configuration and validation, authentication resolution, dynamic API and OAuth providers, OAuth `modifyModels`, and GitHub Copilot model rewriting and filtering. The worktree and frozen `references/pi` gitlink were clean at checkpoint start.

## Next bounded wave

The next wave stays primarily inside `zedflow-coding-agent` and ports only model reference matching, pattern parsing, scoped-model resolution, and diagnostics from frozen `core/model-resolver.ts`.

A workspace validator and fresh Pi-fidelity reviewer gate the next checkpoint. CLI initial selection, session restoration, settings, compaction execution, session runtime, extensions, package management, RPC, CLI, interactive TUI, and global HTTP dispatcher/fetch installation remain unassigned.
