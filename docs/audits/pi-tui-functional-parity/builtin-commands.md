# Built-in commands

**Scope.** Every command in frozen Pi `core/slash-commands.ts`, its interactive dispatch, and Rust `core/slash-commands.rs`/`interactive-mode.rs`. The full inventory is [command-matrix.md](command-matrix.md).

## Tests/source evidence

The two `BUILTIN_SLASH_COMMANDS` arrays contain the same 22 named commands. Pi's interactive submit switch additionally handles developer commands `/debug`, `/arminsayshi`, and `/dementedelves`; they are not members of the built-in inventory and are therefore not matrix rows. Pi accepts only `/quit`; Rust parses unsupported `/exit` as Quit. Saved lane `3c8414b2` ran 13 focused builtins/compaction/import/clone tests successfully, but they do not cover placeholder output, quote parsing, or the alias. Saved lane `84e1369a` independently confirmed `/share`, tree/fork/name gaps.

## Matching behavior

Rust has the frozen 22 command names/descriptions, argument admission for model/import/export/name/compact, and dispatches all named actions rather than leaking recognized commands to model input. Existing focused tests cover selected routing and state paths.

## Mismatches

- **P1 — `/share`.** Pi performs offline `gh` auth/error handling, temporary HTML export, cancellable secret-gist creation, cleanup, and URL report; Rust only reports a configured-service status.
- **P2 — `/session`, `/changelog`, `/hotkeys`.** Rust status-only actions replace Pi transcript UI content.
- **P2 — `/tree`, `/fork`, `/name`, `/import`, `/export`.** See [sessions/navigation](sessions-navigation.md).
- **P2 — `/exit` alias.** Rust accepts an alias Pi does not dispatch.
- **P3 — command effects not marked mismatched in the matrix remain unproven at whole-CLI level.**

## Missing fidelity fixtures

New actual-CLI PTY tests must type each command, test arguments/malformed trailing forms, inspect selector/transcript/root result, and ensure known commands never become prompt text. `/share` needs a deterministic fake `gh` executable for auth failure/success/cancel/cleanup, not network access. These are separate fidelity tests; copied Pi command unit tests cannot substitute.

## Fix boundary

Repair only the frozen command parser and interactive command effects. Do not promote Pi's developer-only handlers into `BUILTIN_SLASH_COMMANDS`, retain the unsupported Rust alias, or substitute a fake success for `/share`.
