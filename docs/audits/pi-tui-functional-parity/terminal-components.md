# Terminal and common components

**Scope.** Terminal lifecycle/cell rendering and the `zedflow-tui` one-to-one component layer used by interactive chat and selectors.

## Tests/source evidence

Pi and Rust contain paired terminal, TUI, utils, primitives, box, input, markdown, select-list, settings-list, text, truncation, image, loader, editor, and keyboard modules. Rust test files mirror most Pi `packages/tui/test` names (listed in [one-to-one-file-map](one-to-one-file-map.md)). Saved lane `3d3dc44e` reports seven `tools/tui-parity` component fixtures and focused editor family tests passing. Saved lane `efb77538` reports focused theme/footer tests passing. These are useful unit/component checks, not raw terminal compatibility proof.

`tools/tui-fidelity/run.py` starts a `pty`, dispatches the frozen Pi CLI and built Rust binary, decodes both raw captures using one `@xterm/headless` decoder, and compares cells/styles/cursor. Its README records that `login.json` and `settings.json` currently differ. Saved lane `c0fa6b31` reports harness syntax checks passed and confirms those two offline differences.

## Matching behavior

A real-CLI differential harness exists, uses frozen Pi `2b00dade`, isolates HOME, avoids network, and treats a difference as failure. Rust mirrors core TUI modules and has broad unit tests for width/ANSI/overlay/render/terminal behavior.

## Mismatches

- **P1 — real differential gate red.** Both supplied raw-PTY scenarios differ in terminal cells/cursor.
- **P1 — harness coverage is too narrow.** It covers only login/settings; chat streaming, themes, editor visual navigation, session commands, terminal title/progress, and common-component edge cases have no actual-CLI trace.
- **P2 — root composition/style differences** are visible in the supplied captures and must be assigned to feature lanes, not normalized by the decoder.
- **P3 — terminal protocol parity unknown.** Mouse, focus, clipboard, image, suspend/restore, OSC hyperlinks/133/title/progress, and platform modifier behavior are not established by the two fixtures.

## Missing fidelity fixtures

First expand the same raw-PTY harness, not a parallel snapshot system. Fixtures must invoke real frozen Pi and Rust CLI dispatch and compare cells/styles/cursor/raw protocol bytes. Add deterministic offline fixture families for chat, theme/chrome, settings/login, editor, sessions/commands, resize/Unicode, and terminal protocol. A failure must retain both raw/decode artifacts; expected output must never be copied from either implementation.

## Fix boundary

Own harness reliability/fixtures and shared `zedflow-tui` terminal/component behavior. Feature-specific visible repairs remain in their lane; do not weaken comparison, hide regions, or bless baseline frames.
