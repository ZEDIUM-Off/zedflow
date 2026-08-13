# Sessions and navigation

**Scope.** Resume, clone, fork, tree navigation, naming, import/export path handling, and transcript/editor restoration.

## Tests/source evidence

Pi `interactive-mode.ts` guards empty fork/tree selectors (`No messages to fork from` / `No entries in session`), and its tree flow handles current leaf, branch-summary/cancel choice, transcript rerender, and conditional editor restoration. Rust dispatches `Fork`/`Tree` to selectors in `interactive-mode.rs`; its overlay can display empty text but still mounts. Saved lane `84e1369a` source comparison found Rust navigation simply `move_to` plus status, omitting those flows. It also reports 14 focused interactive selector/builtin/import/clone tests passing; those tests do not cover the listed visible gaps.

For paths, Pi `getPathCommandArgument` consumes a first token and strips outer quotes. Rust parser/action passes the complete remaining argument to `PathBuf`, as saved lane `3c8414b2` verified. Pi `/name` reports current name or normalized-name warnings; Rust gives generic set/requested statuses.

## Matching behavior

Both have session manager, resume/session selectors, clone/import/export actions, tree/user-message selectors, and persisted session machinery. Rust has focused tests for clone/import/session selector paths.

## Mismatches

- **P2 — empty `/fork` and `/tree`.** Rust mounts an empty overlay instead of Pi's status/no-open behavior.
- **P2 — tree navigation.** Current-leaf no-op, branch summary/cancel, transcript rerender, and editor restoration differ.
- **P2 — import/export quoting.** Full trailing argument is treated as a path rather than Pi's first outer-quote-stripped token.
- **P2 — `/name` output.** Current-name and normalized-name warning semantics differ.
- **P3 — resume/clone/fork persistence and exact transcript restoration** lack raw-PTY proof.

## Missing fidelity fixtures

Add isolated session graphs to actual-CLI fixtures: empty/nonempty fork/tree, current leaf, branch summary/cancel, selected branch transcript/editor state, quoted/trailing import/export path parsing, name query/normalization, resume and clone reopen. Keep fixture sessions local and deterministic; no filesystem deletion outside their temporary HOME.

## Fix boundary

Limit repair to session command/selector orchestration, parser compatibility, and session UI restoration. Do not alter the session file format or branch policy absent a frozen-source finding.
