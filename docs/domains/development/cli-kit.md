---
description: Read before launching machines, authenticating or managing installed resources with zf.
---

# Use the CLI kit

XState owns execution; Pi owns model selection, provider conversion and authentication.
Read [observed.yaml](observed.yaml) for evidence and limits.
Bootstrap through the [program guide](workspace-loop.md).

## Start a run

Inspect available commands, resources and effective settings:

```sh
pnpm zf --help
pnpm zf machines
pnpm zf models
pnpm zf config show agent
```

Review tools before starting an agent.
Tools inherit process privileges and ambient environment.
Code Mode bounds its VM but does not sandbox host tools.

Launch an interactive agent with `pnpm zf run agent`.
A bare `pnpm zf` opens the agent in a terminal.
Use a prompt for one headless turn:

```sh
pnpm zf run agent --prompt 'Explain this project without changes' --tools read,grep,find,ls
```

Use `--tui` to retain interaction after an initial prompt.
Use `--json` for the final view or `--stream` for headless text deltas.
Inspect stderr after an unsuccessful exit.
Generic machines accept JSON events through `--event` or interactive input.

## Use offline search tools

The operator delegates `grep` and `find` binary selection to Pi’s native offline resolver.
Pi chooses its installed binary or a command on `PATH`.
Install `rg` and `fd` before searching.
An absent or unusable selection returns an explicit error, even when another installation works.
The operator defers further fallback policy.
Zedflow does not retry another binary.

Search workers isolate offline settings from the caller and leave the user’s Pi cache unchanged.
Read-only searches require no temporary bin directory or executable links.
Custom operations retain their caller context, and cancellation waits for native child cleanup.

## Authenticate explicitly

Use `pnpm zf auth login` for the native subscription flow.
Add `--api-key` for masked key input.
Use `auth status` and `auth logout` to inspect and remove stored authentication.
Use `/login`, `/auth` and `/logout` inside the TUI.
Follow the native authentication instructions.

The program does not import existing Pi credentials.
Logout leaves ambient environment credentials unchanged.
Keep secrets out of settings, declarations and source control.
Use `--global-dir` for an isolated settings and credential directory.
Ignore `.zedflow/runs/`, project-local authentication files and installation identity files in each project.
Run records contain prompts and tool results.

## Select resources

Inspect settings and leaf provenance with `config show`.
Use JSON values for mutations:

```sh
pnpm zf config set tools '["read","grep"]' --machine agent
pnpm zf config set codemode.mode '"only"' --machine agent
pnpm zf config unset codemode.mode --machine agent
```

Choose `--scope global` for global mutations.
Objects merge; arrays replace earlier values.
CLI overrides remain effective during reload.
Code Mode and direct tools are available by default.
The `only` mode retains selected tools inside Code Mode while hiding direct declarations.
Skill bindings expose metadata and paths; read tools retrieve bodies.

Place native machines under project or global `.zedflow/machines/` directories.
Use qualified IDs for grouped resources.
Use the built-in counter as an authoring example.
Project resources override global resources, then built-ins; same-scope duplicates fail.
Restrictive machine configuration schemas must allow shared `tools` and `codemode` settings.

## Manage installations

Use `zf install` for reviewed local, Git or registry sources.
Inspect `zf installed` before updating or removing installations.
Choose the scope explicitly for global operations.
Use `zf restore --frozen` to retain resolutions.
Keep manager lock data for reproducible installation.
The manager refuses local drift and disables third-party installation scripts.
Investigate committed-cleanup diagnostics before retrying partial failures.

## Reload and stop

Use `/reload` after changing native code, configuration, tool declarations or skills.
Replacement waits for a compatible checkpoint without aborting or replaying its effect.
Failed preparation preserves the previous execution.
Use `/reset` to clear history and `/quit` to stop the interface and run.
Cancellation cannot undo completed effects.
Run records do not provide arbitrary crash recovery.

## Gotchas

Secret prompts disable echo before displaying readiness; immediate-input PTY tests enforce this ordering.
Signalled commands report errors even without numeric exit codes.
Relative tool executable paths resolve from their declaration directory; bare commands use PATH.
Processes retain project cwd, and trusted relative paths may leave their declaration directory.
